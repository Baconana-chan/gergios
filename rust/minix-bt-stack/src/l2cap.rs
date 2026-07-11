//! # L2CAP — Logical Link Control and Adaptation Protocol
//!
//! Implements the Bluetooth L2CAP layer which provides:
//! - Protocol/channel multiplexing
//! - Segmentation and reassembly
//! - Quality of Service configuration
//!
//! ## B-frame format (Basic frame)
//!
//! ```text
//! | Length (2) | Channel ID (2) | Payload (0..65535) |
//! ```
//!
//! ## Signaling packet format
//!
//! ```text
//! | Code (1) | ID (1) | Length (2) | Data (0..65535) |
//! ```

#![allow(dead_code)]

use crate::types::{ConnHandle, L2CapPsm};

// ============================================================================
// Constants
// ============================================================================

/// Default L2CAP MTU.
pub const L2CAP_DEFAULT_MTU: u16 = 672;
/// Minimum L2CAP MTU (as per Bluetooth spec).
pub const L2CAP_MIN_MTU: u16 = 48;
/// Maximum L2CAP MTU.
pub const L2CAP_MAX_MTU: u16 = 65535;
/// L2CAP signaling channel CID.
pub const L2CAP_SIG_CID: u16 = 0x0001;
/// L2CAP connectionless channel CID.
pub const L2CAP_CONNECTIONLESS_CID: u16 = 0x0002;

// ============================================================================
// L2CAP Channel States
// ============================================================================

/// L2CAP channel state machine.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum L2CapChannelState {
    /// Channel is closed.
    Closed,
    /// Connection request sent, waiting for response.
    WaitConnect,
    /// Configure request/response in progress.
    Config,
    /// Channel is open for data transfer.
    Open,
    /// Disconnection request sent, waiting for response.
    WaitDisconnect,
}

// ============================================================================
// L2CAP B-frame
// ============================================================================

/// A decoded L2CAP B-frame.
#[derive(Clone, Debug)]
pub struct L2CapBFrame {
    /// Channel ID (destination).
    pub cid: u16,
    /// Payload data.
    pub payload: Vec<u8>,
}

/// Parse an L2CAP B-frame from raw bytes.
/// Returns (BFrame, bytes_consumed) or None if incomplete.
pub fn parse_l2cap_b_frame(data: &[u8]) -> Option<(L2CapBFrame, usize)> {
    if data.len() < 4 {
        return None;
    }
    let length = u16::from_le_bytes([data[0], data[1]]) as usize;
    let cid = u16::from_le_bytes([data[2], data[3]]);

    if 4 + length > data.len() {
        return None; // Incomplete frame
    }

    let payload = data[4..4 + length].to_vec();
    Some((L2CapBFrame { cid, payload }, 4 + length))
}

/// Build an L2CAP B-frame.
pub fn build_l2cap_b_frame(cid: u16, payload: &[u8]) -> Vec<u8> {
    let len = payload.len() as u16;
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&cid.to_le_bytes());
    buf.extend_from_slice(payload);
    buf
}

// ============================================================================
// L2CAP Signaling Protocol
// ============================================================================

/// L2CAP signaling command codes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum L2CapSigCode {
    CommandReject = 0x01,
    ConnectionRequest = 0x02,
    ConnectionResponse = 0x03,
    ConfigureRequest = 0x04,
    ConfigureResponse = 0x05,
    DisconnectionRequest = 0x06,
    DisconnectionResponse = 0x07,
    EchoRequest = 0x08,
    EchoResponse = 0x09,
    InformationRequest = 0x0A,
    InformationResponse = 0x0B,
    ConnectionParamUpdateReq = 0x12,
    ConnectionParamUpdateRsp = 0x13,
    LeCreditBasedConnReq = 0x14,
    LeCreditBasedConnRsp = 0x15,
    LeFlowControlCredit = 0x16,
}

