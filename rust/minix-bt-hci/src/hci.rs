//! # Bluetooth HCI Protocol Definitions
//!
//! Implements the Bluetooth Host Controller Interface (HCI) packet format,
//! command/event definitions, ACL/SCO data structures, and basic helpers.
//!
//! ## HCI Packet Types (over USB transport)
//!
//! | Type | Name       | Direction      | USB Endpoint        |
//! |------|------------|----------------|---------------------|
//! | 0x01 | Command    | Host → Ctrl    | Bulk OUT            |
//! | 0x02 | ACL Data   | Bidirectional  | Bulk IN/OUT         |
//! | 0x03 | SCO Data   | Bidirectional  | Isochronous IN/OUT  |
//! | 0x04 | Event      | Ctrl → Host    | Bulk IN (or Interrupt) |
//! | 0x05 | ISO Data   | Bidirectional  | Bulk/Iso (BT 5.2+)  |
//!
//! ## HCI Command Format (3 bytes header + data)
//!
//! ```text
//! | OpCode (2)  | ParamLen (1) | Parameters (0..255) |
//! |  OCF | OGF   |              |                      |
//! ```
//!
//! ## HCI Event Format (2 bytes header + data)
//!
//! ```text
//! | EventCode (1) | ParamLen (1) | Parameters (0..255) |
//! ```
//!
//! ## ACL Data Format (4 bytes header + data)
//!
//! ```text
//! | Handle (2) | PB_BC (2) | DataLen (2) | Data |
//! |  handle:12 | flags:4  |             |       |
//! ```

#![allow(dead_code)]

// ============================================================================
// HCI Packet Types
// ============================================================================

/// HCI packet type indicators (first byte of USB HCI frame).
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HciPacketType {
    HciCommand  = 0x01,
    HciAclData  = 0x02,
    HciScoData  = 0x03,
    HciEvent    = 0x04,
    HciIsoData  = 0x05,  // BT 5.2+
}

impl HciPacketType {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Self::HciCommand),
            0x02 => Some(Self::HciAclData),
            0x03 => Some(Self::HciScoData),
            0x04 => Some(Self::HciEvent),
            0x05 => Some(Self::HciIsoData),
            _ => None,
        }
    }
}

// ============================================================================
// OpCode definitions (OGF = OpCode Group Field, OCF = OpCode Command Field)
// ============================================================================

/// OGF (OpCode Group Field) — top 6 bits of OpCode.
#[repr(u8)]
pub enum Ogf {
    LinkControl    = 0x01,
    LinkPolicy     = 0x02,
    ControllerBaseband = 0x03,
    Informational  = 0x04,
    StatusParams   = 0x05,
    Testing        = 0x06,
    LeController   = 0x08,
    VendorSpecific = 0x3F,
}

/// Build an HCI command OpCode from OGF and OCF.
pub const fn hci_opcode(ogf: u8, ocf: u16) -> u16 {
    (ocf & 0x03FF) | ((ogf as u16) << 10)
}

/// Extract OGF from an OpCode.
pub fn hci_ogf(opcode: u16) -> u8 {
    (opcode >> 10) as u8
}

/// Extract OCF from an OpCode.
pub fn hci_ocf(opcode: u16) -> u16 {
    opcode & 0x03FF
}

// ============================================================================
// Link Control Commands (OGF = 0x01)
// ============================================================================

pub mod link_ctrl {
    use super::*;

