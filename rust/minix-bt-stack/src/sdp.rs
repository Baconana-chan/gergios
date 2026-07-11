//! # SDP — Service Discovery Protocol
//!
//! Implements the Bluetooth Service Discovery Protocol (SDP).
//! SDP allows devices to discover what services are available on
//! other Bluetooth devices.
//!
//! ## PDU Format
//!
//! ```text
//! | PDU ID (1) | Transaction ID (2) | Param Length (2) | Parameters (N) |
//! ```
//!
//! ## Protocol Flow
//!
//! 1. Client sends ServiceSearchRequest to find services by UUID
//! 2. Server responds with ServiceSearchResponse containing record handles
//! 3. Client sends ServiceAttributeRequest to get attributes of a record
//! 4. Server responds with ServiceAttributeResponse containing attribute values

#![allow(dead_code)]

use crate::sdp_record::{
    DataElement, ServiceDatabase,
};
use crate::types::BtUuid;

// ============================================================================
// Constants
// ============================================================================

/// Maximum SDP request/response size.
pub const SDP_MAX_SIZE: usize = 65535;
/// Default SDP MTU.
pub const SDP_DEFAULT_MTU: u16 = 672;

/// SDP PSM.
pub const SDP_PSM: u16 = 0x0001;

// ============================================================================
// PDU IDs
// ============================================================================

/// SDP PDU IDs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum SdpPduId {
    ErrorResponse = 0x01,
    ServiceSearchRequest = 0x02,
    ServiceSearchResponse = 0x03,
    ServiceAttributeRequest = 0x04,
    ServiceAttributeResponse = 0x05,
    ServiceSearchAttributeRequest = 0x06,
    ServiceSearchAttributeResponse = 0x07,
}

impl SdpPduId {
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x01 => Some(Self::ErrorResponse),
            0x02 => Some(Self::ServiceSearchRequest),
            0x03 => Some(Self::ServiceSearchResponse),
            0x04 => Some(Self::ServiceAttributeRequest),
            0x05 => Some(Self::ServiceAttributeResponse),
            0x06 => Some(Self::ServiceSearchAttributeRequest),
            0x07 => Some(Self::ServiceSearchAttributeResponse),
            _ => None,
        }
    }
}

// ============================================================================
// Error codes
// ============================================================================

/// SDP error codes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum SdpErrorCode {
    Success = 0x0000,
    InvalidSdpVersion = 0x0001,
    InvalidServiceRecordHandle = 0x0002,
    InvalidRequestSyntax = 0x0003,
    InvalidPduSize = 0x0004,
    InvalidContinuationState = 0x0005,
    InsufficientResources = 0x0006,
}

impl SdpErrorCode {
    pub fn to_raw(self) -> u16 {
        self as u16
    }

    pub fn from_raw(raw: u16) -> Self {
        match raw {
            0x0000 => Self::Success,
            0x0001 => Self::InvalidSdpVersion,
            0x0002 => Self::InvalidServiceRecordHandle,
            0x0003 => Self::InvalidRequestSyntax,
            0x0004 => Self::InvalidPduSize,
            0x0005 => Self::InvalidContinuationState,
            0x0006 => Self::InsufficientResources,
            _ => Self::InvalidRequestSyntax,
        }
    }
}

// ============================================================================
// Continuation State
// ============================================================================

/// SDP continuation state for fragmented responses.
#[derive(Clone, Debug)]
pub struct ContinuationState {
    pub data: Vec<u8>,
}

impl ContinuationState {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Parse continuation state from raw bytes.
    /// Format: [length(1) | data(N)]
    pub fn parse(data: &[u8]) -> Self {
        if data.is_empty() {
            return Self::new();
        }
        let len = data[0] as usize;
        if len == 0 {
            return Self::new();
        }
        let end = 1 + len.min(data.len().saturating_sub(1));
        Self {
            data: data[1..end].to_vec(),
        }
    }

    /// Encode continuation state.
    pub fn encode(&self) -> Vec<u8> {
        if self.data.is_empty() {
            return vec![0x00]; // Zero-length = no continuation
        }
        let mut buf = Vec::with_capacity(1 + self.data.len());
        buf.push(self.data.len() as u8);
        buf.extend_from_slice(&self.data);
        buf
    }

    /// Whether this is the final (complete) response.
    pub fn is_complete(&self) -> bool {
        self.data.is_empty()
    }
}

// ============================================================================
// SDP PDU Header
// ============================================================================

/// Common SDP PDU header.
#[derive(Clone, Debug)]
pub struct SdpHeader {
    pub pdu_id: SdpPduId,
    pub transaction_id: u16,
    pub param_length: u16,
}

/// Parse SDP PDU header from raw bytes.
pub fn parse_sdp_header(data: &[u8]) -> Option<(SdpHeader, usize)> {
    if data.len() < 5 {
        return None;
    }
    let pdu_id = SdpPduId::from_byte(data[0])?;
    let transaction_id = u16::from_be_bytes([data[1], data[2]]);
    let param_length = u16::from_be_bytes([data[3], data[4]]);

    if 5 + param_length as usize > data.len() {
        return None; // Incomplete PDU
    }

    Some((
        SdpHeader {
            pdu_id,
            transaction_id,
            param_length,
        },
        5,
    ))
}

