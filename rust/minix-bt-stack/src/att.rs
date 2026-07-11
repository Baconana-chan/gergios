//! # ATT — Attribute Protocol
//!
//! The Attribute Protocol (ATT) is the foundation for GATT in Bluetooth LE.
//! It defines a client-server architecture where the server exposes a set of
//! attributes (each with a handle, type UUID, and value) that the client can
//! read, write, discover, and receive notifications/indications from.
//!
//! ## PDU Format
//!
//! ```text
//! | Opcode (1) | Parameters (variable) |
//! ```
//!
//! All multi-byte values are in **little-endian** byte order.

#![allow(dead_code)]

use crate::types::BtUuid;

// ============================================================================
// Constants
// ============================================================================

/// Default ATT MTU (minimum, per Bluetooth spec).
pub const ATT_DEFAULT_MTU: u16 = 23;
/// Maximum ATT MTU.
pub const ATT_MAX_MTU: u16 = 512;
/// Minimum ATT MTU per spec.
pub const ATT_MIN_MTU: u16 = 23;

// ============================================================================
// ATT Opcodes
// ============================================================================

/// ATT Protocol PDU opcodes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum AttOpcode {
    ErrorResponse = 0x01,
    ExchangeMtuRequest = 0x02,
    ExchangeMtuResponse = 0x03,
    FindInformationRequest = 0x04,
    FindInformationResponse = 0x05,
    FindByTypeValueRequest = 0x06,
    FindByTypeValueResponse = 0x07,
    ReadByTypeRequest = 0x08,
    ReadByTypeResponse = 0x09,
    ReadRequest = 0x0A,
    ReadResponse = 0x0B,
    ReadBlobRequest = 0x0C,
    ReadBlobResponse = 0x0D,
    ReadMultipleRequest = 0x0E,
    ReadMultipleResponse = 0x0F,
    ReadByGroupTypeRequest = 0x10,
    ReadByGroupTypeResponse = 0x11,
    WriteRequest = 0x12,
    WriteResponse = 0x13,
    WriteCommand = 0x52,
    SignedWriteCommand = 0xD2,
    PrepareWriteRequest = 0x16,
    PrepareWriteResponse = 0x17,
    ExecuteWriteRequest = 0x18,
    ExecuteWriteResponse = 0x19,
    HandleValueNotification = 0x1B,
    HandleValueIndication = 0x1D,
    HandleValueConfirmation = 0x1E,
}

impl AttOpcode {
    pub fn from_byte(b: u8) -> Option<Self> {
        use AttOpcode::*;
        Some(match b {
            0x01 => ErrorResponse,
            0x02 => ExchangeMtuRequest,
            0x03 => ExchangeMtuResponse,
            0x04 => FindInformationRequest,
            0x05 => FindInformationResponse,
            0x06 => FindByTypeValueRequest,
            0x07 => FindByTypeValueResponse,
            0x08 => ReadByTypeRequest,
            0x09 => ReadByTypeResponse,
            0x0A => ReadRequest,
            0x0B => ReadResponse,
            0x0C => ReadBlobRequest,
            0x0D => ReadBlobResponse,
            0x0E => ReadMultipleRequest,
            0x0F => ReadMultipleResponse,
            0x10 => ReadByGroupTypeRequest,
            0x11 => ReadByGroupTypeResponse,
            0x12 => WriteRequest,
            0x13 => WriteResponse,
            0x52 => WriteCommand,
            0xD2 => SignedWriteCommand,
            0x16 => PrepareWriteRequest,
            0x17 => PrepareWriteResponse,
            0x18 => ExecuteWriteRequest,
            0x19 => ExecuteWriteResponse,
            0x1B => HandleValueNotification,
            0x1D => HandleValueIndication,
            0x1E => HandleValueConfirmation,
            _ => return None,
        })
    }
}

// ============================================================================
// Error Response
// ============================================================================

/// ATT error codes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum AttErrorCode {
    InvalidHandle = 0x01,
    ReadNotPermitted = 0x02,
    WriteNotPermitted = 0x03,
    InvalidPdu = 0x04,
    InsufficientAuthentication = 0x05,
    RequestNotSupported = 0x06,
    InvalidOffset = 0x07,
    InsufficientAuthorization = 0x08,
    PrepareQueueFull = 0x09,
    AttributeNotFound = 0x0A,
    AttributeNotLong = 0x0B,
    InsufficientEncryptionKeySize = 0x0C,
    InvalidAttributeValueLength = 0x0D,
    UnlikelyError = 0x0E,
    InsufficientEncryption = 0x0F,
    UnsupportedGroupType = 0x10,
    InsufficientResources = 0x11,
}

impl AttErrorCode {
    pub fn from_byte(b: u8) -> Option<Self> {
        use AttErrorCode::*;
        Some(match b {
            0x01 => InvalidHandle,
            0x02 => ReadNotPermitted,
            0x03 => WriteNotPermitted,
            0x04 => InvalidPdu,
            0x05 => InsufficientAuthentication,
            0x06 => RequestNotSupported,
            0x07 => InvalidOffset,
            0x08 => InsufficientAuthorization,
            0x09 => PrepareQueueFull,
            0x0A => AttributeNotFound,
            0x0B => AttributeNotLong,
            0x0C => InsufficientEncryptionKeySize,
            0x0D => InvalidAttributeValueLength,
            0x0E => UnlikelyError,
            0x0F => InsufficientEncryption,
            0x10 => UnsupportedGroupType,
            0x11 => InsufficientResources,
            _ => return None,
        })
    }
}

/// ATT Error Response PDU.
///
/// ```text
/// | Opcode (1) | Request Opcode (1) | Handle (2) | Error Code (1) |
/// ```
#[derive(Clone, Debug)]
pub struct AttErrorRsp {
    pub request_opcode: u8,
    pub handle: u16,
    pub error_code: AttErrorCode,
}

impl AttErrorRsp {
    pub fn build(req_opcode: u8, handle: u16, error: AttErrorCode) -> Vec<u8> {
        let mut buf = Vec::with_capacity(5);
        buf.push(AttOpcode::ErrorResponse as u8);
        buf.push(req_opcode);
        buf.extend_from_slice(&handle.to_le_bytes());
        buf.push(error as u8);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }
        let request_opcode = data[1];
        let handle = u16::from_le_bytes([data[2], data[3]]);
        let error_code = AttErrorCode::from_byte(data[4])?;
        Some(Self {
            request_opcode,
            handle,
            error_code,
        })
    }
}