    pub const INQUIRY: u16                  = hci_opcode(Ogf::LinkControl as u8, 0x0001);
    pub const INQUIRY_CANCEL: u16           = hci_opcode(Ogf::LinkControl as u8, 0x0002);
    pub const CREATE_CONNECTION: u16        = hci_opcode(Ogf::LinkControl as u8, 0x0005);
    pub const DISCONNECT: u16               = hci_opcode(Ogf::LinkControl as u8, 0x0006);
    pub const ACCEPT_CONN_REQ: u16          = hci_opcode(Ogf::LinkControl as u8, 0x0009);
    pub const REJECT_CONN_REQ: u16          = hci_opcode(Ogf::LinkControl as u8, 0x000A);
    pub const LINK_KEY_REPLY: u16           = hci_opcode(Ogf::LinkControl as u8, 0x000B);
    pub const LINK_KEY_NEG_REPLY: u16       = hci_opcode(Ogf::LinkControl as u8, 0x000C);
    pub const PIN_CODE_REPLY: u16           = hci_opcode(Ogf::LinkControl as u8, 0x000D);
    pub const PIN_CODE_NEG_REPLY: u16       = hci_opcode(Ogf::LinkControl as u8, 0x000E);
    pub const REMOTE_NAME_REQ: u16          = hci_opcode(Ogf::LinkControl as u8, 0x0019);
    pub const REMOTE_NAME_REQ_CANCEL: u16   = hci_opcode(Ogf::LinkControl as u8, 0x001A);
    pub const READ_REMOTE_FEATURES: u16     = hci_opcode(Ogf::LinkControl as u8, 0x001B);
    pub const READ_REMOTE_EXT_FEATURES: u16 = hci_opcode(Ogf::LinkControl as u8, 0x001C);
    pub const READ_REMOTE_VERSION: u16      = hci_opcode(Ogf::LinkControl as u8, 0x001D);
    pub const SETUP_SCO_CONNECTION: u16     = hci_opcode(Ogf::LinkControl as u8, 0x0028);
    pub const ACCEPT_SCO_CONN_REQ: u16      = hci_opcode(Ogf::LinkControl as u8, 0x0029);
    pub const REJECT_SCO_CONN_REQ: u16      = hci_opcode(Ogf::LinkControl as u8, 0x002A);
    pub const SET_TRANSPORT: u16            = hci_opcode(Ogf::LinkControl as u8, 0x0043);  // BT 5.2
}

// ============================================================================
// Controller & Baseband Commands (OGF = 0x03)
// ============================================================================

pub mod ctrl_bb {
    use super::*;

    pub const SET_EVENT_MASK: u16          = hci_opcode(Ogf::ControllerBaseband as u8, 0x0001);
    pub const RESET: u16                   = hci_opcode(Ogf::ControllerBaseband as u8, 0x0003);
    pub const SET_EVENT_FILTER: u16        = hci_opcode(Ogf::ControllerBaseband as u8, 0x0005);
    pub const READ_LOCAL_NAME: u16         = hci_opcode(Ogf::ControllerBaseband as u8, 0x0014);
    pub const WRITE_LOCAL_NAME: u16        = hci_opcode(Ogf::ControllerBaseband as u8, 0x0013);
    pub const READ_CLASS_OF_DEVICE: u16    = hci_opcode(Ogf::ControllerBaseband as u8, 0x0023);
    pub const WRITE_CLASS_OF_DEVICE: u16   = hci_opcode(Ogf::ControllerBaseband as u8, 0x0024);
    pub const READ_VOICE_SETTING: u16      = hci_opcode(Ogf::ControllerBaseband as u8, 0x0025);
    pub const WRITE_VOICE_SETTING: u16     = hci_opcode(Ogf::ControllerBaseband as u8, 0x0026);
    pub const WRITE_AUTOMATIC_FLUSH: u16   = hci_opcode(Ogf::ControllerBaseband as u8, 0x0028);
    pub const READ_NUM_BCAST_RETX: u16     = hci_opcode(Ogf::ControllerBaseband as u8, 0x0029);
    pub const WRITE_NUM_BCAST_RETX: u16    = hci_opcode(Ogf::ControllerBaseband as u8, 0x002A);
    pub const HOST_BUFFER_SIZE: u16        = hci_opcode(Ogf::ControllerBaseband as u8, 0x0033);
    pub const READ_LE_HOST_SUPPORT: u16    = hci_opcode(Ogf::ControllerBaseband as u8, 0x006C);
    pub const WRITE_LE_HOST_SUPPORT: u16   = hci_opcode(Ogf::ControllerBaseband as u8, 0x006D);
    pub const READ_SIMPLE_PAIRING_MODE: u16 = hci_opcode(Ogf::ControllerBaseband as u8, 0x0055);
    pub const WRITE_SIMPLE_PAIRING_MODE: u16 = hci_opcode(Ogf::ControllerBaseband as u8, 0x0056);
}