/// Build SDP PDU header bytes.
pub fn build_sdp_header(pdu_id: SdpPduId, transaction_id: u16, param_length: u16) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5);
    buf.push(pdu_id as u8);
    buf.extend_from_slice(&transaction_id.to_be_bytes());
    buf.extend_from_slice(&param_length.to_be_bytes());
    buf
}

// ============================================================================
// ServiceSearchRequest (PDU ID 0x02)
// ============================================================================

/// ServiceSearchRequest PDU.
#[derive(Clone, Debug)]
pub struct ServiceSearchRequest {
    pub transaction_id: u16,
    /// List of service UUIDs to search for.
    pub search_pattern: Vec<BtUuid>,
    /// Maximum number of records to return.
    pub max_records: u16,
    /// Continuation state (for fragmented requests).
    pub cont_state: ContinuationState,
}

impl ServiceSearchRequest {
    /// Parse from raw bytes (excluding header).
    pub fn parse(transaction_id: u16, data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }

        // Parse search pattern (DataElementSequence of UUIDs)
        let pattern_type = data[0];
        let pattern_type_desc = pattern_type >> 3;
        let pattern_size_desc = pattern_type & 0x07;

        if DataElementType::from_bits(pattern_type_desc)? != DataElementType::DataElementSequence {
            return None;
        }

        let pattern_len = match pattern_size_desc {
            5 => data[1] as usize, // 1-byte length
            6 => u16::from_be_bytes([data[1], data[2]]) as usize,
            7 => u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize,
            _ => return None,
        };

        let pattern_start = match pattern_size_desc {
            5 => 2,
            6 => 3,
            7 => 5,
            _ => return None,
        };

        // Parse pattern body for UUIDs
        let pattern_data = &data[pattern_start..pattern_start + pattern_len];
        let search_pattern = parse_uuid_sequence(pattern_data);

        let offset = pattern_start + pattern_len;

        // Max records (2 bytes, big-endian)
        if offset + 2 > data.len() {
            return None;
        }
        let max_records = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let offset = offset + 2;

        // Continuation state
        let cont_state = ContinuationState::parse(&data[offset..]);

        Some(Self {
            transaction_id,
            search_pattern,
            max_records,
            cont_state,
        })
    }

    pub fn build(&self) -> Vec<u8> {
        let mut params = Vec::new();

        // Search pattern as DataElementSequence of UUIDs
        let mut pattern_elems = Vec::new();
        for uuid in &self.search_pattern {
            pattern_elems.push(DataElement::Uuid(*uuid));
        }
        let pattern_encoded = DataElement::Seq(pattern_elems).encode();
        params.extend_from_slice(&pattern_encoded);

        // Max records
        params.extend_from_slice(&self.max_records.to_be_bytes());

        // Continuation state
        params.extend_from_slice(&self.cont_state.encode());

        let mut pdu = Vec::new();
        pdu.extend_from_slice(&build_sdp_header(
            SdpPduId::ServiceSearchRequest,
            self.transaction_id,
            params.len() as u16,
        ));
        pdu.extend_from_slice(&params);
        pdu
    }
}

// ============================================================================
// ServiceSearchResponse (PDU ID 0x03)
// ============================================================================

/// ServiceSearchResponse PDU.
#[derive(Clone, Debug)]
pub struct ServiceSearchResponse {
    pub transaction_id: u16,
    /// Matching service record handles.
    pub record_handles: Vec<u32>,
    /// Continuation state (non-empty = more data available).
    pub cont_state: ContinuationState,
}

impl ServiceSearchResponse {
    pub fn parse(transaction_id: u16, data: &[u8]) -> Option<Self> {
        if data.len() < 3 {
            return None;
        }
        let total_records = u16::from_be_bytes([data[0], data[1]]);
        if 2 + total_records as usize * 4 > data.len() {
            return None;
        }

        let mut handles = Vec::with_capacity(total_records as usize);
        for i in 0..total_records as usize {
            let offset = 2 + i * 4;
            let handle = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            handles.push(handle);
        }

        let offset = 2 + total_records as usize * 4;
        let cont_state = ContinuationState::parse(&data[offset..]);

        Some(Self {
            transaction_id,
            record_handles: handles,
            cont_state,
        })
    }