// ============================================================================
// Exchange MTU
// ============================================================================

/// Exchange MTU Request.
///
/// ```text
/// | Opcode (1) | Client RX MTU (2) |
/// ```
#[derive(Clone, Debug)]
pub struct AttExchangeMtuReq {
    pub client_rx_mtu: u16,
}

impl AttExchangeMtuReq {
    pub fn build(client_rx_mtu: u16) -> Vec<u8> {
        let mtu = client_rx_mtu.max(ATT_MIN_MTU).min(ATT_MAX_MTU);
        let mut buf = Vec::with_capacity(3);
        buf.push(AttOpcode::ExchangeMtuRequest as u8);
        buf.extend_from_slice(&mtu.to_le_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 3 {
            return None;
        }
        let client_rx_mtu = u16::from_le_bytes([data[1], data[2]]);
        Some(Self { client_rx_mtu })
    }
}

/// Exchange MTU Response.
///
/// ```text
/// | Opcode (1) | Server RX MTU (2) |
/// ```
#[derive(Clone, Debug)]
pub struct AttExchangeMtuRsp {
    pub server_rx_mtu: u16,
}

impl AttExchangeMtuRsp {
    pub fn build(server_rx_mtu: u16) -> Vec<u8> {
        let mtu = server_rx_mtu.max(ATT_MIN_MTU).min(ATT_MAX_MTU);
        let mut buf = Vec::with_capacity(3);
        buf.push(AttOpcode::ExchangeMtuResponse as u8);
        buf.extend_from_slice(&mtu.to_le_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 3 {
            return None;
        }
        let server_rx_mtu = u16::from_le_bytes([data[1], data[2]]);
        Some(Self { server_rx_mtu })
    }
}

// ============================================================================
// Find Information
// ============================================================================

/// Find Information Request.
///
/// ```text
/// | Opcode (1) | Start Handle (2) | End Handle (2) |
/// ```
#[derive(Clone, Debug)]
pub struct AttFindInfoReq {
    pub start_handle: u16,
    pub end_handle: u16,
}

impl AttFindInfoReq {
    pub fn build(start_handle: u16, end_handle: u16) -> Vec<u8> {
        let mut buf = Vec::with_capacity(5);
        buf.push(AttOpcode::FindInformationRequest as u8);
        buf.extend_from_slice(&start_handle.to_le_bytes());
        buf.extend_from_slice(&end_handle.to_le_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }
        let start_handle = u16::from_le_bytes([data[1], data[2]]);
        let end_handle = u16::from_le_bytes([data[3], data[4]]);
        Some(Self {
            start_handle,
            end_handle,
        })
    }
}

/// Format of UUID in Find Information Response.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum AttFindInfoFormat {
    /// 16-bit UUIDs — each entry is handle(2) + uuid16(2).
    Uuid16 = 0x01,
    /// 128-bit UUIDs — each entry is handle(2) + uuid128(16).
    Uuid128 = 0x02,
}

/// A handle-UUID pair for Find Information Response.
#[derive(Clone, Debug)]
pub struct AttHandleUuidPair {
    pub handle: u16,
    pub uuid: BtUuid,
}

/// Find Information Response.
///
/// ```text
/// | Opcode (1) | Format (1) | Handle-UUID pairs ... |
/// ```
#[derive(Clone, Debug)]
pub struct AttFindInfoRsp {
    pub pairs: Vec<AttHandleUuidPair>,
}

impl AttFindInfoRsp {
    pub fn build(pairs: Vec<AttHandleUuidPair>) -> Vec<u8> {
        let format = pairs
            .first()
            .map(|p| match p.uuid.uuid_type() {
                crate::types::BtUuidType::Uuid16 => AttFindInfoFormat::Uuid16,
                _ => AttFindInfoFormat::Uuid128,
            })
            .unwrap_or(AttFindInfoFormat::Uuid16);

        let entry_size = match format {
            AttFindInfoFormat::Uuid16 => 4, // handle(2) + uuid16(2)
            AttFindInfoFormat::Uuid128 => 18, // handle(2) + uuid128(16)
        };

        // Filter pairs to match the format, starting from the first pair's format
        let uuid_type = pairs
            .first()
            .map(|p| p.uuid.uuid_type())
            .unwrap_or(crate::types::BtUuidType::Uuid16);

        let mut buf = Vec::with_capacity(2 + pairs.len() * entry_size);
        buf.push(AttOpcode::FindInformationResponse as u8);
        buf.push(format as u8);

        for pair in &pairs {
            if pair.uuid.uuid_type() == uuid_type {
                buf.extend_from_slice(&pair.handle.to_le_bytes());
                match uuid_type {
                    crate::types::BtUuidType::Uuid16 => {
                        buf.extend_from_slice(&pair.uuid.bytes[2..4]);
                    }
                    _ => {
                        buf.extend_from_slice(&pair.uuid.bytes);
                    }
                }
            }
        }
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        let format = match data[1] {
            0x01 => AttFindInfoFormat::Uuid16,
            0x02 => AttFindInfoFormat::Uuid128,
            _ => return None,
        };
        let entry_size = match format {
            AttFindInfoFormat::Uuid16 => 4,
            AttFindInfoFormat::Uuid128 => 18,
        };
        let payload = &data[2..];
        if payload.len() % entry_size != 0 {
            return None;
        }
        let mut pairs = Vec::with_capacity(payload.len() / entry_size);
        for chunk in payload.chunks(entry_size) {
            let handle = u16::from_le_bytes([chunk[0], chunk[1]]);
            let uuid = match format {
                AttFindInfoFormat::Uuid16 => {
                    let mut uuid_bytes = crate::types::BLUETOOTH_BASE_UUID;
                    uuid_bytes[2] = chunk[2];
                    uuid_bytes[3] = chunk[3];
                    BtUuid::from_bytes(uuid_bytes)
                }
                AttFindInfoFormat::Uuid128 => {
                    let mut uuid_bytes = [0u8; 16];
                    uuid_bytes.copy_from_slice(&chunk[2..18]);
                    BtUuid::from_bytes(uuid_bytes)
                }
            };
            pairs.push(AttHandleUuidPair { handle, uuid });
        }
        Some(Self { pairs })
    }
}

// ============================================================================
// Read By Group Type (used for Primary Service discovery)
// ============================================================================

/// Read By Group Type Request.
///
/// ```text
/// | Opcode (1) | Start Handle (2) | End Handle (2) | Group Type UUID (2 or 16) |
/// ```
#[derive(Clone, Debug)]
pub struct AttReadByGroupTypeReq {
    pub start_handle: u16,
    pub end_handle: u16,
    pub group_type: BtUuid,
}

impl AttReadByGroupTypeReq {
    pub fn build(start_handle: u16, end_handle: u16, group_type: BtUuid) -> Vec<u8> {
        let uuid_type = group_type.uuid_type();
        let uuid_len = match uuid_type {
            crate::types::BtUuidType::Uuid16 => 2,
            _ => 16,
        };
        let mut buf = Vec::with_capacity(5 + uuid_len);
        buf.push(AttOpcode::ReadByGroupTypeRequest as u8);
        buf.extend_from_slice(&start_handle.to_le_bytes());
        buf.extend_from_slice(&end_handle.to_le_bytes());
        match uuid_type {
            crate::types::BtUuidType::Uuid16 => {
                buf.extend_from_slice(&group_type.bytes[2..4]);
            }
            _ => {
                buf.extend_from_slice(&group_type.bytes);
            }
        }
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }
        let start_handle = u16::from_le_bytes([data[1], data[2]]);
        let end_handle = u16::from_le_bytes([data[3], data[4]]);
        let remaining = &data[5..];
        let group_type = match remaining.len() {
            2 => {
                let mut uuid_bytes = crate::types::BLUETOOTH_BASE_UUID;
                uuid_bytes[2] = remaining[0];
                uuid_bytes[3] = remaining[1];
                BtUuid::from_bytes(uuid_bytes)
            }
            16 => {
                let mut uuid_bytes = [0u8; 16];
                uuid_bytes.copy_from_slice(&remaining[..16]);
                BtUuid::from_bytes(uuid_bytes)
            }
            _ => return None,
        };
        Some(Self {
            start_handle,
            end_handle,
            group_type,
        })
    }
}

