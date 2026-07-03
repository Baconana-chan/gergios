//! # HBA (Host Bus Adapter) Controller
//!
//! Top-level AHCI controller management: PCI device discovery,
//! MMIO BAR mapping, HBA reset, AHCI mode enable, capability
//! detection, and MSI-X per-port interrupt support.

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

/// MSI-X table entry (16 bytes): maps an IRQ vector to a PCI MSI-X message.
///
/// Per PCI 3.0 spec, each entry contains:
///   QWORD Message Address  (offset 0)
///   DWORD  Message Data    (offset 8)
///   DWORD  Vector Control  (offset 12, bit 0 = Mask)
#[repr(C)]
struct MsixTableEntry {
    msg_addr: u32,      // Message Address (lower 32 bits)
    msg_addr_upper: u32,// Message Upper Address (upper 32 bits)
    msg_data: u32,      // Message Data (vector)
    ctrl: u32,          // Vector Control (bit 0 = mask)
}

/// Complete AHCI HBA controller state, extended with MSI-X per-port IRQ.
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
    /// Legacy IRQ number (when MSI-X is unavailable).
    pub irq: i32,
    /// Legacy IRQ hook ID.
    pub hook_id: i32,
    /// PCI device index.
    pub devind: i32,
    /// Per-port state.
    pub ports: [Port; MAX_PORTS],
    /// Driver instance number.
    pub instance: i32,
    /// Verbosity level (0..4).
    pub verbose: u8,
    // --- MSI-X per-port interrupt fields ---
    /// Whether MSI-X is available and configured.
    pub msix_available: bool,
    /// Pointer to the MSI-X table in mapped memory (null if not available).
    pub msix_table_base: *mut u8,
    /// Size of the MSI-X table region.
    pub msix_table_size_bytes: usize,
    /// Per-port MSI-X IRQ vector numbers (0 = unallocated).
    pub per_port_irqs: [i32; MAX_PORTS],
    /// Per-port MSI-X IRQ hook IDs (0 = unregistered).
    pub per_port_hook_ids: [i32; MAX_PORTS],
    /// Whether the MSI-X table was mapped from a separate BAR (needs separate unmap).
    pub msix_separate_bar: bool,
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

    /// Try to initialize MSI-X per-port interrupts.
    /// Returns (ok, table_base, table_bytes, separate_bar, table_size)
    fn init_msix(devind: i32, mmio_base: *mut u8) -> (bool, *mut u8, usize, bool, usize) {
        // Parse MSI-X capability from PCI config space
        let msix_info = match ffi::pci_msix_parse_ffi(devind) {
            Some(info) => info,
            None => return (false, ptr::null_mut(), 0, false, 0),
        };

        let table_offset_bytes = (msix_info.msix_table_offset as usize) * 8;
        let table_size = msix_info.msix_table_size as usize;
        let table_bytes = table_size * 16; // 16 bytes per entry

        // Check if MSI-X table is in BAR5 (the HBA's memory BAR) or a separate BAR
        if msix_info.msix_table_bir == 5 || msix_info.msix_table_bir == 6 {
            // Table is within the already-mapped HBA BAR — compute offset
            let table_base = unsafe { mmio_base.add(table_offset_bytes) };
            (true, table_base, table_bytes, false, table_size)
        } else {
            // Table is in a separate BAR — map it
            let bar_idx = msix_info.msix_table_bir;
            let (bar_base, bar_size, ioflag) = match ffi::pci_get_bar_ffi(devind, bar_idx) {
                Some(v) => v,
                None => return (false, ptr::null_mut(), 0, false, 0),
            };
            if ioflag {
                return (false, ptr::null_mut(), 0, false, 0);
            }
            let map_size = core::cmp::min(bar_size as usize, table_offset_bytes + table_bytes);
            let table_bar = ffi::vm_map_phys_ffi(bar_base as *mut core::ffi::c_void, map_size);
            if table_bar.is_null() {
                return (false, ptr::null_mut(), 0, false, 0);
            }
            let table_base = unsafe { (table_bar as *mut u8).add(table_offset_bytes) };
            (true, table_base, table_bytes, true, table_size)
        }
    }

    /// Program one MSI-X table entry for a given port.
    ///
    /// Message Address: 0xFEE00000 + (APIC ID << 12) — x86 LAPIC base.
    /// Message Data:    IRQ0_VECTOR + irq — the vector delivered to the CPU.
    /// Vector Control:  0 = unmasked.
    unsafe fn program_msix_entry(table_base: *mut u8, port_idx: usize, irq: i32) {
        let entry = table_base.add(port_idx * 16) as *mut MsixTableEntry;
        // For now, assume APIC ID = 0 (BSP). Multi-socket support deferred.
        let msg_addr: u32 = 0xFEE00000 | (0 << 12);
        // Vector = IRQ0_VECTOR (0x50) + MSI-X IRQ number
        let msg_data: u32 = 0x50u32 + irq as u32;
        unsafe {
            ptr::write_volatile(&mut (*entry).msg_addr, msg_addr);
            ptr::write_volatile(&mut (*entry).msg_addr_upper, 0);
            ptr::write_volatile(&mut (*entry).msg_data, msg_data);
            ptr::write_volatile(&mut (*entry).ctrl, 0); // unmasked
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

        // Read IRQ from PCI config (used as fallback if MSI-X unavailable)
        let irq = ffi::pci_attr_r8_ffi(devind, 0x3C) as i32; // PCI_ILR

        let mmio = HbaRef::new(mmio_base as *mut u8, real_size);

        let mut hba = Self {
            mmio,
            mmio_size: real_size,
            nr_ports,
            nr_cmds: 1,
            has_ncq: false,
            has_clo: false,
            irq,
            hook_id: -1, // will be set if MSI-X fails
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
            msix_available: false,
            msix_table_base: ptr::null_mut(),
            msix_table_size_bytes: 0,
            per_port_irqs: [0i32; MAX_PORTS],
            per_port_hook_ids: [0i32; MAX_PORTS],
            msix_separate_bar: false,
        };

        // Reset the HBA
        hba.reset();

        // Enable AHCI
        let ghc = hba.mmio.hba_read32(hba::GHC);
        hba.mmio.hba_write32(hba::GHC, ghc | hba::GHC_AE);

        // Read capabilities and ports implemented
        let cap = hba.mmio.hba_read32(hba::CAP);
        hba.has_ncq = (cap & hba::CAP_SNCQ) != 0;
        hba.has_clo = (cap & hba::CAP_SCLO) != 0;
        hba.nr_cmds = core::cmp::min(
            registers::MAX_CMDS,
            (((cap >> hba::CAP_NCS_SHIFT) & hba::CAP_NCS_MASK) + 1) as usize,
        );
        let pi = hba.mmio.hba_read32(hba::PI);

        // Attempt MSI-X initialization
        let (msix_ok, table_base, table_bytes, separate_bar, msix_table_size) =
            Self::init_msix(devind, mmio_base as *mut u8);

        if msix_ok {
            // Allocate MSI-X IRQs only for implemented ports, up to table/IRQ pool limit
            let max_msix = core::cmp::min(msix_table_size, 16);
            let mut msix_ports = 0usize;
            let mut all_ok = true;

            for port_idx in 0..core::cmp::min(nr_ports, max_msix) {
                if (pi & (1u32 << port_idx)) == 0 { continue; }

                // Allocate MSI-X IRQ vector
                let (irq_val,) = match ffi::msix_alloc_irq() {
                    Some(v) => v,
                    None => {
                        all_ok = false;
                        break;
                    }
                };

                // Register IRQ handler
                let hook_id = match ffi::msix_setup(irq_val) {
                    Some(h) => h,
                    None => {
                        let _ = ffi::msix_free_irq(irq_val);
                        all_ok = false;
                        break;
                    }
                };

                hba.per_port_irqs[port_idx] = irq_val;
                hba.per_port_hook_ids[port_idx] = hook_id;
                msix_ports += 1;

                // Program MSI-X table entry
                unsafe {
                    Self::program_msix_entry(table_base, port_idx, irq_val);
                }
            }

            if all_ok && msix_ports > 0 {
                hba.msix_available = true;
                hba.msix_table_base = table_base;
                hba.msix_table_size_bytes = table_bytes;
                hba.msix_separate_bar = separate_bar;
                if verbose >= 1 {
                    ffi::print(b"AHCI: MSI-X enabled\n\0");
                }
            } else {
                // Clean up partially-allocated MSI-X IRQs
                for port_idx in 0..core::cmp::min(nr_ports, max_msix) {
                    if hba.per_port_hook_ids[port_idx] != 0 {
                        let _ = ffi::irq_remove(&mut hba.per_port_hook_ids[port_idx]);
                        hba.per_port_hook_ids[port_idx] = 0;
                    }
                    if hba.per_port_irqs[port_idx] != 0 {
                        let _ = ffi::msix_free_irq(hba.per_port_irqs[port_idx]);
                        hba.per_port_irqs[port_idx] = 0;
                    }
                }
                // Unmap separate BAR if it was mapped before failure
                if separate_bar && !table_base.is_null() {
                    let _ = ffi::vm_unmap_phys_ffi(
                        table_base as *mut core::ffi::c_void,
                        table_bytes,
                    );
                }
            }
        }

        // Fall back to legacy IRQ if MSI-X is unavailable or failed
        if !hba.msix_available {
            if verbose >= 1 {
                ffi::print(b"AHCI: MSI-X unavailable, using legacy IRQ\n\0");
            }
            let hook_id = ffi::irq_setup(irq).expect("AHCI: unable to register legacy IRQ");
            hba.hook_id = hook_id;
        }

        // Enable global HBA interrupts (required for BOTH legacy and MSI-X per AHCI spec)
        let ghc = hba.mmio.hba_read32(hba::GHC);
        hba.mmio.hba_write32(hba::GHC, ghc | hba::GHC_IE);

        // Initialize implemented ports
        for port_idx in 0..hba.nr_ports {
            let pstate = &mut hba.ports[port_idx];
            pstate.device_id = -1;
            if (pi & (1u32 << port_idx)) != 0 {
                hba.mmio.port_init(port_idx, pstate);
            }
        }

        hba
    }

    /// Clean up HBA resources, including MSI-X per-port interrupts.
    pub fn stop(&mut self) {
        for port_idx in 0..self.nr_ports {
            let state = self.ports[port_idx].state;
            if state != PortState::NoPort {
                self.mmio.port_stop(port_idx);
            }
        }

        self.reset();

        // Clean up MSI-X per-port IRQs if active
        if self.msix_available {
            // Remove per-port IRQ handlers
            for port_idx in 0..self.nr_ports {
                if self.per_port_hook_ids[port_idx] != 0 {
                    let _ = ffi::irq_remove(&mut self.per_port_hook_ids[port_idx]);
                    self.per_port_hook_ids[port_idx] = 0;
                }
                if self.per_port_irqs[port_idx] != 0 {
                    let _ = ffi::msix_free_irq(self.per_port_irqs[port_idx]);
                    self.per_port_irqs[port_idx] = 0;
                }
            }
            // Unmap separate MSI-X table BAR if needed
            if self.msix_separate_bar && !self.msix_table_base.is_null() {
                let _ = ffi::vm_unmap_phys_ffi(
                    self.msix_table_base as *mut core::ffi::c_void,
                    self.msix_table_size_bytes,
                );
            }
            self.msix_available = false;
        } else {
            // Legacy IRQ cleanup
            let _ = ffi::irq_remove(&mut self.hook_id);
        }

        // Unmap the main HBA MMIO region
        let _ = ffi::vm_unmap_phys_ffi(self.mmio.base() as *mut core::ffi::c_void, self.mmio_size);
    }

    /// Log a formatted message at the given verbosity level.
    pub fn log(&self, level: u8, msg: &[u8]) {
        if self.verbose >= level {
            ffi::print(msg);
            ffi::print(b"\n\0");
        }
    }
}