// ============================================================================
// Informational Commands (OGF = 0x04)
// ============================================================================

pub mod info {
    use super::*;

    pub const READ_LOCAL_VERSION: u16     = hci_opcode(Ogf::Informational as u8, 0x0001);
    pub const READ_LOCAL_COMMANDS: u16    = hci_opcode(Ogf::Informational as u8, 0x0002);
    pub const READ_LOCAL_FEATURES: u16    = hci_opcode(Ogf::Informational as u8, 0x0003);
    pub const READ_LOCAL_EXT_FEATURES: u16 = hci_opcode(Ogf::Informational as u8, 0x0004);
    pub const READ_BUFFER_SIZE: u16       = hci_opcode(Ogf::Informational as u8, 0x0005);
    pub const READ_BD_ADDR: u16           = hci_opcode(Ogf::Informational as u8, 0x0009);
}

// ============================================================================
// Status Parameters Commands (OGF = 0x05)
// ============================================================================

pub mod status {
    use super::*;

    pub const READ_RSSI: u16              = hci_opcode(Ogf::StatusParams as u8, 0x0005);
    pub const READ_LINK_QUALITY: u16      = hci_opcode(Ogf::StatusParams as u8, 0x0002);
    pub const READ_AFH_MAP: u16           = hci_opcode(Ogf::StatusParams as u8, 0x0006);
}

// ============================================================================
// LE Controller Commands (OGF = 0x08)
// ============================================================================

pub mod le {
    use super::*;

    pub const LE_SET_EVENT_MASK: u16          = hci_opcode(Ogf::LeController as u8, 0x0001);
    pub const LE_READ_BUFFER_SIZE: u16        = hci_opcode(Ogf::LeController as u8, 0x0002);
    pub const LE_READ_LOCAL_FEATURES: u16     = hci_opcode(Ogf::LeController as u8, 0x0003);
    pub const LE_SET_ADV_PARAMS: u16          = hci_opcode(Ogf::LeController as u8, 0x0006);
    pub const LE_SET_ADV_DATA: u16            = hci_opcode(Ogf::LeController as u8, 0x0008);
    pub const LE_SET_ADV_ENABLE: u16          = hci_opcode(Ogf::LeController as u8, 0x000A);
    pub const LE_SET_SCAN_PARAMS: u16         = hci_opcode(Ogf::LeController as u8, 0x000B);
    pub const LE_SET_SCAN_ENABLE: u16         = hci_opcode(Ogf::LeController as u8, 0x000C);
    pub const LE_CREATE_CONNECTION: u16       = hci_opcode(Ogf::LeController as u8, 0x000D);
    pub const LE_CONN_UPDATE: u16             = hci_opcode(Ogf::LeController as u8, 0x0013);
    pub const LE_READ_REMOTE_FEATURES: u16    = hci_opcode(Ogf::LeController as u8, 0x0016);
    pub const LE_SET_EXT_ADV_PARAMS: u16      = hci_opcode(Ogf::LeController as u8, 0x0036); // BT 5.0+
    pub const LE_SET_EXT_ADV_DATA: u16        = hci_opcode(Ogf::LeController as u8, 0x0037);
    pub const LE_SET_ADV_SET_RAND_ADDR: u16   = hci_opcode(Ogf::LeController as u8, 0x0035);
    pub const LE_EXT_ADV_SET_PARAMS: u16      = hci_opcode(Ogf::LeController as u8, 0x0036);
    pub const LE_SETUP_ISO_DATA_PATH: u16     = hci_opcode(Ogf::LeController as u8, 0x0062); // BT 5.2
    pub const LE_REMOVE_ISO_DATA_PATH: u16    = hci_opcode(Ogf::LeController as u8, 0x0063);
}