/// A group attribute entry (for Read By Group Type Response).
#[derive(Clone, Debug)]
pub struct AttGroupAttrEntry {
    pub start_handle: u16,
    pub group_end_handle: u16,
    pub value: Vec<u8>,
}

/// Read By Group Type Response.
///
/// ```text
/// | Opcode (1) | Length (1) | Attribute Data ... |
/// ```
#[derive(Clone, Debug)]
pub struct AttReadByGroupTypeRsp {
    pub entries: Vec<AttGroupAttrEntry>,
}

impl AttReadByGroupTypeRsp {
    pub fn build(entries: Vec<AttGroupAttrEntry>) -> Vec<u8> {
        if entries.is_empty() {
            return vec![AttOpcode::ReadByGroupTypeResponse as u8, 0];
        }
        let entry_size = 4 + entries[0].value.len(); // start_handle(2) + end_handle(2) + value
        let mut buf = Vec::with_capacity(2 + entries.len() * entry_size);
        buf.push(AttOpcode::ReadByGroupTypeResponse as u8);
        buf.push(entry_size as u8);
        for entry in &entries {
            buf.extend_from_slice(&entry.start_handle.to_le_bytes());
            buf.extend_from_slice(&entry.group_end_handle.to_le_bytes());
            buf.extend_from_slice(&entry.value);
        }
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        let opcode = data[0];
        if opcode != AttOpcode::ReadByGroupTypeResponse as u8 {
            return None;
        }
        let entry_size = data[1] as usize;
        if entry_size < 4 {
            return None;
        }
        let payload = &data[2..];
        if payload.len() % entry_size != 0 {
            return None;
        }
        let mut entries = Vec::with_capacity(payload.len() / entry_size);
        for chunk in payload.chunks(entry_size) {
            let start_handle = u16::from_le_bytes([chunk[0], chunk[1]]);
            let group_end_handle = u16::from_le_bytes([chunk[2], chunk[3]]);
            let value = chunk[4..].to_vec();
            entries.push(AttGroupAttrEntry {
                start_handle,
                group_end_handle,
                value,
            });
        }
        Some(Self { entries })
    }
}

// ============================================================================
// Read By Type
// ============================================================================

/// Read By Type Request.
///
/// ```text
/// | Opcode (1) | Start Handle (2) | End Handle (2) | Attribute Type UUID (2 or 16) |
/// ```
#[derive(Clone, Debug)]
pub struct AttReadByTypeReq {
    pub start_handle: u16,
    pub end_handle: u16,
    pub attr_type: BtUuid,
}

impl AttReadByTypeReq {
    pub fn build(start_handle: u16, end_handle: u16, attr_type: BtUuid) -> Vec<u8> {
        let uuid_type = attr_type.uuid_type();
        let uuid_len = match uuid_type {
            crate::types::BtUuidType::Uuid16 => 2,
            _ => 16,
        };
        let mut buf = Vec::with_capacity(5 + uuid_len);
        buf.push(AttOpcode::ReadByTypeRequest as u8);
        buf.extend_from_slice(&start_handle.to_le_bytes());
        buf.extend_from_slice(&end_handle.to_le_bytes());
        match uuid_type {
            crate::types::BtUuidType::Uuid16 => {
                buf.extend_from_slice(&attr_type.bytes[2..4]);
            }
            _ => {
                buf.extend_from_slice(&attr_type.bytes);
            }
        }
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }
        let start_handle = u16::from_le_bytes([data[1], data[2]]);
        let end_handle = u16::from_le_bytes([data[3], data[4]]);
        let remaining = &data[5..];
        let attr_type = match remaining.len() {
            2 => {
                let mut uuid_bytes = crate::types::BLUETOOTH_BASE_UUID;
                uuid_bytes[2] = remaining[0];
                uuid_bytes[3] = remaining[1];
                BtUuid::from_bytes(uuid_bytes)
            }
            16 => {
                let mut uuid_bytes = [0u8; 16];
                uuid_bytes.copy_from_slice(&remaining[..16]);
                BtUuid::from_bytes(uuid_bytes)
            }
            _ => return None,
        };
        Some(Self {
            start_handle,
            end_handle,
            attr_type,
        })
    }
}

