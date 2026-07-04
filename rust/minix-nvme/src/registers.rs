//! # Registers — NVMe Controller Register Definitions
//!
//! From NVM Express Base Specification Revision 1.4.
//! All register offsets are relative to BAR0 (MMIO base).

#![allow(dead_code)]

use core::ffi::c_int;

// ============================================================================
// PCI Class Code
// ============================================================================

/// NVMe PCI class: mass storage controller (0x01), NVM (0x08), NVMe (0x02).
pub const PCI_CLASS_STORAGE: u32 = 0x01;
pub const PCI_SUBCLASS_NVM: u32 = 0x08;
pub const PCI_PROGIF_NVME: u32 = 0x02;

// ============================================================================
// Controller Register Offsets (BAR0, byte offsets)
// ============================================================================

pub mod regs {
    /// CAP — Controller Capabilities (64-bit)
    pub const CAP: usize = 0x00;
    /// VS — Version (32-bit)
    pub const VS: usize = 0x08;
    /// INTMS — Interrupt Mask Set (32-bit)
    pub const INTMS: usize = 0x0C;
    /// INTMC — Interrupt Mask Clear (32-bit)
    pub const INTMC: usize = 0x10;
    /// CC — Controller Configuration (32-bit)
    pub const CC: usize = 0x14;
    /// CSTS — Controller Status (32-bit)
    pub const CSTS: usize = 0x1C;
    /// AQA — Admin Queue Attributes (32-bit)
    pub const AQA: usize = 0x24;
    /// ASQ — Admin Submission Queue Base Address (64-bit)
    pub const ASQ: usize = 0x28;
    /// ACQ — Admin Completion Queue Base Address (64-bit)
    pub const ACQ: usize = 0x30;
    /// Doorbell stride shift (doorbell_size = 4 << (CAP.DSTRD))
    pub const DOORBELL_STRIDE_DEFAULT: usize = 2; // 4 bytes per doorbell

    /// First doorbell offset (SQ0 Tail Doorbell)
    pub const DOORBELL_BASE: usize = 0x1000;

    /// CAP register bits.
    pub mod cap {
        pub const MPSMIN_SHIFT: u32 = 0;
        pub const MPSMIN_MASK: u32 = 0xF;
        pub const MPSMAX_SHIFT: u32 = 4;
        pub const MPSMAX_MASK: u32 = 0xF;
        pub const CSS_SHIFT: u32 = 37;
        pub const CSS_NVME: u64 = 1 << 37;
        pub const NSSRS: u64 = 1 << 33;
        pub const DSTRD_SHIFT: u32 = 32;
        pub const DSTRD_MASK: u64 = 0xF;
        pub const TO_SHIFT: u32 = 24;
        pub const TO_MASK: u32 = 0xFF;
        pub const AMS_SHIFT: u32 = 17;
        pub const AMS_WRR: u32 = 1 << 17;
        pub const AMS_MASK: u32 = 0x3;
        pub const CQR: u64 = 1 << 16;
        pub const MQES_SHIFT: u32 = 0;
        pub const MQES_MASK: u64 = 0xFFFF;
    }

    /// CC register bits.
    pub mod cc {
        pub const ENABLE: u32 = 1 << 0;
        pub const CSS_SHIFT: u32 = 4;
        pub const CSS_MASK: u32 = 0x7;
        pub const CSS_NVME: u32 = 0;
        pub const SHN_SHIFT: u32 = 14;
        pub const SHN_MASK: u32 = 0x3;
        pub const SHN_NORMAL: u32 = 1;
        pub const SHN_ABRUPT: u32 = 2;
        pub const IOSQES_SHIFT: u32 = 16;
        pub const IOSQES_MASK: u32 = 0xF;
        pub const IOCQES_SHIFT: u32 = 20;
        pub const IOCQES_MASK: u32 = 0xF;
        pub const MPS_SHIFT: u32 = 7;
        pub const MPS_MASK: u32 = 0xF;
        pub const AMS_SHIFT: u32 = 11;
        pub const AMS_MASK: u32 = 0x7;
    }

    /// CSTS register bits.
    pub mod csts {
        pub const RDY: u32 = 1 << 0;
        pub const CFS: u32 = 1 << 1;
        pub const SHST_SHIFT: u32 = 2;
        pub const SHST_MASK: u32 = 0x3;
        pub const SHST_NORMAL: u32 = 0;
        pub const SHST_OCCURRING: u32 = 1;
        pub const SHST_COMPLETE: u32 = 2;
        pub const NSSRO: u32 = 1 << 4;
    }
}