impl L2CapSigCode {
    pub fn from_byte(b: u8) -> Option<Self> {
        use L2CapSigCode::*;
        Some(match b {
            0x01 => CommandReject,
            0x02 => ConnectionRequest,
            0x03 => ConnectionResponse,
            0x04 => ConfigureRequest,
            0x05 => ConfigureResponse,
            0x06 => DisconnectionRequest,
            0x07 => DisconnectionResponse,
            0x08 => EchoRequest,
            0x09 => EchoResponse,
            0x0A => InformationRequest,
            0x0B => InformationResponse,
            0x12 => ConnectionParamUpdateReq,
            0x13 => ConnectionParamUpdateRsp,
            0x14 => LeCreditBasedConnReq,
            0x15 => LeCreditBasedConnRsp,
            0x16 => LeFlowControlCredit,
            _ => return None,
        })
    }
}

/// L2CAP signaling command (inside a signaling channel packet).
#[derive(Clone, Debug)]
pub struct L2CapSigCommand {
    pub code: L2CapSigCode,
    pub id: u8,
    pub data: Vec<u8>,
}

/// Parse L2CAP signaling commands from a signaling channel payload.
pub fn parse_sig_commands(data: &[u8]) -> Vec<L2CapSigCommand> {
    let mut commands = Vec::new();
    let mut offset = 0;

    while offset + 4 <= data.len() {
        let code_byte = data[offset];
        let id = data[offset + 1];
        let length = u16::from_le_bytes([data[offset + 2], data[offset + 3]]) as usize;

        if offset + 4 + length > data.len() {
            break; // Malformed — stop parsing
        }

        if let Some(code) = L2CapSigCode::from_byte(code_byte) {
            commands.push(L2CapSigCommand {
                code,
                id,
                data: data[offset + 4..offset + 4 + length].to_vec(),
            });
        }

        offset += 4 + length;
    }

    commands
}

/// Build L2CAP signaling command.
pub fn build_sig_command(code: L2CapSigCode, id: u8, data: &[u8]) -> Vec<u8> {
    let len = data.len() as u16;
    let mut buf = Vec::with_capacity(4 + data.len());
    buf.push(code as u8);
    buf.push(id);
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(data);
    buf
}

// ============================================================================
// Connection Request/Response
// ============================================================================

/// L2CAP connection request.
#[derive(Clone, Debug)]
pub struct L2CapConnReq {
    pub psm: L2CapPsm,
    pub source_cid: u16,
}

impl L2CapConnReq {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let psm_raw = u16::from_le_bytes([data[0], data[1]]);
        let source_cid = u16::from_le_bytes([data[2], data[3]]);
        Some(Self {
            psm: L2CapPsm::from_raw(psm_raw),
            source_cid,
        })
    }

    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4);
        buf.extend_from_slice(&self.psm.to_raw().to_le_bytes());
        buf.extend_from_slice(&self.source_cid.to_le_bytes());
        buf
    }
}

/// L2CAP connection response result codes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum L2CapConnResult {
    Success = 0x0000,
    Pending = 0x0001,
    PsmNotSupported = 0x0002,
    SecurityBlock = 0x0003,
    NoResources = 0x0004,
    InvalidParams = 0x0005,
    InvalidSourceCid = 0x0006,
    SourceCidAlreadyAllocated = 0x0007,
}

impl L2CapConnResult {
    pub fn from_raw(raw: u16) -> Self {
        match raw {
            0x0000 => Self::Success,
            0x0001 => Self::Pending,
            0x0002 => Self::PsmNotSupported,
            0x0003 => Self::SecurityBlock,
            0x0004 => Self::NoResources,
            0x0005 => Self::InvalidParams,
            0x0006 => Self::InvalidSourceCid,
            0x0007 => Self::SourceCidAlreadyAllocated,
            _ => Self::NoResources,
        }
    }
}

/// L2CAP connection response.
#[derive(Clone, Debug)]
pub struct L2CapConnRsp {
    pub destination_cid: u16,
    pub source_cid: u16,
    pub result: L2CapConnResult,
    pub status: u16, // 0x0000 = no info, 0x0001 = authentication pending
}

impl L2CapConnRsp {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        let destination_cid = u16::from_le_bytes([data[0], data[1]]);
        let source_cid = u16::from_le_bytes([data[2], data[3]]);
        let result = L2CapConnResult::from_raw(u16::from_le_bytes([data[4], data[5]]));
        let status = u16::from_le_bytes([data[6], data[7]]);
        Some(Self {
            destination_cid,
            source_cid,
            result,
            status,
        })
    }

    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(8);
        buf.extend_from_slice(&self.destination_cid.to_le_bytes());
        buf.extend_from_slice(&self.source_cid.to_le_bytes());
        buf.extend_from_slice(&(self.result as u16).to_le_bytes());
        buf.extend_from_slice(&self.status.to_le_bytes());
        buf
    }
}