/// Read By Type Response (list of handle-value pairs).
///
/// ```text
/// | Opcode (1) | Length (1) | Handle (2) | Value ... |
/// ```
#[derive(Clone, Debug)]
pub struct AttReadByTypeRsp {
    pub handle_value_pairs: Vec<(u16, Vec<u8>)>,
}

impl AttReadByTypeRsp {
    pub fn build(pairs: Vec<(u16, Vec<u8>)>) -> Vec<u8> {
        if pairs.is_empty() {
            return vec![AttOpcode::ReadByTypeResponse as u8, 0];
        }
        let entry_size = 2 + pairs[0].1.len();
        let mut buf = Vec::with_capacity(2 + pairs.len() * entry_size);
        buf.push(AttOpcode::ReadByTypeResponse as u8);
        buf.push(entry_size as u8);
        for (handle, value) in &pairs {
            buf.extend_from_slice(&handle.to_le_bytes());
            buf.extend_from_slice(value);
        }
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        let opcode = data[0];
        if opcode != AttOpcode::ReadByTypeResponse as u8 {
            return None;
        }
        let entry_size = data[1] as usize;
        if entry_size < 2 {
            return None;
        }
        let payload = &data[2..];
        if payload.len() % entry_size != 0 {
            return None;
        }
        let mut pairs = Vec::with_capacity(payload.len() / entry_size);
        for chunk in payload.chunks(entry_size) {
            let handle = u16::from_le_bytes([chunk[0], chunk[1]]);
            let value = chunk[2..].to_vec();
            pairs.push((handle, value));
        }
        Some(Self {
            handle_value_pairs: pairs,
        })
    }
}

// ============================================================================
// Read Request / Response
// ============================================================================

/// Read Request.
///
/// ```text
/// | Opcode (1) | Attribute Handle (2) |
/// ```
#[derive(Clone, Debug)]
pub struct AttReadReq {
    pub handle: u16,
}

impl AttReadReq {
    pub fn build(handle: u16) -> Vec<u8> {
        let mut buf = Vec::with_capacity(3);
        buf.push(AttOpcode::ReadRequest as u8);
        buf.extend_from_slice(&handle.to_le_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 3 {
            return None;
        }
        let handle = u16::from_le_bytes([data[1], data[2]]);
        Some(Self { handle })
    }
}

/// Read Response.
///
/// ```text
/// | Opcode (1) | Attribute Value (variable) |
/// ```
#[derive(Clone, Debug)]
pub struct AttReadRsp {
    pub value: Vec<u8>,
}

impl AttReadRsp {
    pub fn build(value: Vec<u8>) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + value.len());
        buf.push(AttOpcode::ReadResponse as u8);
        buf.extend_from_slice(&value);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 1 {
            return None;
        }
        Some(Self {
            value: data[1..].to_vec(),
        })
    }
}

// ============================================================================
// Read Blob Request / Response
// ============================================================================

/// Read Blob Request.
///
/// ```text
/// | Opcode (1) | Attribute Handle (2) | Offset (2) |
/// ```
#[derive(Clone, Debug)]
pub struct AttReadBlobReq {
    pub handle: u16,
    pub offset: u16,
}

impl AttReadBlobReq {
    pub fn build(handle: u16, offset: u16) -> Vec<u8> {
        let mut buf = Vec::with_capacity(5);
        buf.push(AttOpcode::ReadBlobRequest as u8);
        buf.extend_from_slice(&handle.to_le_bytes());
        buf.extend_from_slice(&offset.to_le_bytes());
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }
        let handle = u16::from_le_bytes([data[1], data[2]]);
        let offset = u16::from_le_bytes([data[3], data[4]]);
        Some(Self { handle, offset })
    }
}

/// Read Blob Response.
///
/// ```text
/// | Opcode (1) | Attribute Value (variable) |
/// ```
#[derive(Clone, Debug)]
pub struct AttReadBlobRsp {
    pub value: Vec<u8>,
}

impl AttReadBlobRsp {
    pub fn build(value: Vec<u8>) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + value.len());
        buf.push(AttOpcode::ReadBlobResponse as u8);
        buf.extend_from_slice(&value);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 1 {
            return None;
        }
        Some(Self {
            value: data[1..].to_vec(),
        })
    }
}

// ============================================================================
// Write Request / Response
// ============================================================================

/// Write Request.
///
/// ```text
/// | Opcode (1) | Attribute Handle (2) | Value (variable) |
/// ```
#[derive(Clone, Debug)]
pub struct AttWriteReq {
    pub handle: u16,
    pub value: Vec<u8>,
}

impl AttWriteReq {
    pub fn build(handle: u16, value: Vec<u8>) -> Vec<u8> {
        let mut buf = Vec::with_capacity(3 + value.len());
        buf.push(AttOpcode::WriteRequest as u8);
        buf.extend_from_slice(&handle.to_le_bytes());
        buf.extend_from_slice(&value);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 3 {
            return None;
        }
        let handle = u16::from_le_bytes([data[1], data[2]]);
        let value = data[3..].to_vec();
        Some(Self { handle, value })
    }
}

/// Write Response (empty, just opcode).
pub fn build_write_rsp() -> Vec<u8> {
    vec![AttOpcode::WriteResponse as u8]
}

// ============================================================================
// Write Command
// ============================================================================

/// Write Command (no response).
///
/// ```text
/// | Opcode (1) | Attribute Handle (2) | Value (variable) |
/// ```
#[derive(Clone, Debug)]
pub struct AttWriteCmd {
    pub handle: u16,
    pub value: Vec<u8>,
}

impl AttWriteCmd {
    pub fn build(handle: u16, value: Vec<u8>) -> Vec<u8> {
        let mut buf = Vec::with_capacity(3 + value.len());
        buf.push(AttOpcode::WriteCommand as u8);
        buf.extend_from_slice(&handle.to_le_bytes());
        buf.extend_from_slice(&value);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 3 {
            return None;
        }
        let handle = u16::from_le_bytes([data[1], data[2]]);
        let value = data[3..].to_vec();
        Some(Self { handle, value })
    }
}

// ============================================================================
// Handle Value Notification / Indication
// ============================================================================

/// Handle Value Notification.
///
/// ```text
/// | Opcode (1) | Handle (2) | Value (variable) |
/// ```
#[derive(Clone, Debug)]
pub struct AttHandleValueNtf {
    pub handle: u16,
    pub value: Vec<u8>,
}

