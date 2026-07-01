//! # AHCI Port Management
//!
//! Per-port state machine: memory allocation, start/stop,
//! command setup (FIS + PRDT), issue, and completion handling.

#![allow(dead_code)]

use crate::ffi;
use crate::registers::{self, port as reg, port_flags, PortState, MAX_CMDS};
use crate::hba::HbaRef;

use core::ptr;

/// Address of a Physical Region Descriptor (for DMA).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Prd {
    pub dba: u32,       // Data Base Address (lower 32 bits)
    pub resv: u32,      // Reserved / upper 32 bits
    pub resv2: u32,     // Reserved
    pub size: u32,      // Byte count (22 bits) — last entry has interrupt flag (bit 31)
}

/// H2D Register FIS (Host to Device).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct H2dFis {
    pub fis_type: u8,     // 0x27
    pub flags: u8,        // PM port + C (update command)
    pub cmd: u8,          // ATA command
    pub feat: u8,         // Features
    pub lba_lo: u8,       // LBA Low
    pub lba_mid: u8,      // LBA Mid
    pub lba_hi: u8,       // LBA High
    pub dev: u8,          // Device (LBA bit)
    pub lba_lo_exp: u8,   // LBA Low (exp)
    pub lba_mid_exp: u8,  // LBA Mid (exp)
    pub lba_hi_exp: u8,   // LBA High (exp)
    pub feat_exp: u8,     // Features (exp)
    pub sec_cnt: u8,      // Sector Count
    pub sec_cnt_exp: u8,  // Sector Count (exp)
    pub resv: u8,         // Reserved
    pub ctl: u8,          // Control
    pub resv2: [u8; 4],   // Padding to 20 bytes
}

/// Command List entry (32 bytes).
#[repr(C, align(128))]
#[derive(Clone, Copy)]
pub struct ClEntry {
    /// DW0: flags (PRDTL, PM port, command type, CFL, attr)
    pub dw0: u32,
    /// DW1: byte count of data to transfer
    pub dw1: u32,
    /// DW2: command table descriptor (lower 32 bits physical address)
    pub ctba: u32,
    /// DW3: command table descriptor (upper 32 bits + reserved)
    pub ctba_u: u32,
    /// Reserved for future use
    pub resv: [u32; 4],
}

/// Command Table (128 bytes header + PRDT).
#[repr(C, align(128))]
pub struct CommandTable {
    /// CFIS: Command FIS (64 bytes, only 20 used)
    pub cfis: [u8; 64],
    /// ATAPI packet (32 bytes)
    pub atapi: [u8; 32],
    /// Reserved
    pub resv: [u8; 32],
    /// PRDT (Physical Region Descriptor Table)
    pub prdt: [Prd; registers::MAX_PRDS],
}

/// Port DMA buffer set (one per port).
#[derive(Clone, Copy)]
pub struct PortBuffers {
    /// Virtual base address of the allocated region.
    pub virt: *mut u8,
    /// Physical base address.
    pub phys: u64,
    /// Total size in bytes.
    pub size: usize,

    /// Command List (1024 bytes, 32 × ClEntry).
    pub cl_virt: *mut u32,
    pub cl_phys: u64,

    /// FIS Receive Area (256 bytes).
    pub fis_virt: *mut u32,
    pub fis_phys: u64,

    /// Temporary buffer (512 bytes, for IDENTIFY data).
    pub tmp_virt: *mut u8,
    pub tmp_phys: u64,

    /// Command Tables (one per command slot).
    pub ct_virt: [*mut u8; MAX_CMDS],
    pub ct_phys: [u64; MAX_CMDS],

    /// Padding buffer (for sector-unaligned transfers).
    pub pad_virt: *mut u8,
    pub pad_phys: u64,
    pub pad_size: usize,
}

impl PortBuffers {
    /// Total memory needed for all buffers (single 4K page).
    pub const TOTAL_SIZE: usize = {
        let cl_sz = registers::CL_SIZE;       // 1024
        let fis_sz = registers::FIS_SIZE;     // 256
        let tmp_sz = registers::TMP_SIZE;     // 512
        let ct_sz = registers::CT_SIZE;       // ~1184
        // Align each section: CL(0), FIS(align256), TMP(align2), CT[n](align128)
        let fis_off = (cl_sz + fis_sz - 1) / fis_sz * fis_sz;
        let tmp_off = fis_off + fis_sz + 2 - 1;
        let tmp_off = tmp_off - (tmp_off % 2);
        let ct_off = tmp_off + tmp_sz;
        ct_off + ct_sz * MAX_CMDS
    };