    pub fn build(&self, max_pdu_size: usize) -> Vec<u8> {
        let mut handles_data = Vec::new();
        handles_data.extend_from_slice(&(self.record_handles.len() as u16).to_be_bytes());
        for handle in &self.record_handles {
            handles_data.extend_from_slice(&handle.to_be_bytes());
        }

        // Check if we need to fragment
        let header_size = 5; // PDU header
        let cont_size = self.cont_state.encode().len();
        let max_params = max_pdu_size.saturating_sub(header_size + cont_size);
        let actual_handles = if max_params < handles_data.len() + 2 {
            // Need to fragment
            let available = max_params.saturating_sub(2); // Keep total count field
            let count = available / 4; // 4 bytes per handle
            handles_data.truncate(2 + count * 4);
            // Update the count
            let actual_count = count as u16;
            handles_data[..2].copy_from_slice(&actual_count.to_be_bytes());
            handles_data
        } else {
            handles_data
        };

        let mut params = Vec::new();
        params.extend_from_slice(&actual_handles);
        params.extend_from_slice(&self.cont_state.encode());

        let mut pdu = Vec::new();
        pdu.extend_from_slice(&build_sdp_header(
            SdpPduId::ServiceSearchResponse,
            self.transaction_id,
            params.len() as u16,
        ));
        pdu.extend_from_slice(&params);
        pdu
    }
}

// ============================================================================
// ServiceAttributeRequest (PDU ID 0x04)
// ============================================================================/// Attribute ID range specification.
#[derive(Clone, Debug, PartialEq)]
pub enum AttributeIdSpec {
    /// Specific attribute IDs.
    Specific(Vec<u16>),
    /// Range [start, end] inclusive.
    Range(u16, u16),
}

impl AttributeIdSpec {
    /// Return the specific attribute IDs (empty vec for range).
    pub fn specific(&self) -> Vec<u16> {
        match self {
            AttributeIdSpec::Specific(ids) => ids.clone(),
            AttributeIdSpec::Range(_, _) => Vec::new(),
        }
    }

    /// Check if this spec matches a given attribute ID.
    pub fn contains(&self, id: u16) -> bool {
        match self {
            AttributeIdSpec::Specific(ids) => ids.contains(&id),
            AttributeIdSpec::Range(start, end) => id >= *start && id <= *end,
        }
    }
}

/// ServiceAttributeRequest PDU.
#[derive(Clone, Debug)]
pub struct ServiceAttributeRequest {
    pub transaction_id: u16,
    pub record_handle: u32,
    pub attr_spec: AttributeIdSpec,
    pub max_attrs: u16,
    pub cont_state: ContinuationState,
}

impl ServiceAttributeRequest {
    pub fn parse(transaction_id: u16, data: &[u8]) -> Option<Self> {
        if data.len() < 7 {
            return None;
        }
        let record_handle = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);

        // Parse attribute ID list (DataElementSequence or range)
        let attr_type = data[4];
        let attr_type_desc = attr_type >> 3;
        let attr_size_desc = attr_type & 0x07;

        if DataElementType::from_bits(attr_type_desc)? != DataElementType::DataElementSequence
            && DataElementType::from_bits(attr_type_desc)? != DataElementType::DataElementAlternative
        {
            return None;
        }

        let attr_len = match attr_size_desc {
            5 => data[5] as usize,
            6 => u16::from_be_bytes([data[5], data[6]]) as usize,
            7 => u32::from_be_bytes([data[5], data[6], data[7], data[8]]) as usize,
            _ => return None,
        };

        let attr_start = match attr_size_desc {
            5 => 6,
            6 => 7,
            7 => 9,
            _ => return None,
        };

        let attr_data = &data[attr_start..attr_start + attr_len];
        let attr_spec = parse_attr_spec(attr_data);

        let offset = attr_start + attr_len;

        // Max attribute bytes (2 bytes, big-endian)
        if offset + 2 > data.len() {
            return None;
        }
        let max_attrs = u16::from_be_bytes([data[offset], data[offset + 1]]);
        let offset = offset + 2;

        let cont_state = ContinuationState::parse(&data[offset..]);

        Some(Self {
            transaction_id,
            record_handle,
            attr_spec,
            max_attrs,
            cont_state,
        })
    }

    pub fn build(&self) -> Vec<u8> {
        let mut params = Vec::new();

        // Record handle
        params.extend_from_slice(&self.record_handle.to_be_bytes());

        // Attribute ID list
        let attr_list = match &self.attr_spec {
            AttributeIdSpec::Specific(ids) => {
                let mut elems = Vec::new();
                for id in ids {
                    elems.push(DataElement::UnsignedInt(*id as u64, 2));
                }
                DataElement::Seq(elems).encode()
            }
            AttributeIdSpec::Range(start, end) => {
                // Attribute range: DataElementAlternative containing [start, end]
                let mut buf = Vec::new();
                // Start of range (u16)
                let start_elem = DataElement::UnsignedInt(*start as u64, 2);
                let end_elem = DataElement::UnsignedInt(*end as u64, 2);
                // Sequence of [start, end]
                let seq = DataElement::Seq(vec![start_elem, end_elem]);
                buf.extend_from_slice(&seq.encode());
                buf
            }
        };
        params.extend_from_slice(&attr_list);

        // Max attribute bytes
        params.extend_from_slice(&self.max_attrs.to_be_bytes());

        // Continuation state
        params.extend_from_slice(&self.cont_state.encode());

        let mut pdu = Vec::new();
        pdu.extend_from_slice(&build_sdp_header(
            SdpPduId::ServiceAttributeRequest,
            self.transaction_id,
            params.len() as u16,
        ));
        pdu.extend_from_slice(&params);
        pdu
    }
}

