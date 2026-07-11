//! # Common Bluetooth Types
//!
//! Shared types used across all protocol layers of the Bluetooth stack.
//! Includes BD_ADDR, UUID, L2CAP CID, and other fundamental types.

#![allow(dead_code)]

use core::fmt;

// ============================================================================
// Bluetooth Device Address (BD_ADDR)
// ============================================================================

/// 6-byte Bluetooth device address (stored in big-endian / wire order).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BdAddr(pub [u8; 6]);

impl BdAddr {
    /// Create a new BD_ADDR from raw bytes.
    pub const fn new(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }

    /// Create a null / invalid address (00:00:00:00:00:00).
    pub const fn null() -> Self {
        Self([0u8; 6])
    }

    /// Check if this is the null address.
    pub fn is_null(&self) -> bool {
        self.0 == [0u8; 6]
    }

    /// Format as colon-separated hex string (e.g. "AA:BB:CC:DD:EE:FF").
    /// Writes into the provided buffer and returns the number of bytes written.
    pub fn format(&self, buf: &mut [u8]) -> usize {
        if buf.len() < 17 {
            return 0;
        }
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        let mut pos = 0;
        // In human-readable format, the bytes are reversed (big-endian display)
        for i in (0..6).rev() {
            if i < 5 {
                buf[pos] = b':';
                pos += 1;
            }
            buf[pos] = HEX[(self.0[i] >> 4) as usize];
            pos += 1;
            buf[pos] = HEX[(self.0[i] & 0x0F) as usize];
            pos += 1;
        }
        pos
    }

    /// Parse a colon-separated hex address string.
    /// Accepts formats like "AA:BB:CC:DD:EE:FF" or "aabbccddeeff".
    pub fn parse(s: &str) -> Option<Self> {
        let bytes = if s.contains(':') {
            let parts: Vec<&str> = s.split(':').collect();
            if parts.len() != 6 {
                return None;
            }
            let mut b = [0u8; 6];
            for (i, part) in parts.iter().enumerate() {
                if part.len() != 2 {
                    return None;
                }
                b[i] = u8::from_str_radix(part, 16).ok()?;
            }
            b
        } else if s.len() == 12 {
            let mut b = [0u8; 6];
            for i in 0..6 {
                b[i] = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).ok()?;
            }
            b
        } else {
            return None;
        };
        Some(Self(bytes))
    }
}

impl fmt::Display for BdAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; 18];
        let len = self.format(&mut buf);
        let s = core::str::from_utf8(&buf[..len]).unwrap_or("??:??:??:??:??:??");
        write!(f, "{}", s)
    }
}

impl fmt::Debug for BdAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BdAddr({})", self)
    }
}

impl From<[u8; 6]> for BdAddr {
    fn from(bytes: [u8; 6]) -> Self {
        Self(bytes)
    }
}

// ============================================================================
// Bluetooth UUID
// ============================================================================

/// 128-bit Bluetooth UUID.
/// Can represent 16-bit, 32-bit, and 128-bit UUIDs.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BtUuid {
    /// Full 128-bit UUID in big-endian bytes.
    pub bytes: [u8; 16],
}

impl BtUuid {
    /// Create from a 128-bit value (big-endian bytes).
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self { bytes }
    }

    /// Create from a 16-bit Bluetooth UUID (Bluetooth Base UUID).
    /// Bluetooth Base UUID: 0000xxxx-0000-1000-8000-00805F9B34FB
    pub const fn from_uuid16(short: u16) -> Self {
        let mut bytes = BLUETOOTH_BASE_UUID;
        bytes[2] = (short >> 8) as u8;
        bytes[3] = (short & 0xFF) as u8;
        Self { bytes }
    }

    /// Create from a 32-bit Bluetooth UUID.
    pub const fn from_uuid32(short: u32) -> Self {
        let mut bytes = BLUETOOTH_BASE_UUID;
        bytes[0] = (short >> 24) as u8;
        bytes[1] = (short >> 16) as u8;
        bytes[2] = (short >> 8) as u8;
        bytes[3] = (short & 0xFF) as u8;
        Self { bytes }
    }

    /// Return the UUID type.
    /// Detects whether this is a 16-bit, 32-bit, or 128-bit UUID
    /// by checking the Bluetooth Base UUID suffix in bytes[4..10].
    pub fn uuid_type(&self) -> BtUuidType {
        // If bytes[4..10] don't match the Bluetooth Base UUID, it's 128-bit
        if self.bytes[4..10] != BLUETOOTH_BASE_UUID[4..10] {
            return BtUuidType::Uuid128;
        }

        // Upper 16 bits of the 32-bit field are zero → 16-bit UUID
        if self.bytes[0] == 0 && self.bytes[1] == 0 {
            // If lower 16 bits are also zero, it's the base UUID itself (128-bit)
            if self.bytes[2] == 0 && self.bytes[3] == 0 {
                BtUuidType::Uuid128
            } else {
                BtUuidType::Uuid16
            }
        } else {
            BtUuidType::Uuid32
        }
    }

    /// Get the 16-bit short UUID, if applicable.
    pub fn as_uuid16(&self) -> Option<u16> {
        if matches!(self.uuid_type(), BtUuidType::Uuid16) {
            Some((self.bytes[2] as u16) << 8 | (self.bytes[3] as u16))
        } else {
            None
        }
    }

    /// Get the 32-bit short UUID, if applicable.
    pub fn as_uuid32(&self) -> Option<u32> {
        if matches!(self.uuid_type(), BtUuidType::Uuid32) {
            Some(
                (self.bytes[0] as u32) << 24
                    | (self.bytes[1] as u32) << 16
                    | (self.bytes[2] as u32) << 8
                    | (self.bytes[3] as u32),
            )
        } else {
            None
        }
    }
}

