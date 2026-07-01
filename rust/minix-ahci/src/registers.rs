//! # AHCI 1.3 Register Definitions
//!
//! Complete register offset and bitfield constants for the
//! Advanced Host Controller Interface (AHCI) specification v1.3.
//!
//! These correspond to the C constants in `minix/drivers/storage/ahci/ahci.h`.

#![allow(dead_code)]

// ============================================================================
// HBA (Host Bus Adapter) Registers — memory-mapped at BAR5/6
// ============================================================================

/// HBA register offsets (each is a u32 index, not byte offset).
pub mod hba {
    pub const CAP: usize = 0;   // Host Capabilities
    pub const GHC: usize = 1;  // Global Host Control
    pub const IS: usize = 2;   // Interrupt Status
    pub const PI: usize = 3;   // Ports Implemented
    pub const VS: usize = 4;   // Version
    pub const CAP2: usize = 9; // Host Capabilities Extended

    // --- CAP (Host Capabilities) bitfields ---
    pub const CAP_SNCQ: u32 = 1 << 30;    // Native Command Queuing
    pub const CAP_SCLO: u32 = 1 << 24;    // Command List Override
    pub const CAP_NCS_SHIFT: u32 = 8;     // Number of Command Slots
    pub const CAP_NCS_MASK: u32 = 0x1F;
    pub const CAP_NP_SHIFT: u32 = 0;      // Number of Ports
    pub const CAP_NP_MASK: u32 = 0x1F;

    // --- GHC (Global Host Control) bitfields ---
    pub const GHC_AE: u32 = 1 << 31;      // AHCI Enable
    pub const GHC_IE: u32 = 1 << 1;       // Interrupt Enable
    pub const GHC_HR: u32 = 1 << 0;       // HBA Reset
}

// ============================================================================
// Port Registers — per-port, each port occupies 0x80 bytes
// ============================================================================

/// Port register offsets (each is a u32 index within the port's MMIO region).
pub mod port {
    pub const CLB: usize = 0;    // Command List Base (lower 32 bits)
    pub const CLBU: usize = 1;   // Command List Base (upper 32 bits)
    pub const FB: usize = 2;     // FIS Base (lower 32 bits)
    pub const FBU: usize = 3;    // FIS Base (upper 32 bits)
    pub const IS: usize = 4;     // Interrupt Status
    pub const IE: usize = 5;     // Interrupt Enable
    pub const CMD: usize = 6;    // Command and Status
    pub const TFD: usize = 8;    // Task File Data
    pub const SIG: usize = 9;    // Signature
    pub const SSTS: usize = 10;  // Serial ATA Status
    pub const SCTL: usize = 11;  // Serial ATA Control
    pub const SERR: usize = 12;  // Serial ATA Error
    pub const SACT: usize = 13;  // Serial ATA Active
    pub const CI: usize = 14;    // Command Issue

    // --- IS (Interrupt Status) bitfields ---
    pub const IS_TFES: u32 = 1 << 30;  // Task File Error
    pub const IS_HBFS: u32 = 1 << 29;  // Host Bus Fatal
    pub const IS_HBDS: u32 = 1 << 28;  // Host Bus Data
    pub const IS_IFS: u32 = 1 << 27;   // Interface Fatal
    pub const IS_PRCS: u32 = 1 << 22;  // PhyRdy Change
    pub const IS_PCS: u32 = 1 << 6;    // Port Connect Change
    pub const IS_SDBS: u32 = 1 << 3;   // Set Dev Bits FIS
    pub const IS_PSS: u32 = 1 << 1;    // PIO Setup FIS
    pub const IS_DHRS: u32 = 1 << 0;   // D2H Register FIS

    pub const IS_RESTART: u32 =
        IS_TFES | IS_HBFS | IS_HBDS | IS_IFS;

    pub const IS_MASK: u32 =
        IS_RESTART | IS_PRCS | IS_DHRS | IS_PSS | IS_SDBS;

    // --- IE (Interrupt Enable) bitfields ---
    pub const IE_MASK: u32 = IS_MASK;
    pub const IE_PRCE: u32 = IS_PRCS;
    pub const IE_PCE: u32 = IS_PCS;
    pub const IE_NONE: u32 = 0;

    // --- CMD (Command and Status) bitfields ---
    pub const CMD_CR: u32 = 1 << 15;   // Command List Running
    pub const CMD_FR: u32 = 1 << 14;   // FIS Receive Running
    pub const CMD_FRE: u32 = 1 << 4;   // FIS Receive Enabled
    pub const CMD_CLO: u32 = 1 << 3;   // Command List Override
    pub const CMD_SUD: u32 = 1 << 1;   // Spin-Up Device
    pub const CMD_ST: u32 = 1 << 0;    // Start