impl AttHandleValueNtf {
    pub fn build(handle: u16, value: Vec<u8>) -> Vec<u8> {
        let mut buf = Vec::with_capacity(3 + value.len());
        buf.push(AttOpcode::HandleValueNotification as u8);
        buf.extend_from_slice(&handle.to_le_bytes());
        buf.extend_from_slice(&value);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 3 {
            return None;
        }
        let handle = u16::from_le_bytes([data[1], data[2]]);
        let value = data[3..].to_vec();
        Some(Self { handle, value })
    }
}

/// Handle Value Indication.
///
/// ```text
/// | Opcode (1) | Handle (2) | Value (variable) |
/// ```
#[derive(Clone, Debug)]
pub struct AttHandleValueInd {
    pub handle: u16,
    pub value: Vec<u8>,
}

impl AttHandleValueInd {
    pub fn build(handle: u16, value: Vec<u8>) -> Vec<u8> {
        let mut buf = Vec::with_capacity(3 + value.len());
        buf.push(AttOpcode::HandleValueIndication as u8);
        buf.extend_from_slice(&handle.to_le_bytes());
        buf.extend_from_slice(&value);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 3 {
            return None;
        }
        let handle = u16::from_le_bytes([data[1], data[2]]);
        let value = data[3..].to_vec();
        Some(Self { handle, value })
    }
}

/// Handle Value Confirmation (empty, just opcode).
pub fn build_handle_value_cfm() -> Vec<u8> {
    vec![AttOpcode::HandleValueConfirmation as u8]
}

// ============================================================================
// Find By Type Value (used for service discovery by UUID)
// ============================================================================

/// Find By Type Value Request.
///
/// ```text
/// | Opcode (1) | Start Handle (2) | End Handle (2) | Attribute Type (2) | Value (variable) |
/// ```
#[derive(Clone, Debug)]
pub struct AttFindByTypeValueReq {
    pub start_handle: u16,
    pub end_handle: u16,
    pub attr_type: BtUuid,
    pub value: Vec<u8>,
}

impl AttFindByTypeValueReq {
    pub fn build(
        start_handle: u16,
        end_handle: u16,
        attr_type: BtUuid,
        value: Vec<u8>,
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(7 + value.len());
        buf.push(AttOpcode::FindByTypeValueRequest as u8);
        buf.extend_from_slice(&start_handle.to_le_bytes());
        buf.extend_from_slice(&end_handle.to_le_bytes());
        // Attribute type is always 16-bit in this PDU
        buf.extend_from_slice(&attr_type.bytes[2..4]);
        buf.extend_from_slice(&value);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 7 {
            return None;
        }
        let start_handle = u16::from_le_bytes([data[1], data[2]]);
        let end_handle = u16::from_le_bytes([data[3], data[4]]);
        let mut uuid_bytes = crate::types::BLUETOOTH_BASE_UUID;
        uuid_bytes[2] = data[5];
        uuid_bytes[3] = data[6];
        let attr_type = BtUuid::from_bytes(uuid_bytes);
        let value = data[7..].to_vec();
        Some(Self {
            start_handle,
            end_handle,
            attr_type,
            value,
        })
    }
}

/// Handle range entry for Find By Type Value Response.
#[derive(Clone, Debug)]
pub struct AttHandleRange {
    pub start_handle: u16,
    pub end_handle: u16,
}

/// Find By Type Value Response.
///
/// ```text
/// | Opcode (1) | Handle Range(s): start(2) + end(2) ... |
/// ```
#[derive(Clone, Debug)]
pub struct AttFindByTypeValueRsp {
    pub ranges: Vec<AttHandleRange>,
}

impl AttFindByTypeValueRsp {
    pub fn build(ranges: Vec<AttHandleRange>) -> Vec<u8> {
        let mut buf = Vec::with_capacity(1 + ranges.len() * 4);
        buf.push(AttOpcode::FindByTypeValueResponse as u8);
        for range in &ranges {
            buf.extend_from_slice(&range.start_handle.to_le_bytes());
            buf.extend_from_slice(&range.end_handle.to_le_bytes());
        }
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 1 {
            return None;
        }
        let payload = &data[1..];
        if payload.len() % 4 != 0 {
            return None;
        }
        let mut ranges = Vec::with_capacity(payload.len() / 4);
        for chunk in payload.chunks(4) {
            ranges.push(AttHandleRange {
                start_handle: u16::from_le_bytes([chunk[0], chunk[1]]),
                end_handle: u16::from_le_bytes([chunk[2], chunk[3]]),
            });
        }
        Some(Self { ranges })
    }
}

// ============================================================================
// Prepare Write & Execute Write (for long writes)
// ============================================================================

/// Prepare Write Request.
///
/// ```text
/// | Opcode (1) | Handle (2) | Offset (2) | Value (variable) |
/// ```
#[derive(Clone, Debug)]
pub struct AttPrepareWriteReq {
    pub handle: u16,
    pub offset: u16,
    pub value: Vec<u8>,
}

impl AttPrepareWriteReq {
    pub fn build(handle: u16, offset: u16, value: Vec<u8>) -> Vec<u8> {
        let mut buf = Vec::with_capacity(5 + value.len());
        buf.push(AttOpcode::PrepareWriteRequest as u8);
        buf.extend_from_slice(&handle.to_le_bytes());
        buf.extend_from_slice(&offset.to_le_bytes());
        buf.extend_from_slice(&value);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }
        let handle = u16::from_le_bytes([data[1], data[2]]);
        let offset = u16::from_le_bytes([data[3], data[4]]);
        let value = data[5..].to_vec();
        Some(Self {
            handle,
            offset,
            value,
        })
    }
}

/// Prepare Write Response.
#[derive(Clone, Debug)]
pub struct AttPrepareWriteRsp {
    pub handle: u16,
    pub offset: u16,
    pub value: Vec<u8>,
}

impl AttPrepareWriteRsp {
    pub fn build(handle: u16, offset: u16, value: Vec<u8>) -> Vec<u8> {
        let mut buf = Vec::with_capacity(5 + value.len());
        buf.push(AttOpcode::PrepareWriteResponse as u8);
        buf.extend_from_slice(&handle.to_le_bytes());
        buf.extend_from_slice(&offset.to_le_bytes());
        buf.extend_from_slice(&value);
        buf
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }
        let handle = u16::from_le_bytes([data[1], data[2]]);
        let offset = u16::from_le_bytes([data[3], data[4]]);
        let value = data[5..].to_vec();
        Some(Self {
            handle,
            offset,
            value,
        })
    }
}