// ============================================================================
// Configuration Request/Response
// ============================================================================

/// L2CAP configuration option types.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum L2CapConfigOptType {
    Mtu = 0x01,
    FlushTimeout = 0x02,
    QualityOfService = 0x03,
    RetransmissionAndFlowControl = 0x04,
    Fcs = 0x05,
    ExtendedFlowSpec = 0x06,
    ExtendedWindowSize = 0x07,
    /// Unknown option type — preserves the raw type byte through parse/re-serialize.
    Unknown(u8),
}

impl L2CapConfigOptType {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0x01 => Self::Mtu,
            0x02 => Self::FlushTimeout,
            0x03 => Self::QualityOfService,
            0x04 => Self::RetransmissionAndFlowControl,
            0x05 => Self::Fcs,
            0x06 => Self::ExtendedFlowSpec,
            0x07 => Self::ExtendedWindowSize,
            _ => Self::Unknown(b),
        }
    }

    /// Return the raw byte value for this option type.
    pub fn to_byte(self) -> u8 {
        match self {
            Self::Mtu => 0x01,
            Self::FlushTimeout => 0x02,
            Self::QualityOfService => 0x03,
            Self::RetransmissionAndFlowControl => 0x04,
            Self::Fcs => 0x05,
            Self::ExtendedFlowSpec => 0x06,
            Self::ExtendedWindowSize => 0x07,
            Self::Unknown(b) => b,
        }
    }
}

/// L2CAP configuration option.
#[derive(Clone, Debug)]
pub struct L2CapConfigOpt {
    pub opt_type: L2CapConfigOptType,
    pub value: Vec<u8>,
}

/// L2CAP configure request.
#[derive(Clone, Debug)]
pub struct L2CapConfigReq {
    pub destination_cid: u16,
    pub flags: u16, // Bit 0 = continuation
    pub options: Vec<L2CapConfigOpt>,
}

impl L2CapConfigReq {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let destination_cid = u16::from_le_bytes([data[0], data[1]]);
        let flags = u16::from_le_bytes([data[2], data[3]]);
        let mut options = Vec::new();
        let mut offset = 4;

        while offset + 2 <= data.len() {
            let opt_type = L2CapConfigOptType::from_byte(data[offset]);
            let opt_len = data[offset + 1] as usize;
            if offset + 2 + opt_len > data.len() {
                break;
            }
            options.push(L2CapConfigOpt {
                opt_type,
                value: data[offset + 2..offset + 2 + opt_len].to_vec(),
            });
            offset += 2 + opt_len;
        }

        Some(Self {
            destination_cid,
            flags,
            options,
        })
    }

    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&self.destination_cid.to_le_bytes());
        buf.extend_from_slice(&self.flags.to_le_bytes());
        for opt in &self.options {
            buf.push(opt.opt_type.to_byte());
            buf.push(opt.value.len() as u8);
            buf.extend_from_slice(&opt.value);
        }
        buf
    }
}

/// L2CAP configure response result codes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum L2CapConfigResult {
    Success = 0x0000,
    UnacceptableParams = 0x0001,
    Rejected = 0x0002,
}

impl L2CapConfigResult {
    pub fn from_raw(raw: u16) -> Self {
        match raw {
            0x0000 => Self::Success,
            0x0001 => Self::UnacceptableParams,
            0x0002 => Self::Rejected,
            _ => Self::Rejected,
        }
    }
}

/// L2CAP configure response.
#[derive(Clone, Debug)]
pub struct L2CapConfigRsp {
    pub source_cid: u16,
    pub flags: u16,
    pub result: L2CapConfigResult,
    pub options: Vec<L2CapConfigOpt>,
}

impl L2CapConfigRsp {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 6 {
            return None;
        }
        let source_cid = u16::from_le_bytes([data[0], data[1]]);
        let flags = u16::from_le_bytes([data[2], data[3]]);
        let result = L2CapConfigResult::from_raw(u16::from_le_bytes([data[4], data[5]]));
        let mut options = Vec::new();
        let mut offset = 6;

