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
/// Receive Buffer Size
pub const RCTL_BSIZE: u32 = (1 << 16) | (1 << 17);

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
// Configuration constants
// ============================================================================

/// Number of receive descriptors
pub const RXDESC_NR: usize = 256;
/// Number of transmit descriptors
pub const TXDESC_NR: usize = 256;
/// Size of each I/O buffer
pub const IOBUF_SIZE: usize = 2048;
