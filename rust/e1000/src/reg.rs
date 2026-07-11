//! # E1000 Register Definitions
//!
//! Hardware-specific register offsets and bit flags
//! for Intel PRO/1000 Gigabit Ethernet controllers.

// ============================================================================
// Controller Registers
// ============================================================================

/// Device Control
pub const CTRL: u32 = 0x00000;
/// Device Status
pub const STATUS: u32 = 0x00008;
/// EEPROM Read
pub const EERD: u32 = 0x00014;
/// Flow Control Address Low
pub const FCAL: u32 = 0x00028;
/// Flow Control Address High
pub const FCAH: u32 = 0x0002c;
/// Flow Control Type
pub const FCT: u32 = 0x00030;
/// Flow Control Transmit Timer Value
pub const FCTTV: u32 = 0x00170;
/// Interrupt Cause Read
pub const ICR: u32 = 0x000c0;
/// Interrupt Mask Set/Read
pub const IMS: u32 = 0x000d0;
/// Receive Control
pub const RCTL: u32 = 0x00100;
/// Transmit Control
pub const TCTL: u32 = 0x00400;
/// Receive Descriptor Base Address Low
pub const RDBAL: u32 = 0x02800;
/// Receive Descriptor Base Address High
pub const RDBAH: u32 = 0x02804;
/// Receive Descriptor Length
pub const RDLEN: u32 = 0x02808;
/// Receive Descriptor Head
pub const RDH: u32 = 0x02810;
/// Receive Descriptor Tail
pub const RDT: u32 = 0x02818;
/// Transmit Descriptor Base Address Low
pub const TDBAL: u32 = 0x03800;
/// Transmit Descriptor Base Address High
pub const TDBAH: u32 = 0x03804;
/// Transmit Descriptor Length
pub const TDLEN: u32 = 0x03808;
/// Transmit Descriptor Head
pub const TDH: u32 = 0x03810;
/// Transmit Descriptor Tail
pub const TDT: u32 = 0x03818;
/// CRC Error Count
pub const CRCERRS: u32 = 0x04000;
/// RX Error Count
pub const RXERRC: u32 = 0x0400c;
/// Missed Packets Count
pub const MPC: u32 = 0x04010;
/// Collision Count
pub const COLC: u32 = 0x04028;
/// Total Packets Received
pub const TPR: u32 = 0x040D0;
/// Total Packets Transmitted
pub const TPT: u32 = 0x040D4;
/// Receive Address Low
pub const RAL: u32 = 0x05400;
/// Receive Address High
pub const RAH: u32 = 0x05404;
/// Multicast Table Array
pub const MTA: u32 = 0x05200;

// ============================================================================
// CTRL register bits
// ============================================================================

/// Link Reset
pub const CTRL_LRST: u32 = 1 << 3;
/// Auto-Speed Detection Enable
pub const CTRL_ASDE: u32 = 1 << 5;
/// Set Link Up
pub const CTRL_SLU: u32 = 1 << 6;
/// Invert Loss Of Signal
pub const CTRL_ILOS: u32 = 1 << 7;
/// Device Reset
pub const CTRL_RST: u32 = 1 << 26;
/// VLAN Mode Enable
pub const CTRL_VME: u32 = 1 << 30;
/// PHY Reset
pub const CTRL_PHY_RST: u32 = 1 << 31;

// ============================================================================
// STATUS register bits
// ============================================================================

/// Full Duplex
pub const STATUS_FD: u32 = 1 << 0;
/// Link Up
pub const STATUS_LU: u32 = 1 << 1;
/// Transmission Paused
pub const STATUS_TXOFF: u32 = 1 << 4;
/// Link Speed (bits 6:7)
pub const STATUS_SPEED: u32 = (1 << 6) | (1 << 7);
pub const STATUS_SPEED_10: u32 = 0 << 6;
pub const STATUS_SPEED_100: u32 = 1 << 6;
pub const STATUS_SPEED_1000_A: u32 = 2 << 6;
pub const STATUS_SPEED_1000_B: u32 = 3 << 6;

// ============================================================================
// EERD register bits
// ============================================================================

/// Start Read
pub const EERD_START: u32 = 1 << 0;
/// Read Done (8254x)
pub const EERD_DONE: u32 = 1 << 4;
/// Read Address Mask
pub const EERD_ADDR: u32 = 0xff << 8;
/// Read Data Mask
pub const EERD_DATA: u32 = 0xffff << 16;

// ============================================================================
// ICR / IMS register bits
// ============================================================================

/// Transmit Descriptors Written Back
pub const ICR_TXDW: u32 = 1 << 0;
/// Transmit Queue Empty
pub const ICR_TXQE: u32 = 1 << 1;
/// Link Status Change
pub const ICR_LSC: u32 = 1 << 2;
/// Receiver Overrun
pub const ICR_RXO: u32 = 1 << 6;
/// Receiver Timer Interrupt
pub const ICR_RXT: u32 = 1 << 7;