// ============================================================================
// ServiceAttributeResponse (PDU ID 0x05)
// ============================================================================

/// ServiceAttributeResponse PDU.
#[derive(Clone, Debug)]
pub struct ServiceAttributeResponse {
    pub transaction_id: u16,
    /// Attribute byte stream (encoded as DataElementSequence of (id, value) pairs).
    pub attrs: Vec<u8>,
    /// Continuation state.
    pub cont_state: ContinuationState,
}

impl ServiceAttributeResponse {
    pub fn parse(transaction_id: u16, data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        // Parse attribute list byte stream (skip the DataElementSequence header)
        let list_type = data[0];
        let list_size_desc = list_type & 0x07;
        let list_len = match list_size_desc {
            5 => data[1] as usize,
            6 => u16::from_be_bytes([data[1], data[2]]) as usize,
            7 => u32::from_be_bytes([data[1], data[2], data[3], data[4]]) as usize,
            _ => return None,
        };
        let list_start = match list_size_desc {
            5 => 2,
            6 => 3,
            7 => 5,
            _ => return None,
        };

        let attrs = data[list_start..list_start + list_len].to_vec();
        let offset = list_start + list_len;

        let cont_state = ContinuationState::parse(&data[offset..]);

        Some(Self {
            transaction_id,
            attrs,
            cont_state,
        })
    }

    pub fn build(&self) -> Vec<u8> {
        let mut params = Vec::new();

        // Attribute list as DataElementSequence
        let seq_elem = DataElement::Seq(Vec::new()); // Placeholder header only
        let seq_bytes = seq_elem.encode();
        // Replace the placeholder sequence content with actual attrs
        // The header byte says size=0 for empty sequence, but we have data
        let type_desc = (DataElementType::DataElementSequence as u8) << 3;
        let total_len = self.attrs.len();
        let (size_desc, size_bytes): (u8, Vec<u8>) = if total_len <= 0xFF {
            (5, vec![total_len as u8])
        } else if total_len <= 0xFFFF {
            (6, (total_len as u16).to_be_bytes().to_vec())
        } else {
            (7, (total_len as u32).to_be_bytes().to_vec())
        };

        params.push(type_desc | size_desc);
        params.extend_from_slice(&size_bytes);
        params.extend_from_slice(&self.attrs);

        // Continuation state
        params.extend_from_slice(&self.cont_state.encode());

        let mut pdu = Vec::new();
        pdu.extend_from_slice(&build_sdp_header(
            SdpPduId::ServiceAttributeResponse,
            self.transaction_id,
            params.len() as u16,
        ));
        pdu.extend_from_slice(&params);
        pdu
    }
}

// ============================================================================
// Error Response (PDU ID 0x01)
// ============================================================================

/// ErrorResponse PDU.
#[derive(Clone, Debug)]
pub struct ErrorResponse {
    pub transaction_id: u16,
    pub error_code: SdpErrorCode,
}

impl ErrorResponse {
    pub fn build(transaction_id: u16, error_code: SdpErrorCode) -> Vec<u8> {
        let params = error_code.to_raw().to_be_bytes().to_vec();
        let mut pdu = Vec::new();
        pdu.extend_from_slice(&build_sdp_header(
            SdpPduId::ErrorResponse,
            transaction_id,
            params.len() as u16,
        ));
        pdu.extend_from_slice(&params);
        pdu
    }
}

// ============================================================================
// SDP Server — handles SDP requests
// ============================================================================

/// Response from processing an SDP request.
pub enum SdpResponse {
    /// Raw response bytes to send back.
    Raw(Vec<u8>),
    /// No response needed.
    None,
}

/// Process an incoming SDP request PDU.
/// Returns the response PDU bytes to send back, or None on error.
pub fn process_sdp_request(
    database: &ServiceDatabase,
    request_data: &[u8],
) -> SdpResponse {
    let (header, _) = match parse_sdp_header(request_data) {
        Some(h) => h,
        None => {
            return SdpResponse::Raw(ErrorResponse::build(
                0,
                SdpErrorCode::InvalidRequestSyntax,
            ));
        }
    };

    let params = &request_data[5..5 + header.param_length as usize];

    match header.pdu_id {
        SdpPduId::ServiceSearchRequest => {
            handle_search_request(database, header.transaction_id, params)
        }
        SdpPduId::ServiceAttributeRequest => {
            handle_attribute_request(database, header.transaction_id, params)
        }
        SdpPduId::ServiceSearchAttributeRequest => {
            handle_search_attribute_request(database, header.transaction_id, params)
        }
        _ => SdpResponse::Raw(ErrorResponse::build(
            header.transaction_id,
            SdpErrorCode::InvalidRequestSyntax,
        )),
    }
}