// ============================================================================
// CAP register bits (DEPRECATED — use regs::cap instead)
// ============================================================================

pub mod cap {
    pub use super::regs::cap::*;
}

// ============================================================================
// CC register bits (DEPRECATED — use regs::cc instead)
// ============================================================================

pub mod cc {
    pub use super::regs::cc::*;
}

// ============================================================================
// CSTS register bits (DEPRECATED — use regs::csts instead)
// ============================================================================

pub mod csts {
    pub use super::regs::csts::*;
}



// ============================================================================
// Command opcodes
// ============================================================================

pub mod opcode {
    // Admin commands
    pub const DELETE_IO_SQ: u8 = 0x00;
    pub const CREATE_IO_SQ: u8 = 0x01;
    pub const GET_LOG_PAGE: u8 = 0x02;
    pub const DELETE_IO_CQ: u8 = 0x04;
    pub const CREATE_IO_CQ: u8 = 0x05;
    pub const IDENTIFY: u8 = 0x06;
    pub const ABORT: u8 = 0x08;
    pub const SET_FEATURES: u8 = 0x09;
    pub const GET_FEATURES: u8 = 0x0A;
    pub const ASYNCHRONOUS_EVENT_REQUEST: u8 = 0x0C;
    pub const NAMESPACE_MANAGEMENT: u8 = 0x0D;

    // NVM commands
    pub const FLUSH: u8 = 0x00;
    pub const WRITE: u8 = 0x01;
    pub const READ: u8 = 0x02;
    pub const WRITE_UNCORRECTABLE: u8 = 0x04;
    pub const COMPARE: u8 = 0x05;
    pub const WRITE_ZEROES: u8 = 0x08;
    pub const DATASET_MANAGEMENT: u8 = 0x09;
}

// ============================================================================
// Identify CNS (Controller or Namespace Structure) values
// ============================================================================

pub mod cns {
    pub const IDENTIFY_NAMESPACE: u32 = 0x00;
    pub const IDENTIFY_CONTROLLER: u32 = 0x01;
    pub const IDENTIFY_ACTIVE_NS_LIST: u32 = 0x02;
    pub const IDENTIFY_NS_ID_DESCRIPTOR: u32 = 0x03;
}

// ============================================================================
// Feature identifiers (for Set/Get Features)
// ============================================================================

pub mod feature {
    pub const NUMBER_OF_QUEUES: u8 = 0x07;
    pub const INTERRUPT_COALESCING: u8 = 0x08;
    pub const INTERRUPT_VECTOR_CONFIG: u8 = 0x09;
    pub const ASYNC_EVENT_CONFIG: u8 = 0x0B;
    pub const AUTONOMOUS_POWER_STATE_TRANSITION: u8 = 0x0C;
    pub const HOST_MEMORY_BUFFER: u8 = 0x0D;
    pub const KEEP_ALIVE_TIMER: u8 = 0x0F;
}

// ============================================================================
// Status codes (Value field in Completion Queue entry)
// ============================================================================

pub mod status {
    pub const SUCCESS: u16 = 0x0000;
    pub const INVALID_OPCODE: u16 = 0x0001;
    pub const INVALID_FIELD: u16 = 0x0002;
    pub const DATA_TRANSFER_ERROR: u16 = 0x0004;
    pub const ABORTED: u16 = 0x0007;
    pub const INVALID_NSID: u16 = 0x000B;
    pub const LBA_RANGE: u16 = 0x0080;
    pub const CAPACITY_EXCEEDED: u16 = 0x0081;
    pub const NS_NOT_READY: u16 = 0x0082;
    pub const CQ_INVALID: u16 = 0x0100;
    pub const SQ_INVALID: u16 = 0x0101;
    pub const INTERNAL_DEVICE_ERROR: u16 = 0x010B;

    pub const DNR: u16 = 0x4000;  // Do Not Retry
    pub const MORE: u16 = 0x8000; // More
    pub const SCT_MASK: u16 = 0x7800; // Status Code Type
    pub const SC_MASK: u16 = 0x03FF;  // Status Code

    /// Check if a completion status indicates success.
    pub fn is_success(sc: u16) -> bool {
        (sc & SC_MASK) == SUCCESS
    }

    /// Check if a completion status indicates an error.
    pub fn is_error(sc: u16) -> bool {
        (sc & SC_MASK) != SUCCESS
    }

    /// Get the status code type.
    pub fn sct(sc: u16) -> u16 {
        (sc & SCT_MASK) >> 11
    }
}

// ============================================================================
// PCI Power Management capability (cap_id = 0x01)
// ============================================================================

/// PCI PM capability ID.
pub const PCI_PM_CAP_ID: u8 = 0x01;