// ============================================================================
// HCI Event Codes
// ============================================================================

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HciEventCode {
    InquiryComplete           = 0x01,
    InquiryResult             = 0x02,
    ConnectionComplete        = 0x03,
    ConnectionRequest         = 0x04,
    DisconnectionComplete     = 0x05,
    AuthenticationComplete    = 0x06,
    RemoteNameRequestComplete = 0x07,
    EncryptionChange          = 0x08,
    ChangeConnectionLinkKey   = 0x09,
    MasterLinkKeyComplete     = 0x0A,
    ReadRemoteFeaturesComplete = 0x0B,
    ReadRemoteVersionComplete = 0x0C,
    CommandComplete           = 0x0E,
    CommandStatus             = 0x0F,
    HardwareError             = 0x10,
    NumberOfCompletedPackets  = 0x13,
    RoleChange                = 0x12,
    DataBufferOverflow        = 0x1A,
    MaxSlotsChange            = 0x1B,
    PinCodeRequest            = 0x16,
    LinkKeyRequest            = 0x17,
    LinkKeyNotification       = 0x18,
    LoopbackCommand           = 0x19,
    ReadClockOffsetComplete   = 0x1C,
    ConnectionPacketTypeChange = 0x1D,
    QoSViolation              = 0x1E,
    PageScanModeChange        = 0x1F,
    PageScanRepetitionModeChange = 0x20,
    FlowSpecificationComplete = 0x21,
    InquiryResultWithRssi     = 0x22,
    ReadRemoteExtendedFeaturesComplete = 0x23,
    LeMeta                    = 0x3E,
    NumberOfCompletedBlocks   = 0x44,
}

impl HciEventCode {
    pub fn from_byte(b: u8) -> Option<Self> {
        use HciEventCode::*;
        Some(match b {
            0x01 => InquiryComplete,
            0x02 => InquiryResult,
            0x03 => ConnectionComplete,
            0x04 => ConnectionRequest,
            0x05 => DisconnectionComplete,
            0x06 => AuthenticationComplete,
            0x07 => RemoteNameRequestComplete,
            0x08 => EncryptionChange,
            0x09 => ChangeConnectionLinkKey,
            0x0A => MasterLinkKeyComplete,
            0x0B => ReadRemoteFeaturesComplete,
            0x0C => ReadRemoteVersionComplete,
            0x0E => CommandComplete,
            0x0F => CommandStatus,
            0x10 => HardwareError,
            0x12 => RoleChange,
            0x13 => NumberOfCompletedPackets,
            0x16 => PinCodeRequest,
            0x17 => LinkKeyRequest,
            0x18 => LinkKeyNotification,
            0x19 => LoopbackCommand,
            0x1A => DataBufferOverflow,
            0x1B => MaxSlotsChange,
            0x1C => ReadClockOffsetComplete,
            0x1D => ConnectionPacketTypeChange,
            0x1E => QoSViolation,
            0x1F => PageScanModeChange,
            0x20 => PageScanRepetitionModeChange,
            0x21 => FlowSpecificationComplete,
            0x22 => InquiryResultWithRssi,
            0x23 => ReadRemoteExtendedFeaturesComplete,
            0x3E => LeMeta,
            0x44 => NumberOfCompletedBlocks,
            _ => return None,
        })
    }
}

// ============================================================================
// HCI LE Meta Events (sub-event codes for HciEventCode::LeMeta = 0x3E)
// ============================================================================