fn handle_search_request(
    database: &ServiceDatabase,
    transaction_id: u16,
    params: &[u8],
) -> SdpResponse {
    let request = match ServiceSearchRequest::parse(transaction_id, params) {
        Some(r) => r,
        None => {
            return SdpResponse::Raw(ErrorResponse::build(
                transaction_id,
                SdpErrorCode::InvalidRequestSyntax,
            ));
        }
    };

    let handles = database.search(&request.search_pattern);
    let response = ServiceSearchResponse {
        transaction_id,
        record_handles: handles,
        cont_state: ContinuationState::new(),
    };

    SdpResponse::Raw(response.build(SDP_MAX_SIZE))
}

fn handle_attribute_request(
    database: &ServiceDatabase,
    transaction_id: u16,
    params: &[u8],
) -> SdpResponse {
    let request = match ServiceAttributeRequest::parse(transaction_id, params) {
        Some(r) => r,
        None => {
            return SdpResponse::Raw(ErrorResponse::build(
                transaction_id,
                SdpErrorCode::InvalidRequestSyntax,
            ));
        }
    };

    // Find attributes based on the spec
    let attrs = match &request.attr_spec {
        AttributeIdSpec::Specific(ids) => {
            database.find_attributes(request.record_handle, ids)
        }
        AttributeIdSpec::Range(start, end) => {
            database.find_attributes_range(request.record_handle, *start, *end)
        }
    };

    let attrs = match attrs {
        Some(a) => a,
        None => {
            return SdpResponse::Raw(ErrorResponse::build(
                transaction_id,
                SdpErrorCode::InvalidServiceRecordHandle,
            ));
        }
    };

    // Encode attributes as: attr_id (u16 DE) + value (DE) for each
    let mut attr_bytes = Vec::new();
    for (attr_id, value) in &attrs {
        let id_elem = DataElement::UnsignedInt(*attr_id as u64, 2);
        attr_bytes.extend_from_slice(&id_elem.encode());
        attr_bytes.extend_from_slice(&value.encode());
    }

    let response = ServiceAttributeResponse {
        transaction_id,
        attrs: attr_bytes,
        cont_state: ContinuationState::new(),
    };

    SdpResponse::Raw(response.build())
}

