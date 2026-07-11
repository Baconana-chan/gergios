//! # SDP Data Elements and Service Records
//!
//! Implements the Bluetooth SDP (Service Discovery Protocol) data element
//! representation and service record storage.
//!
//! ## Data Element Format (type-length-value)
//!
//! Each data element has a header byte (or header + size bytes):
//!
//! ```text
//! Bit 7-3: Type descriptor (5 bits)
//! Bit 2-0: Size descriptor (3 bits)
//!
//! Size descriptor 0-4 = fixed size (0, 1, 2, 4, 8 bytes)
//! Size descriptor 5 = variable size, 1 additional byte
//! Size descriptor 6 = variable size, 2 additional bytes
//! Size descriptor 7 = variable size, 4 additional bytes
//! ```

#![allow(dead_code)]

use crate::types::BtUuid;

// ============================================================================
// Data Element Type Descriptors
// ============================================================================

/// SDP Data Element type descriptor (upper 5 bits of header byte).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum DataElementType {
    Nil = 0x00,
    UnsignedInt = 0x01,
    SignedInt = 0x02,
    Uuid = 0x03,
    String = 0x04,
    Boolean = 0x05,
    DataElementSequence = 0x06,
    DataElementAlternative = 0x07,
    Url = 0x08,
}

impl DataElementType {
    pub fn from_bits(bits: u8) -> Option<Self> {
        match bits {
            0x00 => Some(Self::Nil),
            0x01 => Some(Self::UnsignedInt),
            0x02 => Some(Self::SignedInt),
            0x03 => Some(Self::Uuid),
            0x04 => Some(Self::String),
            0x05 => Some(Self::Boolean),
            0x06 => Some(Self::DataElementSequence),
            0x07 => Some(Self::DataElementAlternative),
            0x08 => Some(Self::Url),
            _ => None,
        }
    }
}

// ============================================================================
// Data Element — parsed representation
// ============================================================================

/// A parsed SDP Data Element.
#[derive(Clone, Debug, PartialEq)]
pub enum DataElement {
    /// Null value.
    Nil,
    /// Unsigned integer (8, 16, 32, 64, or 128 bits).
    UnsignedInt(u64, u8), // (value, byte_width)
    /// Signed integer (8, 16, 32, 64, or 128 bits).
    SignedInt(i64, u8),
    /// UUID (16, 32, or 128 bit).
    Uuid(BtUuid),
    /// String.
    String(Vec<u8>),
    /// Boolean.
    Bool(bool),
    /// Sequence of data elements.
    Seq(Vec<DataElement>),
    /// Alternative of data elements.
    Alt(Vec<DataElement>),
    /// URL.
    Url(Vec<u8>),
}

impl DataElement {
    /// Encode this data element into bytes.
    pub fn encode(&self) -> Vec<u8> {
        match self {
            DataElement::Nil => vec![0x00],
            DataElement::UnsignedInt(val, width) => {
                let type_desc = DataElementType::UnsignedInt as u8;
                let size_desc = match width {
                    0 => 0,
                    1 => 1,
                    2 => 2,
                    4 => 3,
                    8 => 4,
                    16 => 5, // 128-bit — encoded as uint128
                    _ => 0,
                };
                let header = (type_desc << 3) | size_desc;
                let mut buf = vec![header];
                match width {
                    0 => {} // No value bytes for 0-width
                    1 => buf.push(*val as u8),
                    2 => buf.extend_from_slice(&(*val as u16).to_be_bytes()),
                    4 => buf.extend_from_slice(&(*val as u32).to_be_bytes()),
                    8 => buf.extend_from_slice(&val.to_be_bytes()),
                    16 => {
                        // 128-bit, but we only have u64 — pad to 16 bytes
                        buf.extend_from_slice(&[0u8; 8]);
                        buf.extend_from_slice(&val.to_be_bytes());
                    }
                    _ => {}
                }
                buf
            }
            DataElement::SignedInt(val, width) => {
                let type_desc = DataElementType::SignedInt as u8;
                let size_desc = match width {
                    1 => 1,
                    2 => 2,
                    4 => 3,
                    8 => 4,
                    _ => 0,
                };
                let header = (type_desc << 3) | size_desc;
                let mut buf = vec![header];
                match width {
                    1 => buf.push(*val as i8 as u8),
                    2 => buf.extend_from_slice(&(*val as i16).to_be_bytes()),
                    4 => buf.extend_from_slice(&(*val as i32).to_be_bytes()),
                    8 => buf.extend_from_slice(&val.to_be_bytes()),
                    _ => {}
                }
                buf
            }
            DataElement::Uuid(uuid) => {
                let type_desc = DataElementType::Uuid as u8;
                let (size_desc, bytes) = match uuid.uuid_type() {
                    crate::types::BtUuidType::Uuid16 => (1, uuid.bytes[2..4].to_vec()),
                    crate::types::BtUuidType::Uuid32 => (2, uuid.bytes[0..4].to_vec()),
                    crate::types::BtUuidType::Uuid128 => (4, uuid.bytes.to_vec()),
                };
                let header = (type_desc << 3) | size_desc;
                let mut buf = vec![header];
                buf.extend_from_slice(&bytes);
                buf
            }
            DataElement::String(s) => {
                encode_variable(DataElementType::String, s)
            }
            DataElement::Bool(b) => {
                let header = (DataElementType::Boolean as u8) << 3; // size = 0 (1 byte bool)
                let mut buf = vec![header];
                buf.push(if *b { 0x01 } else { 0x00 });
                buf
            }
            DataElement::Seq(elements) => {
                let payload = encode_sequence(elements);
                encode_variable(DataElementType::DataElementSequence, &payload)
            }
            DataElement::Alt(elements) => {
                let payload = encode_sequence(elements);
                encode_variable(DataElementType::DataElementAlternative, &payload)
            }
            DataElement::Url(url) => {
                encode_variable(DataElementType::Url, url)
            }
        }
    }