#[repr(u8)]
pub enum LeMetaEvent {
    ConnectionComplete        = 0x01,
    AdvertisingReport        = 0x02,
    ConnectionUpdateComplete  = 0x03,
    ReadRemoteFeaturesComplete = 0x04,
    LongTermKeyRequest        = 0x05,
    RemoteConnectionParamReq  = 0x06,
    DataLengthChange          = 0x07,
    ReadLocalP256KeyComplete  = 0x08,
    PhyUpdateComplete         = 0x09,
    EnhancedConnectionComplete = 0x0A,
    DirectAdvReport           = 0x0B,
    ChannelSelection          = 0x0C,
}

// ============================================================================
// HCI Status codes
// ============================================================================

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HciStatus {
    Success               = 0x00,
    UnknownCommand        = 0x01,
    UnknownConnId         = 0x02,
    HardwareFailure       = 0x03,
    PageTimeout           = 0x04,
    AuthenticationFailure = 0x05,
    PinKeyMissing         = 0x06,
    MemoryCapacityExceeded = 0x07,
    ConnectionTimeout     = 0x08,
    ConnectionLimitExceeded = 0x09,
    MaxConnectionsSync    = 0x0A,
    AclDataExceeded       = 0x0B,
    CommandDisallowed     = 0x0C,
    RejectedResources     = 0x0D,
    RejectedSecurity      = 0x0E,
    RejectedPersonal      = 0x0F,
    HostTimeout           = 0x10,
    UnsupportedFeature    = 0x11,
    InvalidParams         = 0x12,
    RemoteUserEnded       = 0x13,
    RemoteLowResources    = 0x14,
    RemotePowerOff        = 0x15,
    LocalHostTerminated   = 0x16,
    RepeatedAttempts      = 0x17,
    PairingNotAllowed     = 0x18,
    UnknownLmpPdu         = 0x19,
    UnsupportedRemote     = 0x1A,
    ScoOffsetRejected     = 0x1B,
    ScoIntervalRejected   = 0x1C,
    AirModeRejected       = 0x1D,
    InvalidLlParams       = 0x1E,
    UnspecifiedError      = 0x1F,
    UnsupportedParam      = 0x20,
    RoleChangeNotAllowed  = 0x21,
}

impl HciStatus {
    pub fn from_byte(b: u8) -> Self {
        use HciStatus::*;
        match b {
            0x00 => Success,
            0x01 => UnknownCommand,
            0x02 => UnknownConnId,
            0x03 => HardwareFailure,
            0x04 => PageTimeout,
            0x05 => AuthenticationFailure,
            0x06 => PinKeyMissing,
            0x07 => MemoryCapacityExceeded,
            0x08 => ConnectionTimeout,
            0x09 => ConnectionLimitExceeded,
            0x0A => MaxConnectionsSync,
            0x0B => AclDataExceeded,
            0x0C => CommandDisallowed,
            0x0D => RejectedResources,
            0x0E => RejectedSecurity,
            0x0F => RejectedPersonal,
            0x10 => HostTimeout,
            0x11 => UnsupportedFeature,
            0x12 => InvalidParams,
            0x13 => RemoteUserEnded,
            0x14 => RemoteLowResources,
            0x15 => RemotePowerOff,
            0x16 => LocalHostTerminated,
            0x17 => RepeatedAttempts,
            0x18 => PairingNotAllowed,
            0x19 => UnknownLmpPdu,
            0x1A => UnsupportedRemote,
            0x1B => ScoOffsetRejected,
            0x1C => ScoIntervalRejected,
            0x1D => AirModeRejected,
            0x1E => InvalidLlParams,
            0x1F => UnspecifiedError,
            0x20 => UnsupportedParam,
            0x21 => RoleChangeNotAllowed,
            _ => UnspecifiedError,
        }
    }
}

// ============================================================================
// Packet Builders — construct HCI command/ACL/event buffers
// ============================================================================

/// Maximum HCI packet payload size (for buffer sizing).
pub const HCI_MAX_CMD_SIZE: usize = 255 + 3;    // header + max param
pub const HCI_MAX_EVT_SIZE: usize = 255 + 2;    // header + max param
pub const HCI_MAX_ACL_SIZE: usize = 65535 + 4;  // header + max data
pub const HCI_MAX_SCO_SIZE: usize = 255 + 3;