/// Execute Write Request.
///
/// ```text
/// | Opcode (1) | Flags (1) — 0x00 = Cancel, 0x01 = Write |
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AttExecWriteFlag {
    Cancel = 0x00,
    Write = 0x01,
}

#[derive(Clone, Debug)]
pub struct AttExecuteWriteReq {
    pub flag: AttExecWriteFlag,
}

impl AttExecuteWriteReq {
    pub fn build(flag: AttExecWriteFlag) -> Vec<u8> {
        vec![AttOpcode::ExecuteWriteRequest as u8, flag as u8]
    }

    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 2 {
            return None;
        }
        let flag = match data[1] {
            0x00 => AttExecWriteFlag::Cancel,
            0x01 => AttExecWriteFlag::Write,
            _ => return None,
        };
        Some(Self { flag })
    }
}

/// Execute Write Response (empty).
pub fn build_execute_write_rsp() -> Vec<u8> {
    vec![AttOpcode::ExecuteWriteResponse as u8]
}

// ============================================================================
// ATT PDU dispatch — decode any incoming ATT PDU
// ============================================================================

/// Dispatched ATT PDU — one variant per opcode.
#[derive(Clone, Debug)]
pub enum AttPdu {
    ErrorRsp(AttErrorRsp),
    ExchangeMtuReq(AttExchangeMtuReq),
    ExchangeMtuRsp(AttExchangeMtuRsp),
    FindInfoReq(AttFindInfoReq),
    FindInfoRsp(AttFindInfoRsp),
    FindByTypeValueReq(AttFindByTypeValueReq),
    FindByTypeValueRsp(AttFindByTypeValueRsp),
    ReadByTypeReq(AttReadByTypeReq),
    ReadByTypeRsp(AttReadByTypeRsp),
    ReadByGroupTypeReq(AttReadByGroupTypeReq),
    ReadByGroupTypeRsp(AttReadByGroupTypeRsp),
    ReadReq(AttReadReq),
    ReadRsp(AttReadRsp),
    ReadBlobReq(AttReadBlobReq),
    ReadBlobRsp(AttReadBlobRsp),
    WriteReq(AttWriteReq),
    WriteRsp,
    WriteCmd(AttWriteCmd),
    PrepareWriteReq(AttPrepareWriteReq),
    PrepareWriteRsp(AttPrepareWriteRsp),
    ExecuteWriteReq(AttExecuteWriteReq),
    ExecuteWriteRsp,
    HandleValueNtf(AttHandleValueNtf),
    HandleValueInd(AttHandleValueInd),
    HandleValueCfm,
    /// Known but unsupported PDU — preserves the opcode.
    Unsupported(u8),
}