        while offset + 2 <= data.len() {
            let opt_type = L2CapConfigOptType::from_byte(data[offset]);
            let opt_len = data[offset + 1] as usize;
            if offset + 2 + opt_len > data.len() {
                break;
            }
            options.push(L2CapConfigOpt {
                opt_type,
                value: data[offset + 2..offset + 2 + opt_len].to_vec(),
            });
            offset += 2 + opt_len;
        }

        Some(Self {
            source_cid,
            flags,
            result,
            options,
        })
    }
}

// ============================================================================
// Disconnection Request/Response
// ============================================================================

/// L2CAP disconnection request.
#[derive(Clone, Debug)]
pub struct L2CapDisconReq {
    pub destination_cid: u16,
    pub source_cid: u16,
}

impl L2CapDisconReq {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let destination_cid = u16::from_le_bytes([data[0], data[1]]);
        let source_cid = u16::from_le_bytes([data[2], data[3]]);
        Some(Self {
            destination_cid,
            source_cid,
        })
    }

    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4);
        buf.extend_from_slice(&self.destination_cid.to_le_bytes());
        buf.extend_from_slice(&self.source_cid.to_le_bytes());
        buf
    }
}

/// L2CAP disconnection response.
#[derive(Clone, Debug)]
pub struct L2CapDisconRsp {
    pub destination_cid: u16,
    pub source_cid: u16,
}

impl L2CapDisconRsp {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let destination_cid = u16::from_le_bytes([data[0], data[1]]);
        let source_cid = u16::from_le_bytes([data[2], data[3]]);
        Some(Self {
            destination_cid,
            source_cid,
        })
    }
}

// ============================================================================
// Command Reject
// ============================================================================

/// L2CAP command reject reason.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum L2CapRejectReason {
    NotUnderstood = 0x0000,
    SignalingMtuExceeded = 0x0001,
    InvalidCidInRequest = 0x0002,
}

/// L2CAP command reject.
#[derive(Clone, Debug)]
pub struct L2CapCommandReject {
    pub reason: L2CapRejectReason,
    pub data: Vec<u8>,
}

impl L2CapCommandReject {
    pub fn build(reason: L2CapRejectReason, id: u8, cid: u16) -> Vec<u8> {
        let mut data = Vec::with_capacity(4);
        data.extend_from_slice(&(reason as u16).to_le_bytes());
        if reason == L2CapRejectReason::InvalidCidInRequest {
            data.extend_from_slice(&cid.to_le_bytes());
        }
        build_sig_command(L2CapSigCode::CommandReject, id, &data)
    }
}

// ============================================================================
// Channel Manager
// ============================================================================

/// L2CAP channel state.
#[derive(Clone)]
pub struct L2CapChannel {
    /// Connection handle this channel belongs to.
    pub conn_handle: ConnHandle,
    /// Local CID.
    pub local_cid: u16,
    /// Remote CID.
    pub remote_cid: u16,
    /// PSM (protocol/service multiplexer).
    pub psm: L2CapPsm,
    /// Current state.
    pub state: L2CapChannelState,
    /// Local MTU.
    pub local_mtu: u16,
    /// Remote MTU.
    pub remote_mtu: u16,
    /// Signaling identifier for pending commands.
    pub pending_id: u8,
}

impl L2CapChannel {
    pub fn new(conn_handle: ConnHandle, local_cid: u16, psm: L2CapPsm) -> Self {
        Self {
            conn_handle,
            local_cid,
            remote_cid: 0,
            psm,
            state: L2CapChannelState::Closed,
            local_mtu: L2CAP_DEFAULT_MTU,
            remote_mtu: L2CAP_DEFAULT_MTU,
            pending_id: 0,
        }
    }
}

/// L2CAP channel manager — maintains all active channels.
pub struct L2CapChannelManager {
    channels: Vec<L2CapChannel>,
    next_local_cid: u16,
    next_sig_id: u8,
    local_mtu: u16,
}