/// PCI PM capability: PMC (Power Management Capabilities) at offset +2.
pub mod pmc {
    /// Version field mask (bits 2:0).
    pub const VERSION_MASK: u16 = 0x0007;
    /// PME from D0 supported.
    pub const PME_D0: u16 = 1 << 15;
    /// PME from D1 supported.
    pub const PME_D1: u16 = 1 << 14;
    /// PME from D2 supported.
    pub const PME_D2: u16 = 1 << 13;
    /// PME from D3hot supported.
    pub const PME_D3HOT: u16 = 1 << 12;
    /// D1 support.
    pub const D1_SUPPORT: u16 = 1 << 9;
    /// D2 support.
    pub const D2_SUPPORT: u16 = 1 << 10;
    /// Immediate ready on return to D0.
    pub const IMMEDIATE_READY: u16 = 1 << 4;
}

/// PCI PM capability: PMCSR (Power Management Control/Status) at offset +4.
pub mod pmcsr {
    /// PowerState mask (bits 1:0).
    pub const POWER_STATE_MASK: u16 = 0x0003;
    /// D0 — fully on.
    pub const D0: u16 = 0x0000;
    /// D1 — light sleep.
    pub const D1: u16 = 0x0001;
    /// D2 — deep sleep.
    pub const D2: u16 = 0x0002;
    /// D3hot — warm sleep (aux power available).
    pub const D3HOT: u16 = 0x0003;
    /// NoSoftReset (bit 3).
    pub const NO_SOFT_RESET: u16 = 1 << 3;
    /// PME_Status (bit 8) — write 1 to clear.
    pub const PME_STATUS: u16 = 1 << 8;
    /// PME_En (bit 15) — enable PME signaling.
    pub const PME_EN: u16 = 1 << 15;
    /// Data_Scale mask (bits 12:9).
    pub const DATA_SCALE_MASK: u16 = 0xF << 9;
    /// Data_Select mask (bits 14:13).
    pub const DATA_SELECT_MASK: u16 = 0x3 << 13;
}

// ============================================================================
// Log Page Identifiers (LID) for Get Log Page command
// ============================================================================

pub mod log_page {
    /// Error Information Log (LID=0x01) — up to `elpe` entries, 64 bytes each.
    pub const ERROR_INFO: u8 = 0x01;
    /// SMART / Health Information (LID=0x02) — 512 bytes.
    pub const SMART_HEALTH: u8 = 0x02;
    /// Firmware Slot Information (LID=0x03).
    pub const FIRMWARE_SLOT: u8 = 0x03;
    /// Changed Namespace List (LID=0x04).
    pub const CHANGED_NS: u8 = 0x04;
    /// Commands Supported and Effects (LID=0x05).
    pub const CMD_EFFECTS: u8 = 0x05;
    /// Device Self-Test (LID=0x06).
    pub const DEVICE_SELF_TEST: u8 = 0x06;
    /// Telemetry Host-Initiated (LID=0x07).
    pub const TELEMETRY_HOST: u8 = 0x07;
    /// Telemetry Controller-Initiated (LID=0x08).
    pub const TELEMETRY_CTRL: u8 = 0x08;
    /// Endurance Group Information (LID=0x09).
    pub const ENDURANCE: u8 = 0x09;
    /// Predictable Latency Per NVM Set (LID=0x0A).
    pub const PREDICTABLE_LATENCY: u8 = 0x0A;
    /// Persistent Event Log (LID=0x0D).
    pub const PERSISTENT_EVENT: u8 = 0x0D;
    /// LPA bit: Error Information log supported.
    pub const LPA_ERROR_LOG: u8 = 0x01;
    /// LPA bit: SMART / Health log supported.
    pub const LPA_SMART_HEALTH: u8 = 0x02;
}

/// Error Information Log entry (64 bytes per entry).
/// The log contains up to `elpe` (Error Log Page Entries) entries.
/// Uses `repr(C, packed)` to match NVMe spec byte-exact layout.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct ErrorLogEntry {
    /// Error Count (64-bit).
    pub error_count: u64,
    /// Submission Queue ID.
    pub sqid: u16,
    /// Command ID.
    pub cid: u16,
    /// Status Field (SF = Status Code Type | Status Code).
    pub status_field: u16,
    /// Reserved (bytes 14-15).
    _reserved_14: [u8; 2],
    /// Error Location (LBA of the error).
    pub error_location_lba: u64,
    /// Namespace ID (NSID).
    pub nsid: u32,
    /// Vendor Specific Information (Lower Dword, bytes 28-31).
    pub vendor_specific_lo: u32,
    /// Vendor Specific Information (Upper Dword, bytes 32-35).
    pub vendor_specific_hi: u32,
    /// Reserved (bytes 36-63).
    pub reserved: [u8; 28],
}