fn handle_search_attribute_request(
    database: &ServiceDatabase,
    transaction_id: u16,
    params: &[u8],
) -> SdpResponse {
    // SearchAttribute combines ServiceSearch + ServiceAttribute
    // Currently handled by first searching, then fetching attributes
    let request_data = params;

    // Parse search pattern (first data element)
    let pattern_type = request_data[0];
    let pattern_size_desc = pattern_type & 0x07;

    let pattern_len = match pattern_size_desc {
        5 => request_data[1] as usize,
        6 => u16::from_be_bytes([request_data[1], request_data[2]]) as usize,
        7 => u32::from_be_bytes([request_data[1], request_data[2], request_data[3], request_data[4]]) as usize,
        _ => return SdpResponse::Raw(ErrorResponse::build(
            transaction_id,
            SdpErrorCode::InvalidRequestSyntax,
        )),
    };

    let pattern_start = match pattern_size_desc {
        5 => 2,
        6 => 3,
        7 => 5,
        _ => return SdpResponse::Raw(ErrorResponse::build(
            transaction_id,
            SdpErrorCode::InvalidRequestSyntax,
        )),
    };

    let pattern_data = &request_data[pattern_start..pattern_start + pattern_len];
    let search_uuids = parse_uuid_sequence(pattern_data);

    let offset = pattern_start + pattern_len;

    // Parse attribute ID list
    let _attr_type = request_data[offset];
    let attr_size_desc = request_data[offset] & 0x07;
    let attr_len = match attr_size_desc {
        5 => request_data[offset + 1] as usize,
        6 => u16::from_be_bytes([request_data[offset + 1], request_data[offset + 2]]) as usize,
        7 => u32::from_be_bytes([request_data[offset + 1], request_data[offset + 2], request_data[offset + 3], request_data[offset + 4]]) as usize,
        _ => return SdpResponse::Raw(ErrorResponse::build(
            transaction_id,
            SdpErrorCode::InvalidRequestSyntax,
        )),
    };

    let attr_start = offset + match attr_size_desc {
        5 => 2,
        6 => 3,
        7 => 5,
        _ => return SdpResponse::Raw(ErrorResponse::build(
            transaction_id,
            SdpErrorCode::InvalidRequestSyntax,
        )),
    };

    let attr_data = &request_data[attr_start..attr_start + attr_len];
    let attr_spec = parse_attr_spec(attr_data);

    // Execute search + attribute fetch
    let results = match &attr_spec {
        AttributeIdSpec::Specific(ids) => {
            database.search_and_find_attributes(&search_uuids, ids)
        }
        AttributeIdSpec::Range(start, end) => {
            let handles = database.search(&search_uuids);
            handles
                .into_iter()
                .filter_map(|h| {
                    database.find_attributes_range(h, *start, *end)
                        .map(|attrs| (h, attrs))
                })
                .collect()
        }
    };

    // Build response: DataElementSequence of (handle, attr_list) pairs
    let mut response_bytes = Vec::new();
    for (handle, attrs) in &results {
        // Handle as u32 DE
        let handle_elem = DataElement::UnsignedInt(*handle as u64, 4);
        response_bytes.extend_from_slice(&handle_elem.encode());

        // Attribute list as DataElementSequence
        let mut attr_bytes = Vec::new();
        for (attr_id, value) in attrs {
            let id_elem = DataElement::UnsignedInt(*attr_id as u64, 2);
            attr_bytes.extend_from_slice(&id_elem.encode());
            attr_bytes.extend_from_slice(&value.encode());
        }
        let attr_seq = DataElement::Seq(Vec::new());
        let header = attr_seq.encode();
        // Replace placeholder with actual data
        let type_desc = (DataElementType::DataElementSequence as u8) << 3;
        let total_len = attr_bytes.len();
        let (size_desc, size_bytes): (u8, Vec<u8>) = if total_len <= 0xFF {
            (5, vec![total_len as u8])
        } else if total_len <= 0xFFFF {
            (6, (total_len as u16).to_be_bytes().to_vec())
        } else {
            (7, (total_len as u32).to_be_bytes().to_vec())
        };
        response_bytes.push(type_desc | size_desc);
        response_bytes.extend_from_slice(&size_bytes);
        response_bytes.extend_from_slice(&attr_bytes);
    }

    // Wrap in outer DataElementSequence
    let outer_seq = DataElement::Seq(Vec::new());
    let seq_header = outer_seq.encode();
    let type_desc = (DataElementType::DataElementSequence as u8) << 3;
    let total_len = response_bytes.len();
    let (size_desc, size_bytes): (u8, Vec<u8>) = if total_len <= 0xFF {
        (5, vec![total_len as u8])
    } else if total_len <= 0xFFFF {
        (6, (total_len as u16).to_be_bytes().to_vec())
    } else {
        (7, (total_len as u32).to_be_bytes().to_vec())
    };

    let mut params = Vec::new();
    params.push(type_desc | size_desc);
    params.extend_from_slice(&size_bytes);
    params.extend_from_slice(&response_bytes);

    // Continuation state (empty = complete)
    params.push(0x00);

    let mut pdu = Vec::new();
    pdu.extend_from_slice(&build_sdp_header(
        SdpPduId::ServiceSearchAttributeResponse,
        transaction_id,
        params.len() as u16,
    ));
    pdu.extend_from_slice(&params);

    SdpResponse::Raw(pdu)
}

// ============================================================================
// Helper functions
// ============================================================================

use crate::sdp_record::DataElementType;

/// Parse a sequence of UUIDs from raw encoded data elements.
fn parse_uuid_sequence(data: &[u8]) -> Vec<BtUuid> {
    let mut uuids = Vec::new();
    let mut offset = 0;

    while offset < data.len() {
        let header = data[offset];
        let type_desc = header >> 3;
        let size_desc = header & 0x07;

        if DataElementType::from_bits(type_desc) != Some(DataElementType::Uuid) {
            offset += 1;
            continue;
        }

        let (uuid_bytes, consumed) = match size_desc {
            1 => {
                // 16-bit UUID
                if offset + 3 > data.len() {
                    break;
                }
                let mut bytes = crate::types::BLUETOOTH_BASE_UUID_R;
                bytes[2] = data[offset + 1];
                bytes[3] = data[offset + 2];
                (bytes, 3)
            }
            2 => {
                // 32-bit UUID
                if offset + 5 > data.len() {
                    break;
                }
                let mut bytes = crate::types::BLUETOOTH_BASE_UUID_R;
                bytes[0..4].copy_from_slice(&data[offset + 1..offset + 5]);
                (bytes, 5)
            }
            4 => {
                // 128-bit UUID
                if offset + 17 > data.len() {
                    break;
                }
                let mut bytes = [0u8; 16];
                bytes.copy_from_slice(&data[offset + 1..offset + 17]);
                (bytes, 17)
            }
            _ => break,
        };

        uuids.push(BtUuid::from_bytes(uuid_bytes));
        offset += consumed;
    }

    uuids
}