/// Bluetooth Base UUID: 00000000-0000-1000-8000-00805F9B34FB
pub const BLUETOOTH_BASE_UUID: [u8; 16] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00,
    0x80, 0x00, 0x00, 0x80, 0x5F, 0x9B, 0x34, 0xFB,
];

/// Alias for the Bluetooth Base UUID (used in SDP parsing).
pub const BLUETOOTH_BASE_UUID_R: [u8; 16] = BLUETOOTH_BASE_UUID;

/// Type of Bluetooth UUID.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BtUuidType {
    Uuid16,
    Uuid32,
    Uuid128,
}

impl fmt::Display for BtUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            self.bytes[0], self.bytes[1], self.bytes[2], self.bytes[3],
            self.bytes[4], self.bytes[5], self.bytes[6], self.bytes[7],
            self.bytes[8], self.bytes[9], self.bytes[10], self.bytes[11],
            self.bytes[12], self.bytes[13], self.bytes[14], self.bytes[15],
        )
    }
}

impl fmt::Debug for BtUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BtUuid({})", self)
    }
}

impl From<u16> for BtUuid {
    fn from(short: u16) -> Self {
        Self::from_uuid16(short)
    }
}

impl From<u32> for BtUuid {
    fn from(short: u32) -> Self {
        Self::from_uuid32(short)
    }
}

// Well-known SDP UUIDs
pub mod sdp_uuids {
    use super::BtUuid;

    /// SDP (Service Discovery Protocol)
    pub const SDP: BtUuid = BtUuid::from_uuid16(0x0001);
    /// RFCOMM (Serial Port Emulation)
    pub const RFCOMM: BtUuid = BtUuid::from_uuid16(0x0003);
    /// L2CAP (Logical Link Control and Adaptation Protocol)
    pub const L2CAP: BtUuid = BtUuid::from_uuid16(0x0100);
    /// ATT (Attribute Protocol)
    pub const ATT: BtUuid = BtUuid::from_uuid16(0x0007);
    /// GATT (Generic Attribute Profile)
    pub const GATT: BtUuid = BtUuid::from_uuid16(0x1801);
    /// Battery Service
    pub const BATTERY_SERVICE: BtUuid = BtUuid::from_uuid16(0x180F);
    /// Device Information Service
    pub const DEVICE_INFO: BtUuid = BtUuid::from_uuid16(0x180A);
    /// Generic Access Profile
    pub const GAP: BtUuid = BtUuid::from_uuid16(0x1800);
    /// Serial Port Profile
    pub const SERIAL_PORT: BtUuid = BtUuid::from_uuid16(0x1101);
    /// Headset Profile
    pub const HEADSET: BtUuid = BtUuid::from_uuid16(0x1108);
    /// Hands-Free Profile
    pub const HANDSFREE: BtUuid = BtUuid::from_uuid16(0x111E);
    /// Human Interface Device (HID)
    pub const HID: BtUuid = BtUuid::from_uuid16(0x1124);
    /// Advanced Audio Distribution Profile (A2DP)
    pub const A2DP: BtUuid = BtUuid::from_uuid16(0x110D);
    /// Audio/Video Remote Control Profile (AVRCP)
    pub const AVRCP: BtUuid = BtUuid::from_uuid16(0x110E);
    /// Personal Area Networking (PAN)
    pub const PAN: BtUuid = BtUuid::from_uuid16(0x1115);
}

// ============================================================================
// L2CAP Channel Identifiers
// ============================================================================