    /// Get the encoded byte length of this data element.
    pub fn encoded_len(&self) -> usize {
        self.encode().len()
    }
}

/// Encode a variable-length data element with size descriptor.
fn encode_variable(elem_type: DataElementType, value: &[u8]) -> Vec<u8> {
    let type_desc = (elem_type as u8) << 3;
    let len = value.len();
    let (size_desc, size_bytes): (u8, Vec<u8>) = if len <= 0xFF {
        (5, vec![len as u8])
    } else if len <= 0xFFFF {
        (6, (len as u16).to_be_bytes().to_vec())
    } else {
        (7, (len as u32).to_be_bytes().to_vec())
    };

    let header = type_desc | size_desc;
    let mut buf = Vec::with_capacity(1 + size_bytes.len() + value.len());
    buf.push(header);
    buf.extend_from_slice(&size_bytes);
    buf.extend_from_slice(value);
    buf
}

/// Encode a sequence of data elements.
fn encode_sequence(elements: &[DataElement]) -> Vec<u8> {
    let mut buf = Vec::new();
    for elem in elements {
        buf.extend_from_slice(&elem.encode());
    }
    buf
}

// ============================================================================
// SDP Attribute ID
// ============================================================================

/// Standard SDP attribute IDs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u16)]
pub enum SdpAttrId {
    ServiceRecordHandle = 0x0000,
    ServiceClassIdList = 0x0001,
    ServiceRecordState = 0x0002,
    ServiceId = 0x0003,
    ProtocolDescriptorList = 0x0004,
    BrowseGroupList = 0x0005,
    LanguageBaseAttributeIdList = 0x0006,
    ServiceInfoTimeToLive = 0x0007,
    ServiceAvailability = 0x0008,
    BluetoothProfileDescriptorList = 0x0009,
    DocumentationUrl = 0x000A,
    ClientExecutableUrl = 0x000B,
    IconUrl = 0x000C,
    AdditionalProtocolDescriptorLists = 0x000D,
    /// User-defined attribute ID.
    UserDefined(u16),
}

impl SdpAttrId {
    // ── Convenience constants for standard attribute IDs ──
    pub const SERVICE_RECORD_HANDLE: u16 = 0x0000;
    pub const SERVICE_CLASS_ID_LIST: u16 = 0x0001;
    pub const SERVICE_RECORD_STATE: u16 = 0x0002;
    pub const SERVICE_ID: u16 = 0x0003;
    pub const PROTOCOL_DESCRIPTOR_LIST: u16 = 0x0004;
    pub const BROWSE_GROUP_LIST: u16 = 0x0005;
    pub const LANGUAGE_BASE_ATTRIBUTE_ID_LIST: u16 = 0x0006;
    pub const SERVICE_INFO_TIME_TO_LIVE: u16 = 0x0007;
    pub const SERVICE_AVAILABILITY: u16 = 0x0008;
    pub const BLUETOOTH_PROFILE_DESCRIPTOR_LIST: u16 = 0x0009;
    pub const DOCUMENTATION_URL: u16 = 0x000A;
    pub const CLIENT_EXECUTABLE_URL: u16 = 0x000B;
    pub const ICON_URL: u16 = 0x000C;
    pub const ADDITIONAL_PROTOCOL_DESCRIPTOR_LISTS: u16 = 0x000D;

