//! # HBA (Host Bus Adapter) Controller
//!
//! Top-level AHCI controller management: PCI device discovery,
//! MMIO BAR mapping, HBA reset, AHCI mode enable, and capability
//! detection.

#![allow(dead_code)]

use crate::ffi;
use crate::registers::{self, hba, PortState, MAX_PORTS};
use crate::port::Port;

use core::ptr;

/// Global HBA singleton (accessible via `hba()` function).
static mut HBA: Option<HbaController> = None;

/// Get a reference to the global HBA controller.
/// SAFETY: caller must guarantee single-threaded access.
pub unsafe fn hba() -> &'static mut HbaController {
    unsafe { &mut *core::ptr::addr_of_mut!(HBA) }
        .as_mut()
        .expect("HBA not initialized")
}

/// Set the global HBA controller.
/// SAFETY: caller must guarantee single-threaded access.
pub unsafe fn set_hba(ctrl: HbaController) {
    unsafe {
        core::ptr::addr_of_mut!(HBA).write(Some(ctrl));
    }
}

/// Shared reference to HBA (for port operations via HbaRef).
pub struct HbaRef(*mut u8, usize);

impl HbaRef {
    pub fn new(base: *mut u8, size: usize) -> Self {
        Self(base, size)
    }

    /// Get the MMIO base address.
    pub fn base(&self) -> *mut u8 {
        self.0
    }

    /// Get the MMIO region size.
    pub fn size(&self) -> usize {
        self.1
    }

    /// Read a 32-bit HBA register.
    pub fn hba_read32(&self, reg_idx: usize) -> u32 {
        unsafe { ffi::read32_raw(self.0 as usize + reg_idx * 4) }
    }

    /// Write a 32-bit HBA register.
    pub fn hba_write32(&self, reg_idx: usize, val: u32) {
        unsafe { ffi::write32_raw(self.0 as usize + reg_idx * 4, val) }
    }
}

unsafe impl Send for HbaRef {}
unsafe impl Sync for HbaRef {}

/// Complete AHCI HBA controller state.
pub struct HbaController {
    /// MMIO reference.
    pub mmio: HbaRef,
    /// MMIO region size.
    pub mmio_size: usize,
    /// Number of addressable ports.
    pub nr_ports: usize,
    /// Maximum commands per port.
    pub nr_cmds: usize,
    /// NCQ support flag.
    pub has_ncq: bool,
    /// CLO support flag.
    pub has_clo: bool,
    /// IRQ number.
    pub irq: i32,
    /// IRQ hook ID.
    pub hook_id: i32,
    /// PCI device index.
    pub devind: i32,
    /// Per-port state.
    pub ports: [Port; MAX_PORTS],
    /// Driver instance number.
    pub instance: i32,
    /// Verbosity level (0..4).
    pub verbose: u8,
}

impl HbaController {
    /// Probe for an AHCI PCI device.
    pub fn probe(skip: i32) -> Option<i32> {
        ffi::pci_init_ffi();
        let (devind, _, _) = ffi::pci_first_dev_ffi()?;

        let mut devind = devind;
        for _ in 0..skip {
            (devind, _, _) = ffi::pci_next_dev_ffi()?;
        }

        ffi::pci_reserve_ffi(devind);
        Some(devind)
    }

    /// Reset the HBA (global reset, keep AHCI enable after).
    pub fn reset(&self) {
        let ghc = self.mmio.hba_read32(hba::GHC);
        // Enable AHCI before reset
        self.mmio.hba_write32(hba::GHC, ghc | hba::GHC_AE);
        // Assert reset
        self.mmio.hba_write32(hba::GHC, ghc | hba::GHC_AE | hba::GHC_HR);

        // Wait for reset to complete
        let timeout = 1_000_000; // 1 second
        let mut waited = 0u32;
        while (self.mmio.hba_read32(hba::GHC) & hba::GHC_HR) != 0 && waited < timeout {
            ffi::udelay(10);
            waited += 10;
        }

        if (self.mmio.hba_read32(hba::GHC) & hba::GHC_HR) != 0 {
            ffi::driver_panic(b"AHCI: unable to reset HBA\0");
        }
    }