impl ErrorLogEntry {
    /// Get the Status Code Type (3 bits).
    pub fn sct(&self) -> u8 {
        ((self.status_field >> 11) & 0x7) as u8
    }

    /// Get the Status Code (lower 8 bits of status field).
    pub fn sc(&self) -> u8 {
        (self.status_field & 0xFF) as u8
    }

    /// Check if this entry is valid (error_count != 0).
    pub fn is_valid(&self) -> bool {
        self.error_count != 0
    }
}

/// SMART / Health Information log (512 bytes, LID=0x02).
/// Uses `repr(C, packed)` to match NVMe spec byte-exact layout.
#[repr(C, packed)]
pub struct SmartHealth {
    /// Critical Warning (bits: 0=spare, 1=temp, 2=reliability, 3=media, 4=volatile).
    pub critical_warning: u8,
    /// Composite Temperature (Kelvin).
    pub temperature: u16,
    /// Available Spare (percentage).
    pub available_spare: u8,
    /// Available Spare Threshold (percentage).
    pub available_spare_threshold: u8,
    /// Percentage Used.
    pub percentage_used: u8,
    /// Endurance Group Critical Warning Summary (NVMe 1.4+).
    pub eg_critical_warning: u8,
    /// Data Units Read (in 512-byte units).
    pub data_units_read: [u8; 16],  // 128-bit value
    /// Data Units Written (in 512-byte units).
    pub data_units_written: [u8; 16],
    /// Host Read Commands.
    pub host_read_commands: [u8; 16],
    /// Host Write Commands.
    pub host_write_commands: [u8; 16],
    /// Controller Busy Time (in minutes).
    pub controller_busy_time: [u8; 16],
    /// Power Cycles.
    pub power_cycles: [u8; 16],
    /// Power On Hours.
    pub power_on_hours: [u8; 16],
    /// Unsafe Shutdowns.
    pub unsafe_shutdowns: [u8; 16],
    /// Media Errors (uncorrectable).
    pub media_errors: [u8; 16],
    /// Number of Error Information Log Entries.
    pub num_error_log_entries: [u8; 16],
    /// Warning Composite Temperature Time (minutes).
    pub warning_temp_time: u32,
    /// Critical Composite Temperature Time (minutes).
    pub critical_temp_time: u32,
    /// Temperature Sensor 1..8 (Kelvin).
    pub temp_sensor: [u16; 8],
    /// Thermal Management Temperature 1..2.
    pub thermal_temp: [u16; 2],
    /// Reserved (padding to 512 bytes).
    pub reserved: [u8; 317],
}

impl SmartHealth {
    /// Check if any critical warning is active.
    pub fn has_critical_warning(&self) -> bool {
        self.critical_warning != 0
    }

    /// Get critical warning flags.
    pub fn critical_warning_flags(&self) -> u8 {
        self.critical_warning
    }

    /// Temperature in Celsius.
    pub fn temperature_celsius(&self) -> i32 {
        // Convert from Kelvin to Celsius
        if self.temperature > 273 {
            (self.temperature as i32) - 273
        } else {
            self.temperature as i32
        }
    }

    /// Read a 128-bit value as u64 lower bits (for simple stats).
    fn read_uint64(data: &[u8; 16]) -> u64 {
        let mut arr = [0u8; 8];
        arr.copy_from_slice(&data[..8]);
        u64::from_le_bytes(arr)
    }

    /// Data Units Read (lower 64 bits).
    pub fn data_units_read_raw(&self) -> u64 {
        Self::read_uint64(&self.data_units_read)
    }

    /// Data Units Written (lower 64 bits).
    pub fn data_units_written_raw(&self) -> u64 {
        Self::read_uint64(&self.data_units_written)
    }

    /// Power On Hours (lower 64 bits).
    pub fn power_on_hours_raw(&self) -> u64 {
        Self::read_uint64(&self.power_on_hours)
    }

    /// Unsafe Shutdowns (lower 64 bits).
    pub fn unsafe_shutdowns_raw(&self) -> u64 {
        Self::read_uint64(&self.unsafe_shutdowns)
    }

    /// Media Errors (lower 64 bits).
    pub fn media_errors_raw(&self) -> u64 {
        Self::read_uint64(&self.media_errors)
    }

    /// Number of Error Log Entries (lower 64 bits).
    pub fn num_error_log_entries_raw(&self) -> u64 {
        Self::read_uint64(&self.num_error_log_entries)
    }
}