    /// Allocate and initialize all port DMA buffers.
    pub fn allocate() -> Option<Self> {
        let size = Self::TOTAL_SIZE;
        let (virt, phys) = ffi::alloc_contig_ffi(size)?;

        let virt_u8 = virt as *mut u8;
        let phys_u64 = phys;

        // Zero-initialize
        unsafe { ptr::write_bytes(virt_u8, 0, size) }

        // Calculate sub-buffer offsets (matching C port_alloc layout)
        let fis_off = align_up(registers::CL_SIZE, registers::FIS_SIZE);
        let tmp_off = align_up(fis_off + registers::FIS_SIZE, 2);
        let mut ct_off = align_up(tmp_off + registers::TMP_SIZE, 128);

        let cl_virt = virt_u8 as *mut u32;
        let cl_phys = phys_u64;

        let fis_virt = unsafe { virt_u8.add(fis_off) as *mut u32 };
        let fis_phys = phys_u64 + fis_off as u64;

        let tmp_virt = unsafe { virt_u8.add(tmp_off) as *mut u8 };
        let tmp_phys = phys_u64 + tmp_off as u64;

        let mut ct_virt = [ptr::null_mut(); MAX_CMDS];
        let mut ct_phys = [0u64; MAX_CMDS];

        for i in 0..MAX_CMDS {
            ct_virt[i] = unsafe { virt_u8.add(ct_off) as *mut u8 };
            ct_phys[i] = phys_u64 + ct_off as u64;
            ct_off += registers::CT_SIZE;
            ct_off = align_up(ct_off, 128);
        }

        Some(Self {
            virt: virt_u8,
            phys: phys_u64,
            size,
            cl_virt,
            cl_phys,
            fis_virt,
            fis_phys,
            tmp_virt,
            tmp_phys,
            ct_virt,
            ct_phys,
            pad_virt: ptr::null_mut(),
            pad_phys: 0,
            pad_size: 0,
        })
    }

    /// Ensure a padding buffer of at least `size` bytes is available.
    pub fn ensure_pad(&mut self, size: usize) -> bool {
        if !self.pad_virt.is_null() && self.pad_size >= size {
            return true;
        }
        if !self.pad_virt.is_null() {
            ffi::free_contig_ffi(self.pad_virt as *mut _, self.pad_size);
        }
        let (ptr, phys) = match ffi::alloc_contig_ffi(size) {
            Some(v) => v,
            None => return false,
        };
        self.pad_virt = ptr as *mut u8;
        self.pad_phys = phys;
        self.pad_size = size;
        true
    }

    /// Free all allocated buffers.
    pub fn free(&mut self) {
        if !self.virt.is_null() {
            ffi::free_contig_ffi(self.virt as *mut _, self.size);
            self.virt = ptr::null_mut();
        }
        if !self.pad_virt.is_null() {
            ffi::free_contig_ffi(self.pad_virt as *mut _, self.pad_size);
            self.pad_virt = ptr::null_mut();
        }
    }
}

// Drop is not used because we handle deallocation manually through C's free_contig.
// (Rust drop would conflict with C's memory lifecycle in a no_std driver.)

// ---------------------------------------------------------------------------
// Port State Structure
// ---------------------------------------------------------------------------

/// Per-port runtime state.
#[derive(Clone, Copy)]
pub struct Port {
    /// Current port state machine state.
    pub state: PortState,
    /// Port flags (ATAPI, HAS_MEDIUM, etc.).
    pub flags: u32,
    /// MMIO register pointer for this port.
    pub reg_phys: u64,
    /// DMA buffers.
    pub bufs: PortBuffers,
    /// Number of valid LBAs.
    pub lba_count: u64,
    /// Sector size in bytes.
    pub sector_size: u32,
    /// Open count.
    pub open_count: u32,
    /// Associated device number.
    pub device_id: i32,
    /// NCQ queue depth.
    pub queue_depth: u32,
    /// Pending commands bitmap.
    pub pend_mask: u32,
}

impl Port {
    /// Create a new port in NO_PORT state (must be initialized).
    pub fn new() -> Self {
        Self {
            state: PortState::NoPort,
            flags: 0,
            reg_phys: 0,
            bufs: PortBuffers::allocate().expect("port buffer allocation failed"),
            lba_count: 0,
            sector_size: 0,
            open_count: 0,
            device_id: -1,
            queue_depth: 1,
            pend_mask: 0,
        }
    }