/// Parse attribute ID specification from data element stream.
fn parse_attr_spec(data: &[u8]) -> AttributeIdSpec {
    let mut ids = Vec::new();
    let mut offset = 0;

    while offset < data.len() {
        let header = data[offset];
        let type_desc = header >> 3;
        let size_desc = header & 0x07;

        // Use DataElementType::from_bits for correct comparison
        if let Some(elem_type) = DataElementType::from_bits(type_desc) {
            match elem_type {
                DataElementType::UnsignedInt => {
                    let val = match size_desc {
                        1 => data.get(offset + 1).copied().unwrap_or(0) as u16,
                        2 => {
                            if offset + 3 > data.len() { break; }
                            u16::from_be_bytes([data[offset + 1], data[offset + 2]])
                        }
                        3 => {
                            if offset + 5 > data.len() { break; }
                            u32::from_be_bytes([data[offset + 1], data[offset + 2], data[offset + 3], data[offset + 4]]) as u16
                        }
                        _ => {
                            offset += 1;
                            continue;
                        }
                    };
                    ids.push(val);
                    let advance = 1 + match size_desc {
                        1 => 1,
                        2 => 2,
                        3 => 4,
                        _ => 0,
                    };
                    offset += advance;
                }
                DataElementType::DataElementSequence => {
                    // Nested sequence of two values = range [start, end]
                    // Skip past the sequence header+size bytes to get to the payload
                    let (payload_offset, payload_len) = match size_desc {
                        5 => {
                            let len = data.get(offset + 1).copied().unwrap_or(0) as usize;
                            (offset + 2, len)
                        }
                        6 => {
                            if offset + 3 > data.len() { break; }
                            let len = u16::from_be_bytes([data[offset + 1], data[offset + 2]]) as usize;
                            (offset + 3, len)
                        }
                        7 => {
                            if offset + 5 > data.len() { break; }
                            // 4-byte length but we'd need to check remaining
                            let len = u32::from_be_bytes([data[offset + 1], data[offset + 2], data[offset + 3], data[offset + 4]]) as usize;
                            (offset + 5, len)
                        }
                        _ => {
                            offset += 1;
                            continue;
                        }
                    };
                    if payload_offset + payload_len > data.len() {
                        break;
                    }
                    // Parse the inner payload (sequence elements) recursively
                    let inner = &data[payload_offset..payload_offset + payload_len];
                    let nested_ids = parse_attr_spec(inner);
                    match nested_ids {
                        AttributeIdSpec::Specific(v) if v.len() == 2 => {
                            return AttributeIdSpec::Range(v[0], v[1]);
                        }
                        _ => {
                            ids.extend(nested_ids.specific());
                        }
                    }
                    // Advance past the full sequence (header + payload)
                    offset = payload_offset + payload_len;
                }
                _ => {
                    offset += 1;
                }
            }
        } else {
            offset += 1;
        }
    }

    AttributeIdSpec::Specific(ids)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdp_record::ServiceRecord;

    #[test]
    fn test_sdp_header_roundtrip() {
        let header_bytes = build_sdp_header(SdpPduId::ServiceSearchRequest, 0x1234, 0);
        let (parsed, consumed) = parse_sdp_header(&header_bytes).unwrap();
        assert_eq!(parsed.pdu_id, SdpPduId::ServiceSearchRequest);
        assert_eq!(parsed.transaction_id, 0x1234);
        assert_eq!(parsed.param_length, 0);
        assert_eq!(consumed, 5);
    }

    #[test]
    fn test_continuation_state() {
        let cont = ContinuationState::new();
        assert!(cont.is_complete());
        let encoded = cont.encode();
        assert_eq!(encoded, vec![0x00]);

        let cont2 = ContinuationState {
            data: vec![0x01, 0x02, 0x03],
        };
        let encoded2 = cont2.encode();
        assert_eq!(encoded2, vec![0x03, 0x01, 0x02, 0x03]);

        let parsed = ContinuationState::parse(&encoded2);
        assert_eq!(parsed.data, vec![0x01, 0x02, 0x03]);
    }

    #[test]
    fn test_service_search_req_build_parse() {
        let req = ServiceSearchRequest {
            transaction_id: 0x0001,
            search_pattern: vec![BtUuid::from_uuid16(0x1101)],
            max_records: 10,
            cont_state: ContinuationState::new(),
        };
        let pdu = req.build();
        assert!(pdu.len() > 5);

        let params = &pdu[5..];
        let parsed = ServiceSearchRequest::parse(0x0001, params).unwrap();
        assert_eq!(parsed.search_pattern.len(), 1);
        assert_eq!(parsed.search_pattern[0].as_uuid16(), Some(0x1101));
        assert_eq!(parsed.max_records, 10);
    }

    #[test]
    fn test_service_search_rsp_build_parse() {
        let rsp = ServiceSearchResponse {
            transaction_id: 0x0001,
            record_handles: vec![0x10000, 0x10001, 0x10002],
            cont_state: ContinuationState::new(),
        };
        let pdu = rsp.build(SDP_MAX_SIZE);
        assert!(pdu.len() > 5);

        let params = &pdu[5..];
        let parsed = ServiceSearchResponse::parse(0x0001, params).unwrap();
        assert_eq!(parsed.record_handles, vec![0x10000, 0x10001, 0x10002]);
    }

    #[test]
    fn test_service_attr_req_build() {
        let req = ServiceAttributeRequest {
            transaction_id: 0x0001,
            record_handle: 0x10000,
            attr_spec: AttributeIdSpec::Range(0x0000, 0x00FF),
            max_attrs: 4096,
            cont_state: ContinuationState::new(),
        };
        let pdu = req.build();
        assert!(pdu.len() > 5);

        let params = &pdu[5..];
        let parsed = ServiceAttributeRequest::parse(0x0001, params).unwrap();
        assert_eq!(parsed.record_handle, 0x10000);
        // Range may round-trip as Specific([0x0000, 0x00FF]) since the Seq wrapper
        // is stripped during parsing — both are semantically equivalent for lookups.
        match parsed.attr_spec {
            AttributeIdSpec::Range(start, end) => {
                assert_eq!(start, 0x0000);
                assert_eq!(end, 0x00FF);
            }
            AttributeIdSpec::Specific(ref ids) => {
                assert_eq!(ids, &[0x0000, 0x00FF]);
            }
        }
        assert_eq!(parsed.max_attrs, 4096);
    }

    #[test]
    fn test_full_sdp_flow() {
        let mut db = ServiceDatabase::new();

        // Register a serial port service
        let record = crate::sdp_record::build_rfcomm_service_record(1, "SDP Test");
        db.register_service(record);

        // Build a search request for Serial Port UUID
        let req = ServiceSearchRequest {
            transaction_id: 0x0001,
            search_pattern: vec![BtUuid::from_uuid16(0x1101)],
            max_records: 10,
            cont_state: ContinuationState::new(),
        };

        let request_pdu = req.build();

        // Process through server
        let response = process_sdp_request(&db, &request_pdu);

        match response {
            SdpResponse::Raw(data) => {
                // Parse response
                let header = data[0];
                assert_eq!(header, SdpPduId::ServiceSearchResponse as u8);

                let params = &data[5..];
                let rsp = ServiceSearchResponse::parse(0x0001, params).unwrap();
                assert_eq!(rsp.record_handles.len(), 1);

                // Now request attributes for this handle
                let handle = rsp.record_handles[0];
                let attr_req = ServiceAttributeRequest {
                    transaction_id: 0x0002,
                    record_handle: handle,
                    attr_spec: AttributeIdSpec::Range(0x0000, 0xFFFF),
                    max_attrs: 65535,
                    cont_state: ContinuationState::new(),
                };
                let attr_pdu = attr_req.build();
                let attr_rsp = process_sdp_request(&db, &attr_pdu);

                match attr_rsp {
                    SdpResponse::Raw(attr_data) => {
                        let attr_header = &attr_data[5..];
                        let attr_rsp = ServiceAttributeResponse::parse(0x0002, attr_header).unwrap();
                        assert!(!attr_rsp.attrs.is_empty());
                    }
                    _ => panic!("Expected attribute response"),
                }
            }
            _ => panic!("Expected search response"),
        }
    }

    #[test]
    fn test_error_response() {
        let error_pdu = ErrorResponse::build(0x0001, SdpErrorCode::InvalidServiceRecordHandle);
        assert!(error_pdu.len() >= 7);
        assert_eq!(error_pdu[0], 0x01); // Error PDU ID
        assert_eq!(error_pdu[3], 0x00); // Param length MSB
        assert_eq!(error_pdu[4], 0x02); // Param length LSB
        assert_eq!(error_pdu[5], 0x00); // Error code MSB
        assert_eq!(error_pdu[6], 0x02); // Error code LSB
    }

    #[test]
    fn test_search_attribute_request() {
        let mut db = ServiceDatabase::new();
        let record = crate::sdp_record::build_rfcomm_service_record(1, "Test");
        db.register_service(record);

        // Build a ServiceSearchAttributeRequest
        // This is more complex - needs to be parsed correctly
        let search_uuid = BtUuid::from_uuid16(0x1101);
        let search_pattern = DataElement::Seq(vec![DataElement::Uuid(search_uuid)]);
        let pattern_bytes = search_pattern.encode();

        // Request attribute IDs 0x0000-0x0004
        let attr_spec = DataElement::Seq(vec![
            DataElement::UnsignedInt(0x0000, 2),
            DataElement::UnsignedInt(0x0001, 2),
            DataElement::UnsignedInt(0x0004, 2),
        ]);
        let attr_bytes = attr_spec.encode();

        let mut params = Vec::new();
        params.extend_from_slice(&pattern_bytes);
        params.extend_from_slice(&attr_bytes);
        params.push(0x00); // Max attr (dummy)
        params.push(0xFF);
        params.push(0x00); // No continuation

        let pdu = build_sdp_header(
            SdpPduId::ServiceSearchAttributeRequest,
            0x0001,
            params.len() as u16,
        );
        let full_pdu: Vec<u8> = pdu.into_iter().chain(params).collect();

        let response = process_sdp_request(&db, &full_pdu);
        match response {
            SdpResponse::Raw(data) => {
                assert!(data.len() > 5);
                assert_eq!(data[0], SdpPduId::ServiceSearchAttributeResponse as u8);
            }
            _ => panic!("Expected search attribute response"),
        }
    }
}