// ============================================================================
// Queue identifiers
// ============================================================================

/// Admin Submission Queue ID (always 0).
pub const ADMIN_SQ_ID: u16 = 0;
/// Admin Completion Queue ID (always 0).
pub const ADMIN_CQ_ID: u16 = 0;
/// First I/O queue ID.
pub const IO_QID_BASE: u16 = 1;

// ============================================================================
// Data structures
// ============================================================================

/// Submission Queue entry (64 bytes = 16 dwords).
/// Used for both admin and I/O commands.
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct SqEntry {
    pub dword: [u32; 16],
}

impl SqEntry {
    pub const fn zeroed() -> Self {
        Self { dword: [0u32; 16] }
    }

    /// Set command opcode and fuse flags.
    pub fn set_cmd(&mut self, opc: u8, fuse: u8) {
        self.dword[0] = (self.dword[0] & 0xFFFF0000) |
            ((opc as u32) << 8) | ((fuse as u32) << 0);
    }

    /// Set namespace identifier (NSID).
    pub fn set_nsid(&mut self, nsid: u32) {
        self.dword[1] = nsid;
    }

    /// Set physical region page entry (PRP1) — lower 64-bit address.
    pub fn set_prp1(&mut self, addr: u64) {
        self.dword[6] = addr as u32;
        self.dword[7] = (addr >> 32) as u32;
    }

    /// Set physical region page entry (PRP2) — upper 64-bit address.
    pub fn set_prp2(&mut self, addr: u64) {
        self.dword[8] = addr as u32;
        self.dword[9] = (addr >> 32) as u32;
    }

    /// Set command dword 10 (CDW10) — e.g. starting LBA, number of LBAs, etc.
    pub fn set_cdw10(&mut self, val: u32) {
        self.dword[10] = val;
    }

    /// Set command dword 11 (CDW11).
    pub fn set_cdw11(&mut self, val: u32) {
        self.dword[11] = val;
    }

    /// Set command dword 12 (CDW12).
    pub fn set_cdw12(&mut self, val: u32) {
        self.dword[12] = val;
    }

    /// Set command dword 13 (CDW13).
    pub fn set_cdw13(&mut self, val: u32) {
        self.dword[13] = val;
    }

    /// Set command dword 14 (CDW14).
    pub fn set_cdw14(&mut self, val: u32) {
        self.dword[14] = val;
    }

    /// Set command dword 15 (CDW15).
    pub fn set_cdw15(&mut self, val: u32) {
        self.dword[15] = val;
    }
}

/// Completion Queue entry (16 bytes = 4 dwords).
#[repr(C, align(4))]
#[derive(Clone, Copy)]
pub struct CqEntry {
    pub dword: [u32; 4],
}

impl CqEntry {
    /// Get the command-specific status field.
    pub fn status(&self) -> u16 {
        (self.dword[3] >> 16) as u16
    }

    /// Get the Submission Queue Head Pointer (SQHD).
    pub fn sq_head(&self) -> u16 {
        self.dword[2] as u16
    }

    /// Get the command identifier (CID).
    pub fn cid(&self) -> u16 {
        (self.dword[3] & 0xFFFF) as u16
    }

    /// Get the phase tag (P) — indicates whether this is a new completion.
    pub fn phase(&self) -> bool {
        (self.dword[3] & 0x1) != 0
    }

    /// Check if command completed with success.
    pub fn is_success(&self) -> bool {
        status::is_success(self.status())
    }
}

/// Admin Completion Queue entry (same layout, but with Admin-specific fields).
pub type AdminCqEntry = CqEntry;

// ============================================================================
// Identify data structures
// ============================================================================