impl L2CapChannelManager {
    pub fn new() -> Self {
        Self {
            channels: Vec::new(),
            next_local_cid: 0x0040, // Start of dynamic range
            next_sig_id: 1,
            local_mtu: L2CAP_DEFAULT_MTU,
        }
    }

    pub fn set_local_mtu(&mut self, mtu: u16) {
        self.local_mtu = mtu.clamp(L2CAP_MIN_MTU, L2CAP_MAX_MTU);
    }

    pub fn local_mtu(&self) -> u16 {
        self.local_mtu
    }

    /// Allocate the next signaling identifier.
    pub fn alloc_sig_id(&mut self) -> u8 {
        let id = self.next_sig_id;
        self.next_sig_id = self.next_sig_id.wrapping_add(1);
        id
    }

    /// Allocate a local CID.
    pub fn alloc_local_cid(&mut self) -> u16 {
        let cid = self.next_local_cid;
        self.next_local_cid = self.next_local_cid.wrapping_add(1);
        if self.next_local_cid < 0x0040 {
            self.next_local_cid = 0x0040;
        }
        cid
    }

    /// Create a new channel.
    pub fn create_channel(
        &mut self,
        conn_handle: ConnHandle,
        psm: L2CapPsm,
    ) -> &mut L2CapChannel {
        let local_cid = self.alloc_local_cid();
        self.channels.push(L2CapChannel::new(conn_handle, local_cid, psm));
        self.channels.last_mut().unwrap()
    }

    /// Find a channel by local CID.
    pub fn find_by_local_cid(&mut self, cid: u16) -> Option<&mut L2CapChannel> {
        self.channels.iter_mut().find(|ch| ch.local_cid == cid)
    }

    /// Find a channel by remote CID.
    pub fn find_by_remote_cid(&mut self, cid: u16) -> Option<&mut L2CapChannel> {
        self.channels.iter_mut().find(|ch| ch.remote_cid == cid)
    }

    /// Find channels by connection handle.
    pub fn find_by_conn_handle(&mut self, handle: ConnHandle) -> Vec<&mut L2CapChannel> {
        self.channels
            .iter_mut()
            .filter(|ch| ch.conn_handle == handle)
            .collect()
    }

    /// Remove a channel by local CID.
    pub fn remove_channel(&mut self, local_cid: u16) {
        self.channels.retain(|ch| ch.local_cid != local_cid);
    }

    /// Number of active channels.
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}

// ============================================================================
// Information Request Types
// ============================================================================

/// L2CAP InfoType for Information Request.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum L2CapInfoType {
    ConnectionlessMtu = 0x0001,
    ExtendedFeatures = 0x0002,
    FixedChannels = 0x0003,
}

impl L2CapInfoType {
    pub fn from_raw(raw: u16) -> Option<Self> {
        match raw {
            0x0001 => Some(Self::ConnectionlessMtu),
            0x0002 => Some(Self::ExtendedFeatures),
            0x0003 => Some(Self::FixedChannels),
            _ => None,
        }
    }
}

/// L2CAP extended features mask.
pub const L2CAP_EXTF_FIXED_CHANNELS: u32 = 0x00000001;
pub const L2CAP_EXTF_UNICAST_CONNECTIONLESS: u32 = 0x00000002;
pub const L2CAP_EXTF_ENHANCED_RETRANSMISSION: u32 = 0x00000004;
pub const L2CAP_EXTF_STREAMING: u32 = 0x00000008;
pub const L2CAP_EXTF_FCS: u32 = 0x00000010;
pub const L2CAP_EXTF_EXTENDED_FLOW_SPEC: u32 = 0x00000020;
pub const L2CAP_EXTF_FIXED_CHANNELS_LE: u32 = 0x00000040;
pub const L2CAP_EXTF_ENHANCED_CREDIT_BASED_FLOW_CTRL: u32 = 0x00000080;

/// Fixed channels mask (bit per CID).
pub const L2CAP_FIXED_CHAN_SIGNALING: u64 = 1 << 0x0001;
pub const L2CAP_FIXED_CHAN_CONNECTIONLESS: u64 = 1 << 0x0002;
pub const L2CAP_FIXED_CHAN_ATT: u64 = 1 << 0x0004;
pub const L2CAP_FIXED_CHAN_LE_SIGNALING: u64 = 1 << 0x0005;
pub const L2CAP_FIXED_CHAN_LE_SECURITY: u64 = 1 << 0x0006;