/// Pre-defined L2CAP Channel Identifiers (CIDs).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum L2CapCid {
    /// Null identifier (invalid).
    Null = 0x0000,
    /// Signaling channel (BR/EDR).
    Signaling = 0x0001,
    /// Connectionless reception.
    Connectionless = 0x0002,
    /// AMP Manager Protocol.
    AmpManager = 0x0003,
    /// Attribute Protocol (ATT).
    Attribute = 0x0004,
    /// LE L2CAP Signaling channel.
    LeSignaling = 0x0005,
    /// LE Security Manager Protocol.
    LeSecurityManager = 0x0006,
    /// BR/EDR Security Manager Protocol.
    BrEdrSecurityManager = 0x0007,
    /// Dynamically allocated channel.
    Dynamic(u16),
}

impl L2CapCid {
    /// Create from raw 16-bit value.
    pub fn from_raw(raw: u16) -> Self {
        match raw {
            0x0000 => Self::Null,
            0x0001 => Self::Signaling,
            0x0002 => Self::Connectionless,
            0x0003 => Self::AmpManager,
            0x0004 => Self::Attribute,
            0x0005 => Self::LeSignaling,
            0x0006 => Self::LeSecurityManager,
            0x0007 => Self::BrEdrSecurityManager,
            cid => Self::Dynamic(cid),
        }
    }

    /// Get raw 16-bit value.
    pub fn to_raw(self) -> u16 {
        match self {
            Self::Null => 0x0000,
            Self::Signaling => 0x0001,
            Self::Connectionless => 0x0002,
            Self::AmpManager => 0x0003,
            Self::Attribute => 0x0004,
            Self::LeSignaling => 0x0005,
            Self::LeSecurityManager => 0x0006,
            Self::BrEdrSecurityManager => 0x0007,
            Self::Dynamic(cid) => cid,
        }
    }

    /// Whether this CID is a fixed/well-known channel.
    pub fn is_fixed(&self) -> bool {
        !matches!(self, Self::Dynamic(_))
    }

    /// Whether this CID is dynamically allocated (in the range 0x0040-0xFFFF).
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Self::Dynamic(cid) if *cid >= 0x0040)
    }
}

impl From<u16> for L2CapCid {
    fn from(raw: u16) -> Self {
        Self::from_raw(raw)
    }
}

impl From<L2CapCid> for u16 {
    fn from(cid: L2CapCid) -> Self {
        cid.to_raw()
    }
}

// ============================================================================
// L2CAP Protocol / Service Multiplexer (PSM)
// ============================================================================

/// L2CAP Protocol/Service Multiplexer values.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum L2CapPsm {
    /// SDP (Service Discovery Protocol)
    Sdp = 0x0001,
    /// RFCOMM
    Rfcomm = 0x0003,
    /// TCS Binary (Telephony Control)
    TcsBin = 0x0005,
    /// TCS Binary (Cordless Telephony)
    TcsBinCordless = 0x0007,
    /// BNEP (Bluetooth Network Encapsulation Protocol)
    Bnep = 0x000F,
    /// HID Control
    HidControl = 0x0011,
    /// HID Interrupt
    HidInterrupt = 0x0013,
    /// AVCTP (Audio/Video Control Transport Protocol)
    Avctp = 0x0017,
    /// AVDTP (Audio/Video Distribution Transport Protocol)
    Avdtp = 0x0019,
    /// CMTP (Common ISDN API)
    Cmtp = 0x001B,
    /// MCAP (Multi-Channel Adaptation Protocol)
    McapControl = 0x001D,
    /// MCAP Data
    McapData = 0x001F,
    /// L2CAP (LE connection-oriented channels)
    LeCoC = 0x0025,
    /// Vendor-specific or unknown
    Unknown(u16),
}

impl L2CapPsm {
    /// Create from raw 16-bit PSM value.
    /// Bluetooth PSM values are 9 bits (0x000–0x1FF), bit 0 MUST be 1 (odd).
    pub fn from_raw(raw: u16) -> Self {
        match raw & 0x01FF {
            0x0001 => Self::Sdp,
            0x0003 => Self::Rfcomm,
            0x0005 => Self::TcsBin,
            0x0007 => Self::TcsBinCordless,
            0x000F => Self::Bnep,
            0x0011 => Self::HidControl,
            0x0013 => Self::HidInterrupt,
            0x0017 => Self::Avctp,
            0x0019 => Self::Avdtp,
            0x001B => Self::Cmtp,
            0x001D => Self::McapControl,
            0x001F => Self::McapData,
            0x0025 => Self::LeCoC,
            _ => Self::Unknown(raw & 0x01FF),
        }
    }

    pub fn to_raw(self) -> u16 {
        match self {
            Self::Sdp => 0x0001,
            Self::Rfcomm => 0x0003,
            Self::TcsBin => 0x0005,
            Self::TcsBinCordless => 0x0007,
            Self::Bnep => 0x000F,
            Self::HidControl => 0x0011,
            Self::HidInterrupt => 0x0013,
            Self::Avctp => 0x0017,
            Self::Avdtp => 0x0019,
            Self::Cmtp => 0x001B,
            Self::McapControl => 0x001D,
            Self::McapData => 0x001F,
            Self::LeCoC => 0x0025,
            Self::Unknown(raw) => raw,
        }
    }
}