/// Controller Identify data structure (4096 bytes).
#[repr(C, align(4096))]
pub struct IdentifyController {
    /// PCI Vendor ID (VID)
    pub vid: u16,
    /// PCI Subsystem Vendor ID (SSVID)
    pub ssvid: u16,
    /// Serial number (20 bytes)
    pub sn: [u8; 20],
    /// Model number (40 bytes)
    pub mn: [u8; 40],
    /// Firmware revision (8 bytes)
    pub fr: [u8; 8],
    /// Recommended Arbitration Burst (RAB)
    pub rab: u8,
    /// IEEE OUI Identifier (3 bytes)
    pub ieee: [u8; 3],
    /// Controller Multi-Path I/O and Namespace Sharing Capabilities (CMIC)
    pub cmic: u8,
    /// Maximum Data Transfer Size (MDTS)
    pub mdts: u8,
    /// Controller ID (CNTLID)
    pub cntlid: u16,
    /// Version (VER)
    pub ver: u32,
    /// RTD3 Resume Latency (RTD3R)
    pub rtd3r: u32,
    /// RTD3 Entry Latency (RTD3E)
    pub rtd3e: u32,
    /// Optional Asynchronous Events Supported (OAES)
    pub oaes: u32,
    /// Controller Attributes (CTRATT)
    pub ctratt: u32,
    /// Reserved
    pub reserved_0: [u8; 156],
    /// Optional Admin Command Support (OACS)
    pub oacs: u16,
    /// Abort Command Limit (ACL)
    pub acl: u8,
    /// Asynchronous Event Request Limit (AERL)
    pub aerl: u8,
    /// Firmware Updates (FRMW)
    pub frmw: u8,
    /// Log Page Attributes (LPA)
    pub lpa: u8,
    /// Error Log Page Entries (ELPE)
    pub elpe: u8,
    /// Number of Power States Support (NPSS)
    pub npss: u8,
    /// Admin Vendor Specific Command Configuration (AVSCC)
    pub avscc: u8,
    /// Autonomous Power State Transition Attributes (APSTA)
    pub apsta: u8,
    /// Warning Composite Temperature Threshold (WCTEMP)
    pub wctemp: u16,
    /// Critical Composite Temperature Threshold (CCTEMP)
    pub cctemp: u16,
    /// Maximum Time for Firmware Activation (MTFA)
    pub mtfa: u16,
    /// Host Memory Buffer Preferred Size (HMPRE)
    pub hmpre: u32,
    /// Host Memory Buffer Minimum Size (HMMIN)
    pub hmmin: u32,
    /// Total NVM Capacity (TNVMCAP)
    pub tnvmcap: u64,
    /// Unallocated NVM Capacity (UNVMCAP)
    pub unvmcap: u64,
    /// Replay Protected Memory Block Support (RPMBS)
    pub rpmbs: u32,
    /// Reserved
    pub reserved_1: [u8; 316],
    /// Submission Queue Entry Size (SQES)
    pub sqes: u8,
    /// Completion Queue Entry Size (CQES)
    pub cqes: u8,
    /// Reserved
    pub reserved_2: [u8; 28],
    /// NVM Subsystem NVMe Qualified Name (255 bytes)
    pub subnqn: [u8; 256],
    /// Reserved
    pub reserved_3: [u8; 768],
    /// I/O Command Set Combinations
    pub ioccss: [u32; 4],
    /// Reserved
    pub reserved_4: [u8; 128],
    /// NVM Subsystem Report
    pub subsys_rep: [u8; 2560],
}

/// Namespace Identify data structure (4096 bytes).
#[repr(C, align(4096))]
pub struct IdentifyNamespace {
    /// Namespace Size (NSZE)
    pub nsze: u64,
    /// Namespace Capacity (NCAP)
    pub ncap: u64,
    /// Namespace Utilization (NUSE)
    pub nuse: u64,
    /// Namespace Features (NSFEAT)
    pub nsfeat: u8,
    /// Number of LBA Formats (NLBAF)
    pub nlbaf: u8,
    /// Formatted LBA Size (FLBAS)
    pub flbas: u8,
    /// Metadata Capabilities (MC)
    pub mc: u8,
    /// End-to-end Data Protection Capabilities (DPC)
    pub dpc: u8,
    /// End-to-end Data Protection Type Settings (DPS)
    pub dps: u8,
    /// Namespace Multi-path I/O and Namespace Sharing Capabilities (NMIC)
    pub nmic: u8,
    /// Reservation Capabilities (RESCAP)
    pub rescap: u8,
    /// Format Progress Indicator (FPI)
    pub fpi: u8,
    /// Namespace Atomic Write Unit Normal (NAWUN)
    pub nawun: u16,
    /// Namespace Atomic Write Unit Power Fail (NAWUPF)
    pub nawupf: u16,
    /// Namespace Atomic Compare & Write Unit (NACWU)
    pub nacwu: u16,
    /// Namespace Atomic Boundary Size Normal (NABSN)
    pub nabsn: u16,
    /// Namespace Atomic Boundary Offset (NABO)
    pub nabo: u16,
    /// Namespace Atomic Boundary Size Power Fail (NABSPF)
    pub nabspf: u16,
    /// Namespace Optimal I/O Boundary (NOIOB)
    pub noiob: u16,
    /// NVM Capacity (NVMCAP)
    pub nvmcap: u64,
    /// Reserved
    pub reserved: [u8; 40],
    /// Namespace Globally Unique Identifier (NGUID)
    pub nguid: [u8; 16],
    /// IEEE Extended Unique Identifier (EUI64)
    pub eui64: u64,
    /// LBA Format Support
    pub lba_format: [LbaFormat; 16],
    /// Reserved
    pub reserved_2: [u8; 192],
    /// Vendor Specific
    pub vs: [u8; 3712],
}