// ============================================================================
// RCTL register bits
// ============================================================================

/// Receive Enable
pub const RCTL_EN: u32 = 1 << 1;
/// Unicast Promiscuous Enable
pub const RCTL_UPE: u32 = 1 << 3;
/// Multicast Promiscuous Enable
pub const RCTL_MPE: u32 = 1 << 4;
/// Broadcast Accept Mode
pub const RCTL_BAM: u32 = 1 << 15;
/// Receive Buffer Size (bits 17:16)
pub const RCTL_BSIZE: u32 = (1 << 16) | (1 << 17);
/// Buffer Size Extension (bit 25) — when set, BSIZE=00→16384, 01→8192, 10→32768, 11→65536
pub const RCTL_BSEX: u32 = 1 << 25;

// ============================================================================
// TCTL register bits
// ============================================================================

/// Transmit Enable
pub const TCTL_EN: u32 = 1 << 1;
/// Pad Short Packets
pub const TCTL_PSP: u32 = 1 << 3;

// ============================================================================
// RAH register bits
// ============================================================================

/// Receive Address Valid
pub const RAH_AV: u32 = 1 << 31;

// ============================================================================
// ICH Flash Registers
// ============================================================================

pub const ICH_FLASH_GFPREG: u32 = 0x0000;
pub const ICH_FLASH_HSFSTS: u32 = 0x0004;
pub const ICH_FLASH_HSFCTL: u32 = 0x0006;
pub const ICH_FLASH_FADDR: u32 = 0x0008;
pub const ICH_FLASH_FDATA0: u32 = 0x0010;
pub const FLASH_GFPREG_BASE_MASK: u32 = 0x1FFF;
pub const FLASH_SECTOR_ADDR_SHIFT: u32 = 12;
pub const ICH_FLASH_READ_COMMAND_TIMEOUT: u32 = 500;
pub const ICH_FLASH_LINEAR_ADDR_MASK: u32 = 0x00FFFFFF;
pub const ICH_CYCLE_READ: u16 = 0;
pub const ICH_FLASH_CYCLE_REPEAT_COUNT: u32 = 10;

// ============================================================================
// MSI-X Registers
// ============================================================================

// IVAR — Interrupt Vector Allocation Register
// Assigns interrupt causes to MSI-X vectors.
// Each 2-byte entry: bits 7:0 = vector, bit 15 = valid
pub const IVAR: u32 = 0x00E00;

// IVAR entry offsets (each 2 bytes within IVAR)
pub const IVAR_RX0: u32 = 0;    // RX queue 0
pub const IVAR_TX0: u32 = 2;    // TX queue 0
pub const IVAR_RX1: u32 = 4;    // RX queue 1
pub const IVAR_TX1: u32 = 6;    // TX queue 1
pub const IVAR_OTHER: u32 = 8;  // Other causes (link, errors)

/// IVAR entry valid bit
pub const IVAR_VALID: u32 = 1 << 15;

// EICR — Extended Interrupt Cause Read (used in MSI-X mode)
pub const EICR: u32 = 0x01580;
// EIAC — Extended Interrupt Auto Clear (auto-clear on read)
pub const EIAC: u32 = 0x0158C;
// EIMS — Extended Interrupt Mask Set
pub const EIMS: u32 = 0x01524;
// EIMC — Extended Interrupt Mask Clear
pub const EIMC: u32 = 0x01528;

/// EICR/EIMS bit definitions
pub const EICR_RX0: u32 = 1 << 0;   // RX queue 0
pub const EICR_TX0: u32 = 1 << 1;   // TX queue 0
pub const EICR_OTHER: u32 = 1 << 2; // Other causes

// ============================================================================
// Configuration constants
// ============================================================================

/// Number of receive descriptors
pub const RXDESC_NR: usize = 256;
/// Number of transmit descriptors
pub const TXDESC_NR: usize = 256;
/// Size of each I/O buffer (16384 = max supported by RCTL.BSEX+BSIZE=00)
pub const IOBUF_SIZE: usize = 16384;

/// EERD read timeout (number of poll iterations before giving up)
pub const EERD_READ_TIMEOUT: u32 = 100_000;

// ============================================================================
// C FFI test verification functions
// ============================================================================

/// Return e1000 version code: 0x1008254E (device ID family).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn e1000_test_version() -> u32 {
    0x1008254E
}