/// Build an HCI command packet in the provided buffer.
/// Returns the total packet length (including type byte and header).
///
/// Buffer layout: [Type(1) | OpCode(2) | ParamLen(1) | Parameters(N)]
pub fn build_hci_cmd(buf: &mut [u8], opcode: u16, params: &[u8]) -> usize {
    if buf.len() < 4 || params.len() > 255 {
        return 0;
    }

    let param_len = params.len() as u8;
    buf[0] = HciPacketType::HciCommand as u8;  // Type = 0x01
    buf[1] = (opcode & 0xFF) as u8;            // OpCode LSB
    buf[2] = (opcode >> 8) as u8;              // OpCode MSB
    buf[3] = param_len;                         // Parameter length

    // Copy parameters
    let copy_len = core::cmp::min(params.len(), buf.len() - 4);
    buf[4..4 + copy_len].copy_from_slice(&params[..copy_len]);

    4 + copy_len
}

/// Build an HCI ACL data packet.
///
/// Buffer layout: [Type(1) | Handle(2) | PB_BC(2) | DataLen(2) | Data(N)]
pub fn build_hci_acl(buf: &mut [u8], handle: u16, pb_flag: u8, bc_flag: u8,
                     data: &[u8]) -> usize {
    if buf.len() < 5 {
        return 0;
    }

    let data_len = core::cmp::min(data.len(), buf.len() - 5);
    if data_len > 65535 {
        return 0;
    }

    buf[0] = HciPacketType::HciAclData as u8;   // Type = 0x02
    // Handle + PB + BC flags in first 2 bytes
    let handle_flags = (handle & 0x0FFF) | ((pb_flag as u16) << 12) | ((bc_flag as u16) << 14);
    buf[1] = (handle_flags & 0xFF) as u8;
    buf[2] = (handle_flags >> 8) as u8;
    buf[3] = (data_len & 0xFF) as u8;            // Data Length LSB
    buf[4] = ((data_len >> 8) & 0xFF) as u8;     // Data Length MSB

    buf[5..5 + data_len].copy_from_slice(&data[..data_len]);

    5 + data_len
}

/// Build an HCI SCO data packet.
pub fn build_hci_sco(buf: &mut [u8], handle: u16, data: &[u8]) -> usize {
    if buf.len() < 4 {
        return 0;
    }

    let data_len = core::cmp::min(data.len(), buf.len() - 4);
    buf[0] = HciPacketType::HciScoData as u8;    // Type = 0x03
    buf[1] = (handle & 0xFF) as u8;              // Connection Handle LSB
    buf[2] = ((handle >> 8) & 0x0F) as u8;       // Connection Handle MSB (bits 8-11)
    buf[3] = data_len as u8;                      // Data Length

    buf[4..4 + data_len].copy_from_slice(&data[..data_len]);

    4 + data_len
}

// ============================================================================
// Packet Parsers — extract data from received HCI packets
// ============================================================================

/// Parse an HCI Command Complete event.
/// Returns (opcode, status, parameter_offset) where parameter_offset is the
/// index into buffer where event parameters start (after the type, event code,
/// param_len, num_cmd_packets, opcode, and status bytes).
pub fn parse_cmd_complete(buf: &[u8]) -> Option<(u16, u8, usize)> {
    // HCI frame: [Type(1) | Event(1) | Len(1) | NumPkt(1) | OpCode(2) | Status(1) | Params...]
    if buf.len() < 8 { return None; }
    if buf[0] != HciPacketType::HciEvent as u8 { return None; }
    if buf[1] != HciEventCode::CommandComplete as u8 { return None; }

    let param_len = buf[2] as usize;
    if param_len < 4 { return None; }
    if 3 + param_len > buf.len() { return None; }

    let _num_cmd_packets = buf[3];
    let opcode = (buf[4] as u16) | ((buf[5] as u16) << 8);
    let status = buf[6];

    // Parameters start at offset 7 (after opcode and status)
    Some((opcode, status, 7))
}