/// Power State Descriptor (32 bytes) — from Identify Controller data.
/// Starts at byte offset 384 (0x180) in the Identify Controller data structure.
/// Each descriptor is 32 bytes, up to NPSS+1 descriptors.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PowerStateDescriptor {
    /// Maximum Power (in centiwatts, or as scaled by MXPS).
    pub mp: u16,
    /// Reserved.
    pub reserved_0: u8,
    /// Power State flags:
    ///   bit 0: MXPS (Max Power Scale) — 0=centiwatts, 1=0.1W
    ///   bit 1: NOPS (Non-Operational State)
    pub flags: u8,
    /// Entry Latency in microseconds.
    pub enlat: u32,
    /// Exit Latency in microseconds.
    pub exlat: u32,
    /// Relative Read Throughput.
    pub rrt: u8,
    /// Relative Read Latency.
    pub rrl: u8,
    /// Relative Write Throughput.
    pub rwt: u8,
    /// Relative Write Latency.
    pub rwl: u8,
    /// Idle Time Prior to Transition (IDLP) — in microseconds.
    pub idlp: u32,
    /// Reserved (bytes 20-31).
    pub reserved_4: [u8; 12],
}

impl PowerStateDescriptor {
    /// Maximum power in milliwatts.
    pub fn power_mw(&self) -> u32 {
        if (self.flags & 0x01) != 0 {
            // MXPS: 0.1W units
            (self.mp as u32) * 100
        } else {
            // Default: centiwatts (0.01W)
            (self.mp as u32) * 10
        }
    }

    /// Whether this is a non-operational state (no I/O processing).
    pub fn is_non_operational(&self) -> bool {
        (self.flags & 0x02) != 0
    }

    /// Whether idle timeout before transition is specified.
    pub fn has_idle_timeout(&self) -> bool {
        self.idlp > 0
    }
}

/// LBA Format data structure (part of IdentifyNamespace).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct LbaFormat {
    pub ms: u16,    // Metadata Size
    pub lbads: u8,  // LBA Data Size (as power of 2)
    pub rp: u8,     // Relative Performance
}

impl IdentifyController {
    /// Get the number of supported power states (NPSS + 1).
    pub fn num_power_states(&self) -> usize {
        (self.npss as usize) + 1
    }

    /// Get a power state descriptor by index.
    /// PSDs start at byte 384 (0x180) in the Identify Controller data.
    /// In our struct layout: reserved_1 starts at byte 300 (0x12C),
    /// so PSD[i] is at reserved_1[84 + i * 32].
    pub fn power_state(&self, idx: usize) -> Option<PowerStateDescriptor> {
        if idx > self.npss as usize {
            return None;
        }
        const PSD_RESERVED1_OFFSET: usize = 84; // 384 - 300
        let data = &self.reserved_1;
        let start = PSD_RESERVED1_OFFSET + idx * 32;
        if start + 32 > data.len() {
            return None;
        }
        unsafe {
            let ptr = &data[start] as *const u8 as *const PowerStateDescriptor;
            Some(*ptr)
        }
    }
}

impl IdentifyNamespace {
    /// Get the number of LBAs (sectors).
    pub fn nsze(&self) -> u64 { self.nsze }

    /// Get the LBA data size in bytes (from the currently formatted LBA).
    pub fn lba_data_size(&self) -> u32 {
        let idx = (self.flbas & 0x0F) as usize;
        if idx < 16 {
            1u32 << self.lba_format[idx].lbads
        } else {
            512 // default fallback
        }
    }

    /// Get metadata size.
    pub fn metadata_size(&self) -> u32 {
        let idx = (self.flbas & 0x0F) as usize;
        if idx < 16 {
            self.lba_format[idx].ms as u32
        } else {
            0
        }
    }
}

// ============================================================================
// Queue structures (in host memory)
// ============================================================================

/// Physical memory descriptor for a submission or completion queue.
/// The queue is allocated as contiguous DMA-able memory.
#[repr(C)]
pub struct QueueMem {
    /// Virtual address (MINIX user-space mapped).
    pub virt: *mut u8,
    /// Physical address (for programming NVMe doorbell registers).
    pub phys: u64,
    /// Size in bytes.
    pub size: usize,
}