/// Return register byte offset by register ID (0..=34).
/// Returns 0xFFFFFFFF for unknown IDs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn e1000_test_reg_offset(reg_id: u32) -> u32 {
    match reg_id {
        0  => CTRL,
        1  => STATUS,
        2  => EERD,
        3  => FCAL,
        4  => FCAH,
        5  => FCT,
        6  => FCTTV,
        7  => ICR,
        8  => IMS,
        9  => RCTL,
        10 => TCTL,
        11 => RDBAL,
        12 => RDBAH,
        13 => RDLEN,
        14 => RDH,
        15 => RDT,
        16 => TDBAL,
        17 => TDBAH,
        18 => TDLEN,
        19 => TDH,
        20 => TDT,
        21 => CRCERRS,
        22 => RXERRC,
        23 => MPC,
        24 => COLC,
        25 => TPR,
        26 => TPT,
        27 => RAL,
        28 => RAH,
        29 => MTA,
        30 => IVAR,
        31 => EICR,
        32 => EIAC,
        33 => EIMS,
        34 => EIMC,
        _  => 0xFFFFFFFF,
    }
}

/// Return bitfield/constant value by bitfield ID (0..=44).
/// Returns 0xFFFFFFFF for unknown IDs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn e1000_test_bitfield(bf_id: u32) -> u32 {
    match bf_id {
        // CTRL bitfields (0-6)
        0  => CTRL_LRST,
        1  => CTRL_ASDE,
        2  => CTRL_SLU,
        3  => CTRL_ILOS,
        4  => CTRL_RST,
        5  => CTRL_VME,
        6  => CTRL_PHY_RST,
        // STATUS bitfields (7-14)
        7  => STATUS_FD,
        8  => STATUS_LU,
        9  => STATUS_TXOFF,
        10 => STATUS_SPEED,
        11 => STATUS_SPEED_10,
        12 => STATUS_SPEED_100,
        13 => STATUS_SPEED_1000_A,
        14 => STATUS_SPEED_1000_B,
        // EERD bitfields (15-18)
        15 => EERD_START,
        16 => EERD_DONE,
        17 => EERD_ADDR,
        18 => EERD_DATA,
        // ICR/IMS bitfields (19-23)
        19 => ICR_TXDW,
        20 => ICR_TXQE,
        21 => ICR_LSC,
        22 => ICR_RXO,
        23 => ICR_RXT,
        // RCTL bitfields (24-29)
        24 => RCTL_EN,
        25 => RCTL_UPE,
        26 => RCTL_MPE,
        27 => RCTL_BAM,
        28 => RCTL_BSIZE,
        29 => RCTL_BSEX,
        // TCTL bitfields (30-31)
        30 => TCTL_EN,
        31 => TCTL_PSP,
        // RAH (32)
        32 => RAH_AV,
        // IVAR (33)
        33 => IVAR_VALID,
        // EICR (34-36)
        34 => EICR_RX0,
        35 => EICR_TX0,
        36 => EICR_OTHER,
        // Config constants (37-40)
        37 => RXDESC_NR as u32,
        38 => TXDESC_NR as u32,
        39 => IOBUF_SIZE as u32,
        40 => EERD_READ_TIMEOUT,
        // IVAR entry offsets (41-45)
        41 => IVAR_RX0,
        42 => IVAR_TX0,
        43 => IVAR_RX1,
        44 => IVAR_TX1,
        45 => IVAR_OTHER,
        _  => 0xFFFFFFFF,
    }
}

#[cfg(test)]
mod ffi_tests {
    use super::*;

    #[test]
    fn test_version_consistency() {
        assert_eq!(unsafe { e1000_test_version() }, 0x1008254E);
    }

    #[test]
    fn test_reg_offset_roundtrip() {
        // Spot-check: known register offsets
        assert_eq!(unsafe { e1000_test_reg_offset(0) },  CTRL);     // 0x00000
        assert_eq!(unsafe { e1000_test_reg_offset(1) },  STATUS);  // 0x00008
        assert_eq!(unsafe { e1000_test_reg_offset(9) },  RCTL);    // 0x00100
        assert_eq!(unsafe { e1000_test_reg_offset(10) }, TCTL);    // 0x00400
        assert_eq!(unsafe { e1000_test_reg_offset(11) }, RDBAL);   // 0x02800
        assert_eq!(unsafe { e1000_test_reg_offset(30) }, IVAR);    // 0x00E00
        // Unknown
        assert_eq!(unsafe { e1000_test_reg_offset(99) }, 0xFFFFFFFF);
    }

    #[test]
    fn test_bitfield_roundtrip() {
        // Spot-check: known bitfields
        assert_eq!(unsafe { e1000_test_bitfield(0) },  CTRL_LRST);
        assert_eq!(unsafe { e1000_test_bitfield(4) },  CTRL_RST);
        assert_eq!(unsafe { e1000_test_bitfield(7) },  STATUS_FD);
        assert_eq!(unsafe { e1000_test_bitfield(19) }, ICR_TXDW);
        assert_eq!(unsafe { e1000_test_bitfield(24) }, RCTL_EN);
        assert_eq!(unsafe { e1000_test_bitfield(32) }, RAH_AV);
        // Unknown
        assert_eq!(unsafe { e1000_test_bitfield(99) }, 0xFFFFFFFF);
    }
}