    /// Initialize the HBA from a PCI device index.
    pub fn init(devind: i32, instance: i32, verbose: u8) -> Self {
        // Read PCI BAR[5] for AHCI
        let (base, size, ioflag) = ffi::pci_get_bar_ffi(devind, 5)
            .or_else(|| ffi::pci_get_bar_ffi(devind, 6))
            .expect("AHCI: no valid BAR found");

        if ioflag {
            ffi::driver_panic(b"AHCI: invalid BAR type (I/O, expected MMIO)\0");
        }

        if (size as usize) < registers::MEM_BASE_SIZE + registers::MEM_PORT_SIZE {
            ffi::driver_panic(b"AHCI: HBA memory size too small\0");
        }

        let real_size = core::cmp::min(
            size as usize,
            registers::MEM_BASE_SIZE + registers::MEM_PORT_SIZE * MAX_PORTS,
        );
        let nr_ports = (real_size - registers::MEM_BASE_SIZE) / registers::MEM_PORT_SIZE;

        // Map the MMIO region
        let mmio_base = ffi::vm_map_phys_ffi(base as *mut core::ffi::c_void, real_size);
        if mmio_base.is_null() {
            ffi::driver_panic(b"AHCI: unable to map HBA memory\0");
        }

        // Read IRQ from PCI config
        let irq = ffi::pci_attr_r8_ffi(devind, 0x3C) as i32; // PCI_ILR

        // Set up IRQ
        let hook_id = ffi::irq_setup(irq).expect("AHCI: unable to register IRQ");

        let mmio = HbaRef::new(mmio_base as *mut u8, real_size);

        let mut hba = Self {
            mmio,
            mmio_size: real_size,
            nr_ports,
            nr_cmds: 1,
            has_ncq: false,
            has_clo: false,
            irq,
            hook_id,
            devind,
            ports: [
                Port::new(), Port::new(), Port::new(), Port::new(),
                Port::new(), Port::new(), Port::new(), Port::new(),
                Port::new(), Port::new(), Port::new(), Port::new(),
                Port::new(), Port::new(), Port::new(), Port::new(),
                Port::new(), Port::new(), Port::new(), Port::new(),
                Port::new(), Port::new(), Port::new(), Port::new(),
                Port::new(), Port::new(), Port::new(), Port::new(),
                Port::new(), Port::new(), Port::new(), Port::new(),
            ],
            instance,
            verbose,
        };

        // Reset the HBA
        hba.reset();

        // Enable AHCI and interrupts
        let ghc = hba.mmio.hba_read32(hba::GHC);
        hba.mmio.hba_write32(hba::GHC, ghc | hba::GHC_AE | hba::GHC_IE);

        // Read capabilities
        let cap = hba.mmio.hba_read32(hba::CAP);
        hba.has_ncq = (cap & hba::CAP_SNCQ) != 0;
        hba.has_clo = (cap & hba::CAP_SCLO) != 0;
        hba.nr_cmds = core::cmp::min(
            registers::MAX_CMDS,
            (((cap >> hba::CAP_NCS_SHIFT) & hba::CAP_NCS_MASK) + 1) as usize,
        );

        // Initialize implemented ports
        let pi = hba.mmio.hba_read32(hba::PI);
        for port_idx in 0..hba.nr_ports {
            let pstate = &mut hba.ports[port_idx];
            pstate.device_id = -1;
            if (pi & (1u32 << port_idx)) != 0 {
                hba.mmio.port_init(port_idx, pstate);
            }
        }

        hba
    }

    /// Clean up HBA resources.
    pub fn stop(&mut self) {
        for port_idx in 0..self.nr_ports {
            let state = self.ports[port_idx].state;
            if state != PortState::NoPort {
                self.mmio.port_stop(port_idx);
                // Port buffers freed automatically on drop
            }
        }

        self.reset();

        let _ = ffi::vm_unmap_phys_ffi(self.mmio.base() as *mut core::ffi::c_void, self.mmio_size);
        let _ = ffi::irq_remove(&mut self.hook_id);
    }

    /// Log a formatted message at the given verbosity level.
    pub fn log(&self, level: u8, msg: &[u8]) {
        if self.verbose >= level {
            ffi::print(msg);
            ffi::print(b"\n\0");
        }
    }
}