    // --- TFD (Task File Data) bitfields ---
    pub const TFD_BSY: u32 = 1 << 7;   // Busy
    pub const TFD_DF: u32 = 1 << 5;    // Device Fault
    pub const TFD_DRQ: u32 = 1 << 3;   // Data Transfer Requested
    pub const TFD_ERR: u32 = 1 << 0;   // Error
    pub const TFD_INIT: u32 = 0x7F;    // Initial state mask

    // --- SIG (Signature) values ---
    pub const SIG_ATA: u32 = 0x0000_0101;    // ATA drive
    pub const SIG_ATAPI: u32 = 0xEB14_0101;  // ATAPI drive

    // --- SSTS (Serial ATA Status) bitfields ---
    pub const SSTS_DET_MASK: u32 = 0x0000_0007;
    pub const SSTS_DET_NONE: u32 = 0x0000_0000; // No device detected
    pub const SSTS_DET_DET: u32 = 0x0000_0001;  // Device detected, PHY not ready
    pub const SSTS_DET_PHY: u32 = 0x0000_0003;  // PHY communication established

    // --- SCTL (Serial ATA Control) bitfields ---
    pub const SCTL_DET_INIT: u32 = 0x0000_0001; // Perform interface initialization
    pub const SCTL_DET_NONE: u32 = 0x0000_0000; // No action requested

    // --- SERR (Serial ATA Error) bitfields ---
    pub const SERR_DIAG_X: u32 = 1 << 26;  // Exchanged
    pub const SERR_DIAG_N: u32 = 1 << 16;  // PhyRdy Change

    // --- SACT / CI ---
    // These are bitmaps indexed by command tag.
}

// ============================================================================
// ATA FIS (Frame Information Structure) definitions
// ============================================================================

pub mod fis {
    // Generic FIS types
    pub const TYPE_H2D: u8 = 0x27;  // Register — Host to Device

    // Register H2D FIS byte offsets
    pub const H2D_SIZE: usize = 20;
    pub const H2D_FLAGS: usize = 1;
    pub const H2D_FLAGS_C: u8 = 0x80;   // Update command register
    pub const H2D_CMD: usize = 2;
    pub const H2D_FEAT: usize = 3;
    pub const H2D_LBA_LOW: usize = 4;
    pub const H2D_LBA_MID: usize = 5;
    pub const H2D_LBA_HIGH: usize = 6;
    pub const H2D_DEV: usize = 7;
    pub const H2D_LBA_LOW_EXP: usize = 8;
    pub const H2D_LBA_MID_EXP: usize = 9;
    pub const H2D_LBA_HIGH_EXP: usize = 10;
    pub const H2D_FEAT_EXP: usize = 11;
    pub const H2D_SEC: usize = 12;
    pub const H2D_SEC_EXP: usize = 13;
    pub const H2D_CTL: usize = 15;

    pub const DEV_LBA: u8 = 0x40;   // Use LBA addressing
    pub const DEV_FUA: u8 = 0x80;   // Force Unit Access (FPDMA)
}

// ============================================================================
// ATA Commands
// ============================================================================

pub mod ata_cmd {
    pub const READ_DMA_EXT: u8 = 0x25;
    pub const WRITE_DMA_EXT: u8 = 0x35;
    pub const READ_FPDMA_QUEUED: u8 = 0x60;
    pub const WRITE_FPDMA_QUEUED: u8 = 0x61;
    pub const WRITE_DMA_FUA_EXT: u8 = 0x3D;
    pub const PACKET: u8 = 0xA0;
    pub const IDENTIFY_PACKET: u8 = 0xA1;
    pub const FLUSH_CACHE: u8 = 0xE7;
    pub const IDENTIFY: u8 = 0xEC;
    pub const SET_FEATURES: u8 = 0xEF;
}

// ============================================================================
// ATA IDENTIFY data word offsets
// ============================================================================

pub mod ata_id {
    pub const GCAP: usize = 0;
    pub const GCAP_ATAPI: u16 = 0x8000;
    pub const GCAP_ATA: u16 = 0x0000;
    pub const GCAP_REMOVABLE: u16 = 0x0080;
    pub const GCAP_INCOMPLETE: u16 = 0x0004;

    pub const CAP: usize = 49;
    pub const CAP_DMA: u16 = 0x0100;
    pub const CAP_LBA: u16 = 0x0200;

    pub const DMADIR: usize = 62;
    pub const DMADIR_DMADIR: u16 = 0x8000;
    pub const DMADIR_DMA: u16 = 0x0400;

    pub const QDEPTH: usize = 75;
    pub const QDEPTH_MASK: u16 = 0x000F;

    pub const SATA_CAP: usize = 76;
    pub const SATA_CAP_NCQ: u16 = 0x0100;

    pub const SUP0: usize = 82;
    pub const SUP0_WCACHE: u16 = 0x0020;