/// Parse any ATT PDU from raw bytes.
pub fn parse_att_pdu(data: &[u8]) -> Option<AttPdu> {
    if data.is_empty() {
        return None;
    }
    let opcode = AttOpcode::from_byte(data[0])?;
    use AttOpcode::*;
    Some(match opcode {
        ErrorResponse => AttPdu::ErrorRsp(AttErrorRsp::parse(data)?),
        ExchangeMtuRequest => AttPdu::ExchangeMtuReq(AttExchangeMtuReq::parse(data)?),
        ExchangeMtuResponse => AttPdu::ExchangeMtuRsp(AttExchangeMtuRsp::parse(data)?),
        FindInformationRequest => AttPdu::FindInfoReq(AttFindInfoReq::parse(data)?),
        FindInformationResponse => AttPdu::FindInfoRsp(AttFindInfoRsp::parse(data)?),
        FindByTypeValueRequest => AttPdu::FindByTypeValueReq(AttFindByTypeValueReq::parse(data)?),
        FindByTypeValueResponse => {
            AttPdu::FindByTypeValueRsp(AttFindByTypeValueRsp::parse(data)?)
        }
        ReadByTypeRequest => AttPdu::ReadByTypeReq(AttReadByTypeReq::parse(data)?),
        ReadByTypeResponse => AttPdu::ReadByTypeRsp(AttReadByTypeRsp::parse(data)?),
        ReadByGroupTypeRequest => {
            AttPdu::ReadByGroupTypeReq(AttReadByGroupTypeReq::parse(data)?)
        }
        ReadByGroupTypeResponse => {
            AttPdu::ReadByGroupTypeRsp(AttReadByGroupTypeRsp::parse(data)?)
        }
        ReadRequest => AttPdu::ReadReq(AttReadReq::parse(data)?),
        ReadResponse => AttPdu::ReadRsp(AttReadRsp::parse(data)?),
        ReadBlobRequest => AttPdu::ReadBlobReq(AttReadBlobReq::parse(data)?),
        ReadBlobResponse => AttPdu::ReadBlobRsp(AttReadBlobRsp::parse(data)?),
        WriteRequest => AttPdu::WriteReq(AttWriteReq::parse(data)?),
        WriteResponse => AttPdu::WriteRsp,
        WriteCommand => AttPdu::WriteCmd(AttWriteCmd::parse(data)?),
        // Known but unsupported — just preserve the opcode
        ReadMultipleRequest => AttPdu::Unsupported(data[0]),
        ReadMultipleResponse => AttPdu::Unsupported(data[0]),
        SignedWriteCommand => AttPdu::Unsupported(data[0]),
        PrepareWriteRequest => AttPdu::PrepareWriteReq(AttPrepareWriteReq::parse(data)?),
        PrepareWriteResponse => AttPdu::PrepareWriteRsp(AttPrepareWriteRsp::parse(data)?),
        ExecuteWriteRequest => AttPdu::ExecuteWriteReq(AttExecuteWriteReq::parse(data)?),
        ExecuteWriteResponse => AttPdu::ExecuteWriteRsp,
        HandleValueNotification => AttPdu::HandleValueNtf(AttHandleValueNtf::parse(data)?),
        HandleValueIndication => AttPdu::HandleValueInd(AttHandleValueInd::parse(data)?),
        HandleValueConfirmation => AttPdu::HandleValueCfm,
    })
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── Error Response ──

    #[test]
    fn test_error_rsp_roundtrip() {
        let raw = AttErrorRsp::build(
            AttOpcode::ReadRequest as u8,
            0x0042,
            AttErrorCode::InvalidHandle,
        );
        assert_eq!(raw.len(), 5);
        assert_eq!(raw[0], 0x01); // ErrorResponse opcode

        let parsed = AttErrorRsp::parse(&raw).unwrap();
        assert_eq!(parsed.request_opcode, AttOpcode::ReadRequest as u8);
        assert_eq!(parsed.handle, 0x0042);
        assert_eq!(parsed.error_code, AttErrorCode::InvalidHandle);
    }

    // ── Exchange MTU ──

    #[test]
    fn test_exchange_mtu_roundtrip() {
        let raw_req = AttExchangeMtuReq::build(256);
        assert_eq!(raw_req[0], 0x02);
        let req = AttExchangeMtuReq::parse(&raw_req).unwrap();
        assert_eq!(req.client_rx_mtu, 256);

        let raw_rsp = AttExchangeMtuRsp::build(128);
        assert_eq!(raw_rsp[0], 0x03);
        let rsp = AttExchangeMtuRsp::parse(&raw_rsp).unwrap();
        assert_eq!(rsp.server_rx_mtu, 128);
    }

    // ── Find Information ──

    #[test]
    fn test_find_info_req_roundtrip() {
        let raw = AttFindInfoReq::build(0x0001, 0x00FF);
        assert_eq!(raw[0], 0x04);
        let req = AttFindInfoReq::parse(&raw).unwrap();
        assert_eq!(req.start_handle, 0x0001);
        assert_eq!(req.end_handle, 0x00FF);
    }

    #[test]
    fn test_find_info_rsp_16bit() {
        let pairs = vec![
            AttHandleUuidPair {
                handle: 0x0001,
                uuid: BtUuid::from_uuid16(0x2800),
            },
            AttHandleUuidPair {
                handle: 0x0002,
                uuid: BtUuid::from_uuid16(0x2801),
            },
        ];
        let raw = AttFindInfoRsp::build(pairs.clone());
        assert_eq!(raw[0], 0x05);
        assert_eq!(raw[1], 0x01); // UUID16 format

        let rsp = AttFindInfoRsp::parse(&raw).unwrap();
        assert_eq!(rsp.pairs.len(), 2);
        assert_eq!(rsp.pairs[0].handle, 0x0001);
        assert_eq!(rsp.pairs[0].uuid.as_uuid16(), Some(0x2800));
    }

    // ── Read By Group Type (Primary Service Discovery) ──

    #[test]
    fn test_read_by_group_type_req_roundtrip() {
        let primary_service = BtUuid::from_uuid16(0x2800);
        let raw = AttReadByGroupTypeReq::build(0x0001, 0xFFFF, primary_service);
        assert_eq!(raw[0], 0x10);
        let req = AttReadByGroupTypeReq::parse(&raw).unwrap();
        assert_eq!(req.start_handle, 0x0001);
        assert_eq!(req.end_handle, 0xFFFF);
        assert_eq!(req.group_type.as_uuid16(), Some(0x2800));
    }

    #[test]
    fn test_read_by_group_type_rsp() {
        let entries = vec![
            AttGroupAttrEntry {
                start_handle: 0x0001,
                group_end_handle: 0x0005,
                value: vec![0x00, 0x18], // UUID 0x1800 (GAP)
            },
            AttGroupAttrEntry {
                start_handle: 0x0006,
                group_end_handle: 0x000A,
                value: vec![0x0F, 0x18], // UUID 0x180F (Battery)
            },
        ];
        let raw = AttReadByGroupTypeRsp::build(entries.clone());
        assert_eq!(raw[0], 0x11);
        assert_eq!(raw[1], 6); // entry_size = 4 + value.len(2) = 6

        let rsp = AttReadByGroupTypeRsp::parse(&raw).unwrap();
        assert_eq!(rsp.entries.len(), 2);
        assert_eq!(rsp.entries[0].start_handle, 0x0001);
        assert_eq!(rsp.entries[0].group_end_handle, 0x0005);
    }

    // ── Read By Type ──

    #[test]
    fn test_read_by_type_req_roundtrip() {
        let char_decl = BtUuid::from_uuid16(0x2803);
        let raw = AttReadByTypeReq::build(0x0001, 0xFFFF, char_decl);
        assert_eq!(raw[0], 0x08);
        let req = AttReadByTypeReq::parse(&raw).unwrap();
        assert_eq!(req.attr_type.as_uuid16(), Some(0x2803));
    }

    #[test]
    fn test_read_by_type_rsp() {
        let pairs = vec![
            (0x0002u16, vec![0x02, 0x00, 0x03, 0x00, 0x00, 0x18]),
            (0x0007u16, vec![0x02, 0x00, 0x08, 0x00, 0x01, 0x18]),
        ];
        let raw = AttReadByTypeRsp::build(pairs.clone());
        assert_eq!(raw[0], 0x09);

        let rsp = AttReadByTypeRsp::parse(&raw).unwrap();
        assert_eq!(rsp.handle_value_pairs.len(), 2);
        assert_eq!(rsp.handle_value_pairs[0].0, 0x0002);
    }

    // ── Read Request / Response ──

    #[test]
    fn test_read_req_rsp_roundtrip() {
        let raw_req = AttReadReq::build(0x0042);
        assert_eq!(raw_req[0], 0x0A);
        let req = AttReadReq::parse(&raw_req).unwrap();
        assert_eq!(req.handle, 0x0042);

        let raw_rsp = AttReadRsp::build(vec![0x01, 0x02, 0x03]);
        assert_eq!(raw_rsp[0], 0x0B);
        let rsp = AttReadRsp::parse(&raw_rsp).unwrap();
        assert_eq!(rsp.value, vec![0x01, 0x02, 0x03]);
    }

    // ── Read Blob ──

    #[test]
    fn test_read_blob_roundtrip() {
        let raw_req = AttReadBlobReq::build(0x0042, 64);
        assert_eq!(raw_req[0], 0x0C);
        let req = AttReadBlobReq::parse(&raw_req).unwrap();
        assert_eq!(req.handle, 0x0042);
        assert_eq!(req.offset, 64);

        let raw_rsp = AttReadBlobRsp::build(vec![0xAA; 32]);
        assert_eq!(raw_rsp[0], 0x0D);
        let rsp = AttReadBlobRsp::parse(&raw_rsp).unwrap();
        assert_eq!(rsp.value.len(), 32);
    }

    // ── Write Request / Response ──

    #[test]
    fn test_write_req_roundtrip() {
        let raw = AttWriteReq::build(0x0042, vec![0x01]);
        assert_eq!(raw[0], 0x12);
        let req = AttWriteReq::parse(&raw).unwrap();
        assert_eq!(req.handle, 0x0042);
        assert_eq!(req.value, vec![0x01]);
    }

    #[test]
    fn test_write_rsp() {
        let raw = build_write_rsp();
        assert_eq!(raw, vec![0x13]);
    }

    // ── Write Command ──

    #[test]
    fn test_write_cmd_roundtrip() {
        let raw = AttWriteCmd::build(0x0042, vec![0xAA]);
        assert_eq!(raw[0], 0x52);
        let cmd = AttWriteCmd::parse(&raw).unwrap();
        assert_eq!(cmd.handle, 0x0042);
        assert_eq!(cmd.value, vec![0xAA]);
    }

    // ── Handle Value Notification / Indication ──

    #[test]
    fn test_notification_roundtrip() {
        let raw = AttHandleValueNtf::build(0x0042, vec![0x01, 0x02, 0x03]);
        assert_eq!(raw[0], 0x1B);
        let ntf = AttHandleValueNtf::parse(&raw).unwrap();
        assert_eq!(ntf.handle, 0x0042);
        assert_eq!(ntf.value, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_indication_roundtrip() {
        let raw = AttHandleValueInd::build(0x0042, vec![0x01, 0x02]);
        assert_eq!(raw[0], 0x1D);
        let ind = AttHandleValueInd::parse(&raw).unwrap();
        assert_eq!(ind.handle, 0x0042);

        let cfm = build_handle_value_cfm();
        assert_eq!(cfm, vec![0x1E]);
    }

    // ── Find By Type Value ──

    #[test]
    fn test_find_by_type_value_roundtrip() {
        let raw = AttFindByTypeValueReq::build(
            0x0001,
            0xFFFF,
            BtUuid::from_uuid16(0x2800),
            vec![0x00, 0x18], // GAP service UUID
        );
        assert_eq!(raw[0], 0x06);
        let req = AttFindByTypeValueReq::parse(&raw).unwrap();
        assert_eq!(req.start_handle, 0x0001);
        assert_eq!(req.attr_type.as_uuid16(), Some(0x2800));
        assert_eq!(req.value, vec![0x00, 0x18]);

        let ranges = vec![
            AttHandleRange {
                start_handle: 0x0001,
                end_handle: 0x0005,
            },
        ];
        let raw_rsp = AttFindByTypeValueRsp::build(ranges.clone());
        assert_eq!(raw_rsp[0], 0x07);
        let rsp = AttFindByTypeValueRsp::parse(&raw_rsp).unwrap();
        assert_eq!(rsp.ranges.len(), 1);
        assert_eq!(rsp.ranges[0].start_handle, 0x0001);
    }

    // ── Prepare Write / Execute Write ──

    #[test]
    fn test_prepare_write_roundtrip() {
        let raw = AttPrepareWriteReq::build(0x0042, 0, vec![0x01, 0x02, 0x03]);
        assert_eq!(raw[0], 0x16);
        let req = AttPrepareWriteReq::parse(&raw).unwrap();
        assert_eq!(req.handle, 0x0042);
        assert_eq!(req.offset, 0);

        let raw_rsp = AttPrepareWriteRsp::build(0x0042, 0, vec![0x01, 0x02, 0x03]);
        assert_eq!(raw_rsp[0], 0x17);
    }

    #[test]
    fn test_execute_write_roundtrip() {
        let raw = AttExecuteWriteReq::build(AttExecWriteFlag::Write);
        assert_eq!(raw[0], 0x18);
        assert_eq!(raw[1], 0x01);
        let req = AttExecuteWriteReq::parse(&raw).unwrap();
        assert_eq!(req.flag, AttExecWriteFlag::Write);

        let raw_cancel = AttExecuteWriteReq::build(AttExecWriteFlag::Cancel);
        assert_eq!(raw_cancel[1], 0x00);

        let raw_rsp = build_execute_write_rsp();
        assert_eq!(raw_rsp, vec![0x19]);
    }

    // ── ATT PDU dispatch ──

    #[test]
    fn test_parse_att_pdu_write_req() {
        let raw = AttWriteReq::build(0x0001, vec![0x00]);
        let decoded = parse_att_pdu(&raw).unwrap();
        match decoded {
            AttPdu::WriteReq(req) => {
                assert_eq!(req.handle, 0x0001);
            }
            _ => panic!("Wrong PDU type"),
        }
    }

    #[test]
    fn test_parse_att_pdu_notification() {
        let raw = AttHandleValueNtf::build(0x000A, vec![0x01]);
        let decoded = parse_att_pdu(&raw).unwrap();
        match decoded {
            AttPdu::HandleValueNtf(ntf) => {
                assert_eq!(ntf.handle, 0x000A);
            }
            _ => panic!("Wrong PDU type"),
        }
    }

    #[test]
    fn test_parse_att_pdu_invalid() {
        assert!(parse_att_pdu(&[]).is_none());
        assert!(parse_att_pdu(&[0xFF]).is_none());
    }

    // ── MTU clamping ──

    #[test]
    fn test_mtu_clamping() {
        let raw = AttExchangeMtuReq::build(1000); // > MAX_MTU (512)
        let req = AttExchangeMtuReq::parse(&raw).unwrap();
        assert_eq!(req.client_rx_mtu, 512);

        let raw = AttExchangeMtuRsp::build(10); // < MIN_MTU (23)
        let rsp = AttExchangeMtuRsp::parse(&raw).unwrap();
        assert_eq!(rsp.server_rx_mtu, 23);
    }

    // ── Error cases ──

    #[test]
    fn test_parse_short_data_returns_none() {
        assert!(AttErrorRsp::parse(&[0x01, 0x01]).is_none());
        assert!(AttReadReq::parse(&[0x0A]).is_none());
        assert!(AttWriteReq::parse(&[0x12, 0x01]).is_none());
        assert!(AttFindByTypeValueReq::parse(&[0x06, 0x01, 0x00, 0xFF]).is_none());
    }
}