/// Parse a simple HCI event (Command Complete variant with immediate status).
/// Returns true if the command completed successfully.
pub fn check_cmd_success(buf: &[u8], expected_opcode: u16) -> bool {
    if let Some((opcode, status, _params)) = parse_cmd_complete(buf) {
        opcode == expected_opcode && status == 0
    } else {
        false
    }
}

/// Parse an HCI ACL data header from a received buffer.
/// Returns (connection_handle, pb_flag, bc_flag, data_length) or None.
pub fn parse_acl_header(buf: &[u8]) -> Option<(u16, u8, u8, u16)> {
    if buf.len() < 5 { return None; }
    if buf[0] != HciPacketType::HciAclData as u8 { return None; }

    let handle_flags = (buf[1] as u16) | ((buf[2] as u16) << 8);
    let handle = handle_flags & 0x0FFF;
    let pb = ((handle_flags >> 12) & 0x03) as u8;
    let bc = ((handle_flags >> 14) & 0x03) as u8;
    let data_len = (buf[3] as u16) | ((buf[4] as u16) << 8);

    Some((handle, pb, bc, data_len))
}

/// Parse an HCI Event header from a received buffer.
/// Returns (event_code, param_length) or None.
pub fn parse_event_header(buf: &[u8]) -> Option<(u8, u8)> {
    if buf.len() < 3 { return None; }
    if buf[0] != HciPacketType::HciEvent as u8 { return None; }

    Some((buf[1], buf[2]))
}

// ============================================================================
// BD_ADDR helper (Bluetooth device address — 6 bytes)
// ============================================================================

/// Bluetooth Device Address (6 bytes, stored in little-endian on wire).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct BdAddr(pub [u8; 6]);

impl BdAddr {
    pub fn new(bytes: [u8; 6]) -> Self { Self(bytes) }

    /// Format as colon-separated hex string (e.g. "AA:BB:CC:DD:EE:FF").
    pub fn format(&self, buf: &mut [u8]) -> usize {
        if buf.len() < 18 { return 0; }
        let hex = b"0123456789ABCDEF";
        let mut pos = 0;
        for i in 0..6 {
            if i > 0 { buf[pos] = b':'; pos += 1; }
            buf[pos] = hex[(self.0[5 - i] >> 4) as usize]; pos += 1;
            buf[pos] = hex[(self.0[5 - i] & 0x0F) as usize]; pos += 1;
        }
        pos
    }

    pub fn is_empty(&self) -> bool {
        self.0 == [0u8; 6]
    }
}

// ============================================================================
// HCI Transport State
// ============================================================================

/// State of the HCI controller.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HciState {
    /// Initial state — waiting for reset.
    Reset,
    /// Controller reset completed — configuring.
    Configuring,
    /// Controller is up and ready for commands/data.
    Up,
    /// Controller is down / radio off.
    Down,
    /// Fatal error.
    Error,
}

/// Buffer management for USB HCI transfers.
/// Sized for the largest possible HCI packet (ACL data: 65535 + header).
pub const HCI_TRANSFER_BUF_SIZE: usize = 65536;