impl QueueMem {
    /// Create a zeroed QueueMem.
    pub fn zeroed() -> Self {
        Self {
            virt: core::ptr::null_mut(),
            phys: 0,
            size: 0,
        }
    }
}

impl Clone for QueueMem {
    fn clone(&self) -> Self {
        Self { virt: self.virt, phys: self.phys, size: self.size }
    }
}

// ============================================================================
// PRP (Physical Region Page) constants
// ============================================================================

/// Default NVMe page size (4KB — MPS=0 → 2^12 = 4096).
pub const NVME_PAGE_SIZE: usize = 4096;

/// Maximum number of PRP entries per command (PRP list).
pub const MAX_PRP_LIST: usize = 512;

// ============================================================================
// Doorbell offsets
// ============================================================================

/// Compute the Submission Queue Tail Doorbell offset.
pub fn sq_tail_doorbell(qid: u16) -> usize {
    regs::DOORBELL_BASE + (2 * qid as usize) * (1 << regs::DOORBELL_STRIDE_DEFAULT)
}

/// Compute the Completion Queue Head Doorbell offset.
pub fn cq_head_doorbell(qid: u16) -> usize {
    regs::DOORBELL_BASE + (2 * qid as usize + 1) * (1 << regs::DOORBELL_STRIDE_DEFAULT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_entry_sizes() {
        assert_eq!(core::mem::size_of::<SqEntry>(), 64);
        assert_eq!(core::mem::size_of::<CqEntry>(), 16);
    }

    #[test]
    fn identify_struct_sizes() {
        assert_eq!(core::mem::size_of::<IdentifyController>(), 4096);
        assert_eq!(core::mem::size_of::<IdentifyNamespace>(), 4096);
    }

    #[test]
    fn sq_entry_ops() {
        let mut sq = SqEntry::zeroed();
        sq.set_cmd(opcode::IDENTIFY, 0);
        assert_eq!((sq.dword[0] >> 8) & 0xFF, opcode::IDENTIFY as u32);
        sq.set_nsid(1);
        assert_eq!(sq.dword[1], 1);
        sq.set_prp1(0x1000);
        assert_eq!(sq.dword[6], 0x1000);
        assert_eq!(sq.dword[7], 0);
        sq.set_prp1(0xF000_0000_1000);
        assert_eq!(sq.dword[7], 0xF000_0000);
    }

    #[test]
    fn cq_entry_status() {
        let mut cq = CqEntry { dword: [0; 4] };
        cq.dword[3] = 0; // Success
        assert!(cq.is_success());
        cq.dword[3] = 1; // Phase bit
        assert!(cq.phase());
        cq.dword[3] = (1 << 16) | 1; // Invalid Opcode + Phase
        assert_eq!(cq.status(), status::INVALID_OPCODE);
        assert!(!cq.is_success());
    }

    #[test]
    fn lba_format_extraction() {
        let mut ns = IdentifyNamespace {
            nsze: 0x1000,
            ncap: 0x1000,
            nuse: 0x1000,
            nsfeat: 0,
            nlbaf: 0,
            flbas: 0, // LBA format 0
            mc: 0, dpc: 0, dps: 0, nmic: 0, rescap: 0, fpi: 0,
            nawun: 0, nawupf: 0, nacwu: 0, nabsn: 0, nabo: 0, nabspf: 0, noiob: 0,
            nvmcap: 0,
            reserved: [0; 40],
            nguid: [0; 16],
            eui64: 0,
            lba_format: [LbaFormat { ms: 0, lbads: 9, rp: 0 }; 16],
            reserved_2: [0; 192],
            vs: [0; 3712],
        };
        // lbads=9 → 2^9 = 512 bytes per LBA
        assert_eq!(ns.lba_data_size(), 512);
        ns.lba_format[0].lbads = 12; // 2^12 = 4096 bytes per LBA
        assert_eq!(ns.lba_data_size(), 4096);
        assert_eq!(ns.nsze(), 0x1000);
    }

    #[test]
    fn doorbell_offsets() {
        assert_eq!(sq_tail_doorbell(0), regs::DOORBELL_BASE);
        assert_eq!(cq_head_doorbell(0), regs::DOORBELL_BASE + 4);
        assert_eq!(sq_tail_doorbell(1), regs::DOORBELL_BASE + 8);
        assert_eq!(cq_head_doorbell(1), regs::DOORBELL_BASE + 12);
    }

    #[test]
    fn pci_class_code() {
        assert_eq!(PCI_CLASS_STORAGE, 0x01);
        assert_eq!(PCI_SUBCLASS_NVM, 0x08);
        assert_eq!(PCI_PROGIF_NVME, 0x02);
    }
}