    pub const SUP1: usize = 83;
    pub const SUP1_VALID_MASK: u16 = 0xC000;
    pub const SUP1_VALID: u16 = 0x4000;
    pub const SUP1_FLUSH: u16 = 0x1000;
    pub const SUP1_LBA48: u16 = 0x0400;

    pub const ENA0: usize = 85;
    pub const ENA0_WCACHE: u16 = 0x0020;

    pub const ENA2: usize = 87;
    pub const ENA2_VALID_MASK: u16 = 0xC000;
    pub const ENA2_VALID: u16 = 0x4000;
    pub const ENA2_FUA: u16 = 0x0040;

    pub const LBA0: usize = 100;
    pub const LBA1: usize = 101;
    pub const LBA2: usize = 102;
    pub const LBA3: usize = 103;

    pub const PLSS: usize = 106;
    pub const PLSS_VALID_MASK: u16 = 0xC000;
    pub const PLSS_VALID: u16 = 0x4000;
    pub const PLSS_LLS: u16 = 0x1000;

    pub const LSS0: usize = 118;
    pub const LSS1: usize = 119;
}

// ============================================================================
// AHCI memory layout constants
// ============================================================================

/// Size of the HBA MMIO base region (before port registers).
pub const MEM_BASE_SIZE: usize = 0x100;

/// Size of each port's MMIO region.
pub const MEM_PORT_SIZE: usize = 0x80;

/// Maximum number of ports.
pub const MAX_PORTS: usize = 32;

/// Maximum number of queued commands per port.
pub const MAX_CMDS: usize = 32;

/// Number of PRD entries per command table.
pub const MAX_PRDS: usize = 66; // NR_IOREQS + 2 = 64 + 2

/// Size constants for DMA buffers.
pub const FIS_SIZE: usize = 256;
pub const CL_SIZE: usize = 1024;     // Command List: 32 entries × 32 bytes
pub const TMP_SIZE: usize = 512;     // Temp buffer: ATAPI IDENTIFY result
pub const CT_SIZE: usize = 128 + MAX_PRDS * 16; // Command Table

/// ATA sector size constants.
pub const ATA_SECTOR_SIZE: usize = 512;

/// IDENTIFY data size in bytes (256 words × 2 bytes).
pub const ATA_ID_SIZE: usize = 512;

/// ATA maximum sectors per transfer.
pub const ATA_MAX_SECTORS: u32 = 0x0001_0000;

/// Maximum bytes per PRD entry (4MB per AHCI spec).
pub const MAX_PRD_BYTES: u32 = 1 << 22;

/// Maximum transfer size (4MB).
pub const MAX_TRANSFER: u32 = MAX_PRD_BYTES;

/// Is this an FPDMA (NCQ) command?
pub fn is_fpdma_cmd(cmd: u8) -> bool {
    cmd == ata_cmd::READ_FPDMA_QUEUED || cmd == ata_cmd::WRITE_FPDMA_QUEUED
}

// ============================================================================
// Port state machine
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
    /// Port not physically present.
    NoPort,
    /// Waiting for initial device spin-up.
    SpinUp,
    /// No device detected.
    NoDev,
    /// Waiting for device to become ready.
    WaitDev,
    /// Waiting for device identification.
    WaitId,
    /// Device identified but unusable.
    BadDev,
    /// Device is usable and operational.
    GoodDev,
}

impl PortState {
    pub fn is_device_present(self) -> bool {
        matches!(self, Self::WaitId | Self::GoodDev | Self::BadDev)
    }

    pub fn is_operational(self) -> bool {
        matches!(self, Self::GoodDev)
    }
}

// ============================================================================
// Port flags
// ============================================================================

/// Port flags (replaces bitflags for no_std compatibility).
pub mod port_flags {
    pub const ATAPI: u32      = 0x0000_0001;
    pub const HAS_MEDIUM: u32 = 0x0000_0002;
    pub const USE_DMADIR: u32 = 0x0000_0004;
    pub const READONLY: u32   = 0x0000_0008;
    pub const BUSY: u32       = 0x0000_0010;
    pub const FAILURE: u32    = 0x0000_0020;
    pub const BARRIER: u32    = 0x0000_0040;
    pub const HAS_WCACHE: u32 = 0x0000_0080;
    pub const HAS_FLUSH: u32  = 0x0000_0100;
    pub const SUSPENDED: u32  = 0x0000_0200;
    pub const HAS_FUA: u32    = 0x0000_0400;
    pub const HAS_NCQ: u32    = 0x0000_0800;
    pub const NCQ_MODE: u32   = 0x0000_1000;
}

// ============================================================================
// Verbosity levels
// ============================================================================

pub mod verbose {
    pub const NONE: u8 = 0;
    pub const ERR: u8 = 1;
    pub const INFO: u8 = 2;
    pub const DEV: u8 = 3;
    pub const REQ: u8 = 4;
}