// ============================================================================
// HCI Connection Handle
// ============================================================================

/// HCI Connection Handle (12-bit value).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ConnHandle(pub u16);

impl ConnHandle {
    pub fn new(raw: u16) -> Self {
        Self(raw & 0x0FFF)
    }

    pub fn to_raw(self) -> u16 {
        self.0
    }
}

impl From<u16> for ConnHandle {
    fn from(raw: u16) -> Self {
        Self(raw & 0x0FFF)
    }
}

// ============================================================================
// Bluetooth Device Class
// ============================================================================

/// Bluetooth Class of Device (24-bit).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClassOfDevice(pub [u8; 3]);

impl ClassOfDevice {
    pub const fn new(bytes: [u8; 3]) -> Self {
        Self(bytes)
    }

    /// Major service class bits.
    pub fn major_service(&self) -> u8 {
        self.0[2]
    }

    /// Major device class.
    pub fn major_class(&self) -> u8 {
        self.0[1] & 0x1F
    }

    /// Minor device class.
    pub fn minor_class(&self) -> u8 {
        (self.0[0] >> 2) & 0x3F
    }
}

// ============================================================================
// Inquiry Result
// ============================================================================

/// Result from an HCI Inquiry.
#[derive(Clone, Debug)]
pub struct InquiryResult {
    pub bdaddr: BdAddr,
    pub class_of_device: ClassOfDevice,
    pub clock_offset: u16,
    pub rssi: Option<i8>,
    pub page_scan_repetition_mode: u8,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bdaddr_format() {
        let addr = BdAddr([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        let mut buf = [0u8; 18];
        let len = addr.format(&mut buf);
        assert_eq!(len, 17);
        assert_eq!(&buf[..17], b"FF:EE:DD:CC:BB:AA");
    }

    #[test]
    fn test_bdaddr_parse_colon() {
        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();
        assert_eq!(addr.0, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }

    #[test]
    fn test_bdaddr_parse_hex() {
        let addr = BdAddr::parse("AABBCCDDEEFF").unwrap();
        assert_eq!(addr.0, [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
    }

    #[test]
    fn test_bdaddr_null() {
        let addr = BdAddr::null();
        assert!(addr.is_null());
    }

    #[test]
    fn test_bdaddr_display() {
        let addr = BdAddr([0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC]);
        assert_eq!(format!("{}", addr), "BC:9A:78:56:34:12");
    }

    #[test]
    fn test_uuid16() {
        let uuid = BtUuid::from_uuid16(0x1101);
        assert_eq!(uuid.uuid_type(), BtUuidType::Uuid16);
        assert_eq!(uuid.as_uuid16(), Some(0x1101));
    }

    #[test]
    fn test_uuid32() {
        let uuid = BtUuid::from_uuid32(0x12345678);
        assert_eq!(uuid.uuid_type(), BtUuidType::Uuid32);
        assert_eq!(uuid.as_uuid32(), Some(0x12345678));
    }

    #[test]
    fn test_uuid128() {
        let bytes = [
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
            0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10,
        ];
        let uuid = BtUuid::from_bytes(bytes);
        assert_eq!(uuid.uuid_type(), BtUuidType::Uuid128);
    }

    #[test]
    fn test_sdp_uuids() {
        assert_eq!(sdp_uuids::SDP.as_uuid16(), Some(0x0001));
        assert_eq!(sdp_uuids::RFCOMM.as_uuid16(), Some(0x0003));
        assert_eq!(sdp_uuids::GATT.as_uuid16(), Some(0x1801));
    }

    #[test]
    fn test_l2cap_cid() {
        assert_eq!(L2CapCid::from_raw(0x0001), L2CapCid::Signaling);
        assert!(L2CapCid::from_raw(0x0001).is_fixed());
        assert!(!L2CapCid::from_raw(0x0041).is_fixed());
        assert!(L2CapCid::from_raw(0x0041).is_dynamic());
    }

    #[test]
    fn test_l2cap_psm() {
        assert_eq!(L2CapPsm::from_raw(0x0001), L2CapPsm::Sdp);
        assert_eq!(L2CapPsm::from_raw(0x0003), L2CapPsm::Rfcomm);
        assert_eq!(L2CapPsm::from_raw(0x0019), L2CapPsm::Avdtp);
    }

    #[test]
    fn test_class_of_device() {
        let cod = ClassOfDevice::new([0x04, 0x02, 0x00]);
        assert_eq!(cod.major_class(), 0x02);
        assert_eq!(cod.minor_class(), 0x01);
    }
}