    /// Compute the port register MMIO offset from the HBA base.
    pub fn reg_offset(&self, port_idx: usize) -> u32 {
        (registers::MEM_BASE_SIZE + registers::MEM_PORT_SIZE * port_idx) as u32
    }
}

// ---------------------------------------------------------------------------
// Safe port register access via HbaRef
// ---------------------------------------------------------------------------

impl HbaRef {
    /// Read a port register.
    pub fn port_read32(&self, port_idx: usize, reg_idx: usize) -> u32 {
        let off = registers::MEM_BASE_SIZE + registers::MEM_PORT_SIZE * port_idx
                  + reg_idx * 4;
        // SAFETY: port_idx < nr_ports is guaranteed by caller
        unsafe { ffi::read32_raw(self.base() as usize + off) }
    }

    /// Write a port register.
    pub fn port_write32(&self, port_idx: usize, reg_idx: usize, val: u32) {
        let off = registers::MEM_BASE_SIZE + registers::MEM_PORT_SIZE * port_idx
                  + reg_idx * 4;
        unsafe { ffi::write32_raw(self.base() as usize + off, val) }
    }

    /// Stop a port (clear ST, wait for CR to clear).
    pub fn port_stop(&self, port_idx: usize) {
        let cmd = self.port_read32(port_idx, reg::CMD);
        if cmd & (reg::CMD_CR | reg::CMD_ST) != 0 {
            self.port_write32(port_idx, reg::CMD, cmd & !reg::CMD_ST);
            // Spin until CR clears (max PORTREG_DELAY ms)
            let timeout = 500_000; // 500 ms
            let mut waited = 0u32;
            while (self.port_read32(port_idx, reg::CMD) & reg::CMD_CR) != 0 && waited < timeout {
                ffi::udelay(10);
                waited += 10;
            }
        }
    }

    /// Start a port (set ST after clearing SERR/IS).
    pub fn port_start(&self, port_idx: usize) {
        self.port_write32(port_idx, reg::SERR, !0u32);
        self.port_write32(port_idx, reg::IS, !0u32);
        let cmd = self.port_read32(port_idx, reg::CMD);
        self.port_write32(port_idx, reg::CMD, cmd | reg::CMD_ST);
    }

    /// Perform a hard reset on a port.
    pub fn port_hardreset(&self, port_idx: usize) {
        self.port_write32(port_idx, reg::SCTL, reg::SCTL_DET_INIT);
        ffi::udelay(1_000); // COMRESET_DELAY = 1ms
        self.port_write32(port_idx, reg::SCTL, reg::SCTL_DET_NONE);
    }

    /// Initialize a port (allocate memory, set up registers, trigger reset).
    pub fn port_init(&self, port_idx: usize, port: &mut Port) {
        port.state = PortState::SpinUp;
        port.flags = port_flags::BUSY;
        port.sector_size = 0;
        port.pend_mask = 0;

        // Set up physical addresses in the port registers
        self.port_write32(port_idx, reg::FBU, 0);
        self.port_write32(port_idx, reg::FB, port.bufs.fis_phys as u32);
        self.port_write32(port_idx, reg::CLBU, 0);
        self.port_write32(port_idx, reg::CLB, port.bufs.cl_phys as u32);

        // Enable FIS receive
        let mut cmd = self.port_read32(port_idx, reg::CMD);
        cmd |= reg::CMD_FRE;
        self.port_write32(port_idx, reg::CMD, cmd);

        // Initially listen for connect events only
        self.port_write32(port_idx, reg::IE, reg::IE_PCE);

        // Spin-up device (no-op for HBAs without staggered spin-up)
        cmd = self.port_read32(port_idx, reg::CMD);
        self.port_write32(port_idx, reg::CMD, cmd | reg::CMD_SUD);

        // Trigger port reset
        self.port_hardreset(port_idx);
    }

    /// Issue a command on a port.
    pub fn port_issue_cmd(&self, port_idx: usize, cmd_tag: u32) {
        // Note: SACT is set by the caller (HbaController) when NCQ is enabled.
        // Here we just write CI to trigger command issue.
        self.port_write32(port_idx, reg::CI, 1 << cmd_tag);
    }

    /// Check if a port has physical device presence (PHY established).
    pub fn port_has_device(&self, port_idx: usize) -> bool {
        let ssts = self.port_read32(port_idx, reg::SSTS);
        (ssts & reg::SSTS_DET_MASK) == reg::SSTS_DET_PHY
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Align a value up to the given alignment.
pub const fn align_up(val: usize, align: usize) -> usize {
    (val + align - 1) / align * align
}