/// Number of HCI command buffers (for queued commands).
pub const HCI_NUM_CMD_BUFS: usize = 16;

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hci_opcode_build() {
        assert_eq!(hci_opcode(Ogf::Informational as u8, 0x0001), 0x1001);
        assert_eq!(hci_ogf(0x1001), 0x04);
        assert_eq!(hci_ocf(0x1001), 0x0001);
    }

    #[test]
    fn build_cmd_packet() {
        let mut buf = [0u8; 32];
        let params = [0x01, 0x02, 0x03];
        let len = build_hci_cmd(&mut buf, 0x1001, &params);
        assert_eq!(len, 7);
        assert_eq!(buf[0], 0x01);  // Type = Command
        assert_eq!(buf[1], 0x01);  // OpCode LSB
        assert_eq!(buf[2], 0x10);  // OpCode MSB
        assert_eq!(buf[3], 0x03);  // Param len
        assert_eq!(buf[4], 0x01);  // Param[0]
        assert_eq!(buf[5], 0x02);  // Param[1]
        assert_eq!(buf[6], 0x03);  // Param[2]
    }

    #[test]
    fn build_acl_packet() {
        let mut buf = [0u8; 32];
        let data = [0xAA; 10];
        let len = build_hci_acl(&mut buf, 0x0042, 0x02, 0x00, &data);
        assert_eq!(len, 15);
        assert_eq!(buf[0], 0x02);  // Type = ACL
        // handle=0x0042, pb=2, bc=0 → handle_flags=0x0042 | 0x2000 = 0x2042
        assert_eq!(buf[1], 0x42);
        assert_eq!(buf[2], 0x20);
        assert_eq!(buf[3], 0x0A);  // Data len = 10
        assert_eq!(buf[4], 0x00);
        assert_eq!(&buf[5..15], &[0xAA; 10]);
    }

    #[test]
    fn parse_cmd_complete_success() {
        // Mock a Command Complete event
        let mut evt = [0u8; 10];
        evt[0] = HciPacketType::HciEvent as u8;
        evt[1] = HciEventCode::CommandComplete as u8;
        evt[2] = 0x06;  // Param len (4 fixed + 2 params)
        evt[3] = 0x01;  // Num cmd packets
        evt[4] = 0x01;  // OpCode LSB (0x1001)
        evt[5] = 0x10;  // OpCode MSB
        evt[6] = 0x00;  // Status = Success
        evt[7] = 0xAB;  // Extra param data
        evt[8] = 0xCD;

        let result = parse_cmd_complete(&evt);
        assert!(result.is_some());
        let (opcode, status, off) = result.unwrap();
        assert_eq!(opcode, 0x1001);
        assert_eq!(status, 0x00);
        assert_eq!(off, 7);
        assert!(check_cmd_success(&evt, 0x1001));
    }

    #[test]
    fn parse_cmd_complete_failure() {
        let mut evt = [0u8; 10];
        evt[0] = HciPacketType::HciEvent as u8;
        evt[1] = HciEventCode::CommandComplete as u8;
        evt[2] = 0x06;
        evt[3] = 0x01;
        evt[4] = 0x01;
        evt[5] = 0x10;
        evt[6] = 0x0C;  // Status = CommandDisallowed

        assert!(!check_cmd_success(&evt, 0x1001));
    }

    #[test]
    fn bd_addr_format() {
        let addr = BdAddr([0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF]);
        let mut buf = [0u8; 18];
        let len = addr.format(&mut buf);
        assert_eq!(len, 17);
        assert_eq!(&buf[..17], b"FF:EE:DD:CC:BB:AA");
    }

    #[test]
    fn event_code_from_byte() {
        assert_eq!(HciEventCode::from_byte(0x0E), Some(HciEventCode::CommandComplete));
        assert_eq!(HciEventCode::from_byte(0x3E), Some(HciEventCode::LeMeta));
        assert_eq!(HciEventCode::from_byte(0xFF), None);
    }

    #[test]
    fn test_parse_event_header() {
        let mut buf = [0u8; 5];
        buf[0] = HciPacketType::HciEvent as u8;
        buf[1] = 0x0E;  // CommandComplete
        buf[2] = 0x03;  // param len
        let (code, plen) = parse_event_header(&buf).unwrap();
        assert_eq!(code, 0x0E);
        assert_eq!(plen, 3);
    }

    #[test]
    fn test_hci_state_transitions() {
        assert_eq!(HciState::Reset as u8, 0);
        assert_eq!(HciState::Configuring as u8, 1);
        assert_eq!(HciState::Up as u8, 2);
        assert_eq!(HciState::Down as u8, 3);
        assert_eq!(HciState::Error as u8, 4);
    }
}