    pub fn from_raw(raw: u16) -> Self {
        match raw {
            0x0000 => Self::ServiceRecordHandle,
            0x0001 => Self::ServiceClassIdList,
            0x0002 => Self::ServiceRecordState,
            0x0003 => Self::ServiceId,
            0x0004 => Self::ProtocolDescriptorList,
            0x0005 => Self::BrowseGroupList,
            0x0006 => Self::LanguageBaseAttributeIdList,
            0x0007 => Self::ServiceInfoTimeToLive,
            0x0008 => Self::ServiceAvailability,
            0x0009 => Self::BluetoothProfileDescriptorList,
            0x000A => Self::DocumentationUrl,
            0x000B => Self::ClientExecutableUrl,
            0x000C => Self::IconUrl,
            0x000D => Self::AdditionalProtocolDescriptorLists,
            _ => Self::UserDefined(raw),
        }
    }

    pub fn to_raw(self) -> u16 {
        match self {
            Self::ServiceRecordHandle => 0x0000,
            Self::ServiceClassIdList => 0x0001,
            Self::ServiceRecordState => 0x0002,
            Self::ServiceId => 0x0003,
            Self::ProtocolDescriptorList => 0x0004,
            Self::BrowseGroupList => 0x0005,
            Self::LanguageBaseAttributeIdList => 0x0006,
            Self::ServiceInfoTimeToLive => 0x0007,
            Self::ServiceAvailability => 0x0008,
            Self::BluetoothProfileDescriptorList => 0x0009,
            Self::DocumentationUrl => 0x000A,
            Self::ClientExecutableUrl => 0x000B,
            Self::IconUrl => 0x000C,
            Self::AdditionalProtocolDescriptorLists => 0x000D,
            Self::UserDefined(raw) => raw,
        }
    }
}

// ============================================================================
// Service Record
// ============================================================================

/// A single Bluetooth service record.
#[derive(Clone, Debug)]
pub struct ServiceRecord {
    /// Assigned record handle.
    pub handle: u32,
    /// Attribute ID → value.
    pub attributes: Vec<(u16, DataElement)>,
}

impl ServiceRecord {
    pub fn new(handle: u32) -> Self {
        Self {
            handle,
            attributes: Vec::new(),
        }
    }

    /// Add an attribute.
    pub fn set_attr(&mut self, id: u16, value: DataElement) {
        // Replace existing attribute with same ID, or add new
        if let Some(pos) = self.attributes.iter().position(|(aid, _)| *aid == id) {
            self.attributes[pos] = (id, value);
        } else {
            self.attributes.push((id, value));
        }
    }

    /// Get an attribute value by ID.
    pub fn get_attr(&self, id: u16) -> Option<&DataElement> {
        self.attributes
            .iter()
            .find(|(aid, _)| *aid == id)
            .map(|(_, v)| v)
    }

    /// Encode the full service record as a DataElementSequence of attributes.
    /// Each attribute is encoded as: attr_id (u16) + value (DataElement).
    pub fn encode_service_record(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        for (attr_id, value) in &self.attributes {
            // Encode attribute ID as unsigned 16-bit integer
            let id_elem = DataElement::UnsignedInt(*attr_id as u64, 2);
            buf.extend_from_slice(&id_elem.encode());
            // Encode attribute value
            buf.extend_from_slice(&value.encode());
        }
        // Wrap in a DataElementSequence
        let sequence = DataElement::Seq(Vec::new()); // Placeholder — rebuild with actual data
        let type_desc = (DataElementType::DataElementSequence as u8) << 3;
        let total_len = buf.len();
        let (size_desc, size_bytes): (u8, Vec<u8>) = if total_len <= 0xFF {
            (5, vec![total_len as u8])
        } else if total_len <= 0xFFFF {
            (6, (total_len as u16).to_be_bytes().to_vec())
        } else {
            (7, (total_len as u32).to_be_bytes().to_vec())
        };
        let header = type_desc | size_desc;

        let mut result = Vec::with_capacity(1 + size_bytes.len() + buf.len());
        result.push(header);
        result.extend_from_slice(&size_bytes);
        result.extend_from_slice(&buf);
        result
    }