// ============================================================================
// Information Request/Response
// ============================================================================

/// L2CAP information request.
#[derive(Clone, Debug)]
pub struct L2CapInfoReq {
    pub info_type: L2CapInfoType,
}

/// L2CAP information response.
#[derive(Clone, Debug)]
pub struct L2CapInfoRsp {
    pub info_type: L2CapInfoType,
    pub result: u16, // 0x0000 = success, 0x0001 = not supported
    pub data: Vec<u8>,
}

// ============================================================================
// LE Credit-Based Connection
// ============================================================================

/// LE Credit-Based Connection Request.
#[derive(Clone, Debug)]
pub struct L2CapLeCreditConnReq {
    pub psm: L2CapPsm,
    pub source_cid: u16,
    pub mtu: u16,
    pub mps: u16, // Maximum PDU size
    pub initial_credits: u16,
}

impl L2CapLeCreditConnReq {
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        let psm = L2CapPsm::from_raw(u16::from_le_bytes([data[0], data[1]]));
        let source_cid = u16::from_le_bytes([data[2], data[3]]);
        let mtu = u16::from_le_bytes([data[4], data[5]]);
        let mps = u16::from_le_bytes([data[6], data[7]]);
        let initial_credits = if data.len() >= 10 {
            u16::from_le_bytes([data[8], data[9]])
        } else {
            0
        };
        Some(Self {
            psm,
            source_cid,
            mtu,
            mps,
            initial_credits,
        })
    }

    pub fn build(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(10);
        buf.extend_from_slice(&self.psm.to_raw().to_le_bytes());
        buf.extend_from_slice(&self.source_cid.to_le_bytes());
        buf.extend_from_slice(&self.mtu.to_le_bytes());
        buf.extend_from_slice(&self.mps.to_le_bytes());
        buf.extend_from_slice(&self.initial_credits.to_le_bytes());
        buf
    }
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ConnHandle;

    #[test]
    fn test_b_frame_roundtrip() {
        let payload = vec![0x01, 0x02, 0x03, 0x04];
        let frame = build_l2cap_b_frame(0x0041, &payload);
        assert_eq!(
            frame,
            vec![0x04, 0x00, 0x41, 0x00, 0x01, 0x02, 0x03, 0x04]
        );

        let (parsed, consumed) = parse_l2cap_b_frame(&frame).unwrap();
        assert_eq!(parsed.cid, 0x0041);
        assert_eq!(parsed.payload, payload);
        assert_eq!(consumed, 8);
    }

    #[test]
    fn test_sig_command_roundtrip() {
        let data = vec![0x40, 0x00, 0x41, 0x00];
        let cmd = build_sig_command(L2CapSigCode::ConnectionRequest, 0x05, &data);
        assert_eq!(cmd[0], 0x02); // Code
        assert_eq!(cmd[1], 0x05); // ID
        assert_eq!(cmd[2], 0x04); // Length LSB
        assert_eq!(cmd[3], 0x00); // Length MSB

        let parsed = parse_sig_commands(&cmd);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].code, L2CapSigCode::ConnectionRequest);
        assert_eq!(parsed[0].id, 0x05);
        assert_eq!(parsed[0].data, data);
    }

    #[test]
    fn test_connection_req_rsp() {
        let req = L2CapConnReq {
            psm: L2CapPsm::Rfcomm,
            source_cid: 0x0041,
        };
        let req_data = req.build();
        assert_eq!(req_data.len(), 4);

        let parsed = L2CapConnReq::parse(&req_data).unwrap();
        assert_eq!(parsed.psm, L2CapPsm::Rfcomm);
        assert_eq!(parsed.source_cid, 0x0041);

        let rsp = L2CapConnRsp {
            destination_cid: 0x0042,
            source_cid: 0x0041,
            result: L2CapConnResult::Success,
            status: 0,
        };
        let rsp_data = rsp.build();
        assert_eq!(rsp_data.len(), 8);

        let parsed_rsp = L2CapConnRsp::parse(&rsp_data).unwrap();
        assert_eq!(parsed_rsp.destination_cid, 0x0042);
        assert_eq!(parsed_rsp.result, L2CapConnResult::Success);
    }

    #[test]
    fn test_config_req_parse() {
        // Build a config request with MTU option
        let cfg_req = L2CapConfigReq {
            destination_cid: 0x0041,
            flags: 0,
            options: vec![L2CapConfigOpt {
                opt_type: L2CapConfigOptType::Mtu,
                value: vec![0x00, 0x02], // MTU = 512 (little-endian)
            }],
        };
        let data = cfg_req.build();
        let parsed = L2CapConfigReq::parse(&data).unwrap();
        assert_eq!(parsed.destination_cid, 0x0041);
        assert_eq!(parsed.options.len(), 1);
        assert_eq!(parsed.options[0].opt_type, L2CapConfigOptType::Mtu);
        assert_eq!(parsed.options[0].value, vec![0x00, 0x02]);
    }

    #[test]
    fn test_discon_req() {
        let req = L2CapDisconReq {
            destination_cid: 0x0042,
            source_cid: 0x0041,
        };
        let data = req.build();
        assert_eq!(data.len(), 4);

        let parsed = L2CapDisconReq::parse(&data).unwrap();
        assert_eq!(parsed.destination_cid, 0x0042);
        assert_eq!(parsed.source_cid, 0x0041);
    }

    #[test]
    fn test_channel_manager() {
        let mut mgr = L2CapChannelManager::new();
        assert_eq!(mgr.channel_count(), 0);

        let ch = mgr.create_channel(ConnHandle::new(0x0001), L2CapPsm::Rfcomm);
        ch.state = L2CapChannelState::Open;
        let ch_cid = ch.local_cid;

        assert_eq!(mgr.channel_count(), 1);
        assert!(ch_cid >= 0x0040);

        let found = mgr.find_by_local_cid(ch_cid);
        assert!(found.is_some());

        mgr.remove_channel(ch_cid);
        assert_eq!(mgr.channel_count(), 0);
    }

    #[test]
    fn test_info_type() {
        assert_eq!(
            L2CapInfoType::from_raw(0x0001),
            Some(L2CapInfoType::ConnectionlessMtu)
        );
        assert_eq!(L2CapInfoType::from_raw(0x0004), None);
    }

    #[test]
    fn test_le_credit_conn() {
        let req = L2CapLeCreditConnReq {
            psm: L2CapPsm::LeCoC,
            source_cid: 0x0041,
            mtu: 512,
            mps: 256,
            initial_credits: 10,
        };
        let data = req.build();
        let parsed = L2CapLeCreditConnReq::parse(&data).unwrap();
        assert_eq!(parsed.psm, L2CapPsm::LeCoC);
        assert_eq!(parsed.mtu, 512);
        assert_eq!(parsed.mps, 256);
        assert_eq!(parsed.initial_credits, 10);
    }

    #[test]
    fn test_sig_id_allocation() {
        let mut mgr = L2CapChannelManager::new();
        let id1 = mgr.alloc_sig_id();
        let id2 = mgr.alloc_sig_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_command_reject_build() {
        let reject = L2CapCommandReject::build(
            L2CapRejectReason::NotUnderstood,
            0x05,
            0,
        );
        assert_eq!(reject[0], 0x01); // CommandReject code
        assert_eq!(reject[1], 0x05); // ID
        // Length field: data = reason(2 bytes) + NO padding for NotUnderstood
        assert_eq!(reject[2], 0x02); // Length LSB = 2
        assert_eq!(reject[3], 0x00); // Length MSB = 0
        assert_eq!(reject.len(), 6); // 4 header + 2 reason data

        // Test InvalidCidInRequest — includes CID in data (4 bytes total)
        let reject2 = L2CapCommandReject::build(
            L2CapRejectReason::InvalidCidInRequest,
            0x06,
            0x0041,
        );
        assert_eq!(reject2.len(), 8); // 4 header + 2 reason + 2 CID
        assert_eq!(reject2[4], 0x02); // reason LSB = 0x0002
        assert_eq!(reject2[5], 0x00); // reason MSB
        assert_eq!(reject2[6], 0x41); // CID LSB = 0x0041
        assert_eq!(reject2[7], 0x00); // CID MSB
    }
}