    /// Get the class UUID from the ServiceClassIDList attribute.
    pub fn service_class_uuids(&self) -> Vec<BtUuid> {
        if let Some(DataElement::Seq(uuids)) = self.get_attr(SdpAttrId::SERVICE_CLASS_ID_LIST) {
            uuids
                .iter()
                .filter_map(|e| {
                    if let DataElement::Uuid(uuid) = e {
                        Some(*uuid)
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get the Browse Group UUIDs from the BrowseGroupList attribute (0x0005).
    pub fn browse_group_uuids(&self) -> Vec<BtUuid> {
        if let Some(DataElement::Seq(uuids)) = self.get_attr(SdpAttrId::BROWSE_GROUP_LIST) {
            uuids
                .iter()
                .filter_map(|e| {
                    if let DataElement::Uuid(uuid) = e {
                        Some(*uuid)
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            Vec::new()
        }
    }
}

// ============================================================================
// SDP Service Database
// ============================================================================

/// The SDP service database — maintains all local service records.
pub struct ServiceDatabase {
    records: Vec<ServiceRecord>,
    next_handle: u32,
}

impl ServiceDatabase {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            next_handle: 0x00010000, // Start of user-accessible range
        }
    }

    /// Register a new service. Returns the assigned handle.
    pub fn register_service(&mut self, record: ServiceRecord) -> u32 {
        let handle = self.next_handle;
        self.next_handle += 1;
        let mut rec = record;
        rec.handle = handle;
        // Set the ServiceRecordHandle attribute
        rec.set_attr(
            SdpAttrId::SERVICE_RECORD_HANDLE,
            DataElement::UnsignedInt(handle as u64, 4),
        );
        self.records.push(rec);
        handle
    }

    /// Remove a service by handle.
    pub fn unregister_service(&mut self, handle: u32) -> bool {
        let len = self.records.len();
        self.records.retain(|r| r.handle != handle);
        self.records.len() < len
    }

    /// Find a record by handle.
    pub fn find_by_handle(&self, handle: u32) -> Option<&ServiceRecord> {
        self.records.iter().find(|r| r.handle == handle)
    }

    /// Public Browse Group UUID (0x1002).
    const PUBLIC_BROWSE_GROUP: u16 = 0x1002;

    /// Check whether a given UUID is a known browse group UUID.
    fn is_browse_group_uuid(uuid: &BtUuid) -> bool {
        uuid.as_uuid16() == Some(Self::PUBLIC_BROWSE_GROUP)
    }

    /// Search for services by a list of UUID patterns (ServiceSearch).
    /// Returns handles of records whose ServiceClassIDList contains any of the UUIDs.
    ///
    /// If the search UUIDs include a known Browse Group UUID (e.g. 0x1002 = Public
    /// Browse Group), also searches the BrowseGroupList attribute. This allows SDP
    /// clients to discover allbrowseable services by searching for the Public Browse
    /// Group UUID.
    pub fn search(&self, search_uuids: &[BtUuid]) -> Vec<u32> {
        let mut handles = Vec::new();

        for record in &self.records {
            let mut matched = false;

            // 1. Match against ServiceClassIDList (attribute 0x0001) — standard lookup
            let class_uuids = record.service_class_uuids();
            if search_uuids.iter().any(|suuid| {
                class_uuids.iter().any(|ruuid| ruuid.bytes == suuid.bytes)
            }) {
                matched = true;
            }

            // 2. If any search UUID is a Browse Group UUID, match against BrowseGroupList
            if !matched && search_uuids.iter().any(|suuid| Self::is_browse_group_uuid(suuid)) {
                let browse_uuids = record.browse_group_uuids();
                if search_uuids.iter().any(|suuid| {
                    browse_uuids.iter().any(|ruuid| ruuid.bytes == suuid.bytes)
                }) {
                    matched = true;
                }
            }

            if matched {
                handles.push(record.handle);
            }
        }

        handles
    }

    /// Search for services by a Browse Group UUID.
    /// Returns handles of records whose BrowseGroupList contains the given UUID.
    pub fn search_by_browse_group(&self, browse_uuid: &BtUuid) -> Vec<u32> {
        self.records
            .iter()
            .filter(|record| {
                let browse_uuids = record.browse_group_uuids();
                browse_uuids.iter().any(|ruuid| ruuid.bytes == browse_uuid.bytes)
            })
            .map(|r| r.handle)
            .collect()
    }

    /// Find attributes for a specific record handle.
    /// Returns vec of (attr_id, encoded_value) or None if handle not found.
    pub fn find_attributes(
        &self,
        handle: u32,
        attr_ids: &[u16],
    ) -> Option<Vec<(u16, DataElement)>> {
        let record = self.records.iter().find(|r| r.handle == handle)?;
        let result: Vec<(u16, DataElement)> = record
            .attributes
            .iter()
            .filter(|(aid, _)| attr_ids.is_empty() || attr_ids.contains(aid))
            .cloned()
            .collect();
        Some(result)
    }

    /// Find attributes using a range of attribute IDs [start, end].
    pub fn find_attributes_range(
        &self,
        handle: u32,
        start_id: u16,
        end_id: u16,
    ) -> Option<Vec<(u16, DataElement)>> {
        let record = self.records.iter().find(|r| r.handle == handle)?;
        let result: Vec<(u16, DataElement)> = record
            .attributes
            .iter()
            .filter(|(aid, _)| *aid >= start_id && *aid <= end_id)
            .cloned()
            .collect();
        Some(result)
    }

    /// Search + Attribute combined: search by UUID, return attributes.
    pub fn search_and_find_attributes(
        &self,
        search_uuids: &[BtUuid],
        attr_ids: &[u16],
    ) -> Vec<(u32, Vec<(u16, DataElement)>)> {
        let handles = self.search(search_uuids);
        handles
            .into_iter()
            .filter_map(|h| {
                self.find_attributes(h, attr_ids)
                    .map(|attrs| (h, attrs))
            })
            .collect()
    }

    /// Number of registered services.
    pub fn service_count(&self) -> usize {
        self.records.len()
    }

    /// Get all records (for debugging).
    pub fn all_records(&self) -> &[ServiceRecord] {
        &self.records
    }
}

// ============================================================================
// Convenience builders for common SDP records
// ============================================================================

/// Build a minimal generic SDP service record with ServiceClassIDList.
pub fn build_service_record(
    service_uuid: BtUuid,
    protocol_uuid: BtUuid,
    psm: Option<u16>,
    port: Option<u8>,
    name: &str,
    description: &str,
) -> ServiceRecord {
    let mut record = ServiceRecord::new(0);

    // ServiceClassIDList
    record.set_attr(
        SdpAttrId::SERVICE_CLASS_ID_LIST,
        DataElement::Seq(vec![DataElement::Uuid(service_uuid)]),
    );

    // ProtocolDescriptorList (L2CAP + optional PSM, then protocol-specific)
    let mut proto_list = Vec::new();

    // L2CAP protocol descriptor
    if let Some(psm_val) = psm {
        proto_list.push(DataElement::Seq(vec![
            DataElement::Uuid(crate::types::sdp_uuids::L2CAP),
            DataElement::UnsignedInt(psm_val as u64, 2),
        ]));
    } else {
        proto_list.push(DataElement::Seq(vec![
            DataElement::Uuid(crate::types::sdp_uuids::L2CAP),
        ]));
    }

    // Upper protocol (e.g., RFCOMM with port/channel)
    if let Some(p) = port {
        proto_list.push(DataElement::Seq(vec![
            DataElement::Uuid(protocol_uuid),
            DataElement::UnsignedInt(p as u64, 1),
        ]));
    } else {
        proto_list.push(DataElement::Seq(vec![
            DataElement::Uuid(protocol_uuid),
        ]));
    }

    record.set_attr(
        SdpAttrId::PROTOCOL_DESCRIPTOR_LIST,
        DataElement::Seq(proto_list),
    );

    // BrowseGroupList — place in Public Browse Group
    record.set_attr(
        SdpAttrId::BROWSE_GROUP_LIST,
        DataElement::Seq(vec![DataElement::Uuid(BtUuid::from_uuid16(0x1002))]),
    );

    // LanguageBaseAttributeIDList (English, UTF-8)
    record.set_attr(
        SdpAttrId::LANGUAGE_BASE_ATTRIBUTE_ID_LIST,
        DataElement::Seq(vec![
            DataElement::UnsignedInt(0x656E, 2), // Language = 'en' (0x656E)
            DataElement::UnsignedInt(0x006A, 2), // Encoding = UTF-8 (0x006A)
            DataElement::UnsignedInt(0x0100, 2), // Base offset = 0x0100
        ]),
    );

    // ServiceName (offset 0x0100 from base)
    record.set_attr(
        0x0100, // base + 0x0000
        DataElement::String(name.as_bytes().to_vec()),
    );

    // ServiceDescription (offset 0x0101 from base)
    record.set_attr(
        0x0101, // base + 0x0001
        DataElement::String(description.as_bytes().to_vec()),
    );

    // ProviderName (offset 0x0102 from base)
    record.set_attr(
        0x0102, // base + 0x0002
        DataElement::String(b"GergiOS\0".to_vec()),
    );

    record
}

/// Build an RFCOMM serial port SDP record.
pub fn build_rfcomm_service_record(channel: u8, name: &str) -> ServiceRecord {
    build_service_record(
        crate::types::sdp_uuids::SERIAL_PORT,
        crate::types::sdp_uuids::RFCOMM,
        Some(crate::types::L2CapPsm::Rfcomm.to_raw()),
        Some(channel),
        name,
        "RFCOMM Serial Port",
    )
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_element_nil() {
        let elem = DataElement::Nil;
        let encoded = elem.encode();
        assert_eq!(encoded, vec![0x00]);
    }

    #[test]
    fn test_data_element_uint8() {
        let elem = DataElement::UnsignedInt(0x42, 1);
        let encoded = elem.encode();
        assert_eq!(encoded, vec![0x09, 0x42]); // type=1<<3|1=9
    }

    #[test]
    fn test_data_element_uint16() {
        let elem = DataElement::UnsignedInt(0x1234, 2);
        let encoded = elem.encode();
        assert_eq!(encoded, vec![0x0A, 0x12, 0x34]); // type=1<<3|2=10
    }

    #[test]
    fn test_data_element_uint32() {
        let elem = DataElement::UnsignedInt(0x12345678, 4);
        let encoded = elem.encode();
        assert_eq!(encoded, vec![0x0B, 0x12, 0x34, 0x56, 0x78]); // type=1<<3|3=11
    }

    #[test]
    fn test_data_element_uuid16() {
        let uuid = BtUuid::from_uuid16(0x1101);
        let elem = DataElement::Uuid(uuid);
        let encoded = elem.encode();
        assert_eq!(encoded, vec![0x19, 0x11, 0x01]); // type=3<<3|1=25=0x19
    }

    #[test]
    fn test_data_element_string_short() {
        let elem = DataElement::String(b"Hello".to_vec());
        let encoded = elem.encode();
        let expected_header = (DataElementType::String as u8) << 3 | 5; // 0x20 | 5 = 0x25
        assert_eq!(encoded[0], expected_header);
        assert_eq!(encoded[1], 5); // length byte
        assert_eq!(&encoded[2..], b"Hello");
    }

    #[test]
    fn test_data_element_bool() {
        let elem = DataElement::Bool(true);
        let encoded = elem.encode();
        assert_eq!(encoded, vec![0x28, 0x01]); // type=5<<3=40=0x28
    }

    #[test]
    fn test_data_element_sequence() {
        let elem = DataElement::Seq(vec![
            DataElement::UnsignedInt(0x01, 1),
            DataElement::UnsignedInt(0x02, 1),
        ]);
        let encoded = elem.encode();
        // Type = 6<<3 = 48 = 0x30, size = 5 (1-byte len)
        assert_eq!(encoded[0], 0x35); // 0x30 | 5
        assert_eq!(encoded[1], 4); // payload len = 4 bytes (0x09, 0x01, 0x09, 0x02)
        assert_eq!(&encoded[2..], &[0x09, 0x01, 0x09, 0x02]);
    }

    #[test]
    fn test_service_record_basic() {
        let mut record = ServiceRecord::new(0x10000);
        record.set_attr(
            SdpAttrId::SERVICE_CLASS_ID_LIST,
            DataElement::Seq(vec![DataElement::Uuid(BtUuid::from_uuid16(0x1101))]),
        );

        let class_uuids = record.service_class_uuids();
        assert_eq!(class_uuids.len(), 1);
        assert_eq!(class_uuids[0].as_uuid16(), Some(0x1101));
    }

    #[test]
    fn test_service_database() {
        let mut db = ServiceDatabase::new();
        assert_eq!(db.service_count(), 0);

        let handle = db.register_service(ServiceRecord::new(0));
        assert!(handle >= 0x00010000);
        assert_eq!(db.service_count(), 1);

        assert!(db.find_by_handle(handle).is_some());
        assert!(db.unregister_service(handle));
        assert_eq!(db.service_count(), 0);
    }

    #[test]
    fn test_service_search() {
        let mut db = ServiceDatabase::new();

        let mut record = ServiceRecord::new(0);
        record.set_attr(
            SdpAttrId::SERVICE_CLASS_ID_LIST,
            DataElement::Seq(vec![DataElement::Uuid(BtUuid::from_uuid16(0x1101))]),
        );
        let h1 = db.register_service(record);

        let mut record2 = ServiceRecord::new(0);
        record2.set_attr(
            SdpAttrId::SERVICE_CLASS_ID_LIST,
            DataElement::Seq(vec![DataElement::Uuid(BtUuid::from_uuid16(0x1108))]),
        );
        let _h2 = db.register_service(record2);

        // Search for serial port
        let results = db.search(&[BtUuid::from_uuid16(0x1101)]);
        assert_eq!(results, vec![h1]);
    }

    #[test]
    fn test_build_rfcomm_service() {
        let record = build_rfcomm_service_record(1, "Test Serial");
        let class_uuids = record.service_class_uuids();
        assert_eq!(class_uuids[0].as_uuid16(), Some(0x1101)); // Serial Port

        // Check ProtocolDescriptorList exists
        assert!(record.get_attr(SdpAttrId::PROTOCOL_DESCRIPTOR_LIST).is_some());
    }

    #[test]
    fn test_find_attributes() {
        let mut db = ServiceDatabase::new();
        let mut record = ServiceRecord::new(0);
        record.set_attr(
            SdpAttrId::SERVICE_CLASS_ID_LIST,
            DataElement::Seq(vec![DataElement::Uuid(BtUuid::from_uuid16(0x1101))]),
        );
        record.set_attr(
            SdpAttrId::SERVICE_RECORD_HANDLE,
            DataElement::UnsignedInt(42, 4),
        );
        let handle = db.register_service(record);

        let attrs = db.find_attributes(handle, &[]).unwrap();
        assert!(attrs.len() >= 2); // service class + handle + language base + name etc.
    }

    #[test]
    fn test_search_and_find_attributes() {
        let mut db = ServiceDatabase::new();
        let mut record = ServiceRecord::new(0);
        record.set_attr(
            SdpAttrId::SERVICE_CLASS_ID_LIST,
            DataElement::Seq(vec![DataElement::Uuid(BtUuid::from_uuid16(0x1101))]),
        );
        record.set_attr(
            0x0100,
            DataElement::String(b"Test Service".to_vec()),
        );
        let _handle = db.register_service(record);

        let results = db.search_and_find_attributes(
            &[BtUuid::from_uuid16(0x1101)],
            &[0x0001, 0x0100],
        );
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.len(), 2);
    }

    #[test]
    fn test_find_attributes_range() {
        let mut db = ServiceDatabase::new();
        let mut record = ServiceRecord::new(0);
        record.set_attr(0x0100, DataElement::String(b"Name".to_vec()));
        record.set_attr(0x0101, DataElement::String(b"Desc".to_vec()));
        record.set_attr(0x0200, DataElement::String(b"Other".to_vec()));
        let handle = db.register_service(record);

        let attrs = db.find_attributes_range(handle, 0x0100, 0x01FF).unwrap();
        assert_eq!(attrs.len(), 2); // 0x0100 and 0x0101
    }

    // ── Browse Group Auto-Discovery ──

    #[test]
    fn test_service_has_browse_group() {
        let record = build_rfcomm_service_record(1, "Serial");
        let browse_uuids = record.browse_group_uuids();
        assert!(!browse_uuids.is_empty());
        assert_eq!(browse_uuids[0].as_uuid16(), Some(0x1002)); // Public Browse Group
    }

    #[test]
    fn test_service_without_browse_group_returns_empty() {
        let record = ServiceRecord::new(0);
        let browse_uuids = record.browse_group_uuids();
        assert!(browse_uuids.is_empty());
    }

    #[test]
    fn test_search_by_browse_group_finds_services() {
        let mut db = ServiceDatabase::new();

        // Register a service built with build_service_record — it gets BrowseGroupList
        let record = build_rfcomm_service_record(1, "Serial Port");
        let h1 = db.register_service(record);

        // Register a second service
        let record2 = build_rfcomm_service_record(2, "Second Port");
        let h2 = db.register_service(record2);

        // Register a service WITHOUT browse group (manually created)
        let mut record3 = ServiceRecord::new(0);
        record3.set_attr(
            SdpAttrId::SERVICE_CLASS_ID_LIST,
            DataElement::Seq(vec![DataElement::Uuid(BtUuid::from_uuid16(0x1108))]),
        );
        record3.set_attr(
            SdpAttrId::PROTOCOL_DESCRIPTOR_LIST,
            DataElement::Seq(vec![]),
        );
        let h3 = db.register_service(record3);

        // Search by Public Browse Group should find h1 and h2 but not h3
        let browse_uuid = BtUuid::from_uuid16(0x1002);
        let results = db.search_by_browse_group(&browse_uuid);
        assert_eq!(results.len(), 2);
        assert!(results.contains(&h1));
        assert!(results.contains(&h2));
        assert!(!results.contains(&h3));
    }

    #[test]
    fn test_search_finds_both_class_and_browse() {
        let mut db = ServiceDatabase::new();

        // Service with Serial Port class UUID (0x1101) — searchable by class
        let record = build_rfcomm_service_record(1, "Serial");
        let h1 = db.register_service(record);

        // Service with Headset class UUID (0x1108) — NOT in browse mode
        let mut record2 = ServiceRecord::new(0);
        record2.set_attr(
            SdpAttrId::SERVICE_CLASS_ID_LIST,
            DataElement::Seq(vec![DataElement::Uuid(BtUuid::from_uuid16(0x1108))]),
        );
        let _h2 = db.register_service(record2);

        // Search by Serial Port UUID should find h1
        let results = db.search(&[BtUuid::from_uuid16(0x1101)]);
        assert_eq!(results.len(), 1);
        assert!(results.contains(&h1));

        // Search by Public Browse Group should find h1 (has BrowseGroupList)
        let browse_results = db.search(&[BtUuid::from_uuid16(0x1002)]);
        assert_eq!(browse_results.len(), 1);
        assert!(browse_results.contains(&h1));
    }

    #[test]
    fn test_search_browse_group_via_service_search_request() {
        use crate::sdp::{
            process_sdp_request, SdpResponse,
            ServiceSearchRequest, ContinuationState, ServiceSearchResponse,
        };

        let mut db = ServiceDatabase::new();

        // Register a service in the Public Browse Group
        let record = build_rfcomm_service_record(1, "Serial Port");
        db.register_service(record);

        // Build a ServiceSearchRequest for Public Browse Group UUID 0x1002
        let req = ServiceSearchRequest {
            transaction_id: 0x0001,
            search_pattern: vec![BtUuid::from_uuid16(0x1002)],
            max_records: 10,
            cont_state: ContinuationState::new(),
        };
        let request_pdu = req.build();

        let response = process_sdp_request(&db, &request_pdu);

        match response {
            SdpResponse::Raw(data) => {
                assert_eq!(data[0], 0x03); // ServiceSearchResponse PDU ID
                let rsp = ServiceSearchResponse::parse(0x0001, &data[5..]).unwrap();
                // Should find the serial port service via its BrowseGroupList
                assert_eq!(rsp.record_handles.len(), 1);
            }
            _ => panic!("Expected search response"),
        }
    }

    #[test]
    fn test_build_service_record_includes_browse_group() {
        let record = build_service_record(
            BtUuid::from_uuid16(0x1101),
            BtUuid::from_uuid16(0x0100),
            Some(0x0003),
            Some(1),
            "Test",
            "Desc",
        );

        // Verify BrowseGroupList attribute exists
        assert!(record.get_attr(SdpAttrId::BROWSE_GROUP_LIST).is_some());

        // Verify it contains the Public Browse Group UUID
        let browse_uuids = record.browse_group_uuids();
        assert_eq!(browse_uuids.len(), 1);
        assert_eq!(browse_uuids[0].as_uuid16(), Some(0x1002));
    }
}
