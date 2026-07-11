//! # proptest-helpers — Shared proptest strategies for GergiOS components
//!
//! Provides reusable proptest strategies for:
//!
//! - **IPC** (`minix-rs`): `Message` payloads, endpoints, message types
//! - **ext4** (`ext4-core`): extent trees, directory entries, superblocks
//! - **Network** (`net-parse`): TCP/UDP/DNS packet bytes
//! - **Bluetooth** (`minix-bt-stack`): SDP records, DataElements, UUIDs

#![cfg_attr(not(test), no_std)]
#![cfg(test)]

use proptest::prelude::*;

// ============================================================================
// IPC — minix-rs strategies
// ============================================================================

/// Generate a random valid MINIX endpoint (0..256 or -24..-1).
pub fn endpoint() -> impl Strategy<Value = i32> {
    prop_oneof![
        0i32..256i32,                        // server endpoints
        (-24i32..0i32).prop_filter("no-zero", |v| *v != 0), // kernel tasks
    ]
}

/// Generate a random valid PM message type (PM_BASE + 0..=127).
pub fn pm_msg_type() -> impl Strategy<Value = i32> {
    (0i32..0x80u32 as i32).prop_map(|n| 0x000 + n)
}

/// Generate a random valid VFS message type (VFS_BASE + 0..=127).
pub fn vfs_msg_type() -> impl Strategy<Value = i32> {
    (0i32..0x80u32 as i32).prop_map(|n| 0x100 + n)
}

/// Generate a random valid kernel call type (KERNEL_CALL + 0..=127).
pub fn kernel_msg_type() -> impl Strategy<Value = i32> {
    (0i32..0x80u32 as i32).prop_map(|n| 0x600 + n)
}

/// Generate a random message type (any valid range or notify).
pub fn msg_type() -> impl Strategy<Value = i32> {
    prop_oneof![
        pm_msg_type(),
        vfs_msg_type(),
        kernel_msg_type(),
        Just(minix_rs::NOTIFY_MESSAGE),
    ]
}

/// Generate a random 56-byte IPC message payload.
pub fn msg_payload() -> impl Strategy<Value = [u8; 56]> {
    proptest::collection::vec(any::<u8>(), 56).prop_map(|v| {
        let mut arr = [0u8; 56];
        arr.copy_from_slice(&v);
        arr
    })
}

/// Generate a complete random `minix_rs::Message`.
pub fn message() -> impl Strategy<Value = minix_rs::Message> {
    (endpoint(), msg_type(), msg_payload()).prop_map(|(src, typ, payload)| {
        let mut msg = minix_rs::Message::new();
        msg.m_source = src;
        msg.m_type = typ;
        msg.payload = payload;
        msg
    })
}

/// Generate an (offset, value) pair for write_i32, with random valid offset.
pub fn i32_write_op() -> impl Strategy<Value = (usize, i32)> {
    (0usize..52usize, any::<i32>())
}

/// Generate an (offset, size) pair for check_offset, with random valid params.
pub fn offset_check_op() -> impl Strategy<Value = (usize, usize)> {
    (0usize..56usize, 0usize..56usize)
}

// ============================================================================
// ext4-core strategies
// ============================================================================

/// Generate a valid extent header (magic = 0xF30A, depth 0, entries 0..4).
pub fn extent_header() -> impl Strategy<Value = ext4_core::Ext4ExtentHeader> {
    (0u16..=4u16, 4u16..=4u16).prop_map(|(entries, max_entries)| {
        ext4_core::Ext4ExtentHeader {
            eh_magic: ext4_core::EXT4_EXTENT_MAGIC,
            eh_entries: entries,
            eh_max: max_entries,
            eh_depth: 0,
            eh_generation: 0,
        }
    })
}

/// Generate a random extent leaf entry with valid ee_len (1..=32768).
pub fn extent_entry() -> impl Strategy<Value = ext4_core::Ext4Extent> {
    (0u32..100000u32, 1u16..=1000u16, any::<u32>()).prop_map(|(block, len, start_lo)| {
        ext4_core::Ext4Extent {
            ee_block: block,
            ee_len: len,
            ee_start_hi: 0,
            ee_start_lo: start_lo,
        }
    })
}

/// Generate a list of 0..4 non-overlapping extent entries (simplified).
pub fn extent_list() -> impl Strategy<Value = Vec<ext4_core::Ext4Extent>> {
    proptest::collection::vec(extent_entry(), 0..=4).prop_map(|extents| {
        // Sort by ee_block
        let mut sorted = extents;
        sorted.sort_by_key(|e| e.ee_block);
        sorted
    })
}

/// Generate a random filename (1..32 bytes, printable ASCII).
pub fn dir_filename() -> impl Strategy<Value = String> {
    "[a-z0-9._-]{1,32}"
}

/// Generate a random directory entry combination.
pub fn dir_entry_combo() -> impl Strategy<Value = (u32, String, u8)> {
    (1u32..100000u32, dir_filename(), prop_oneof![
        Just(ext4_core::EXT4_FT_REG_FILE),
        Just(ext4_core::EXT4_FT_DIR),
        Just(ext4_core::EXT4_FT_SYMLINK),
    ])
}

/// Generate random ext4 superblock feature flags (compatible combinations).
pub fn sb_features() -> impl Strategy<Value = (u32, u32)> {
    let incompat = (
        ext4_core::EXT4_FEATURE_INCOMPAT_FILETYPE |
        ext4_core::EXT4_FEATURE_INCOMPAT_EXTENTS |
        ext4_core::EXT4_FEATURE_INCOMPAT_FLEX_BG
    );
    let ro_compat = ext4_core::EXT4_FEATURE_RO_COMPAT_SPARSE_SUPER;

    Just((incompat, ro_compat))
}

// ============================================================================
// (net-parse and BPF strategies reserved for future use)
// ============================================================================

// ============================================================================
// Bluetooth SDP strategies
// ============================================================================

use minix_bt_stack::{BtUuid, DataElement, DataElementType, ServiceRecord, SdpAttrId};

/// Generate a random 6-byte Bluetooth address.
pub fn bdaddr_bytes() -> impl Strategy<Value = [u8; 6]> {
    proptest::array::uniform6(any::<u8>())
}

/// Generate a random UUID variant.
pub fn bt_uuid() -> impl Strategy<Value = BtUuid> {
    prop_oneof![
        (0u16..0xFFFFu16).prop_map(BtUuid::from_uuid16),
        (0u32..0xFFFFFFFFu32).prop_map(BtUuid::from_uuid32),
        proptest::array::uniform16(any::<u8>()).prop_map(BtUuid::from_bytes),
    ]
}

/// Generate a random SDP Data Element.
pub fn data_element() -> impl Strategy<Value = DataElement> {
    let leaf = prop_oneof![
        Just(DataElement::Nil),
        proptest::array::uniform1(any::<u8>()).prop_map(|v| DataElement::Bool(v[0] != 0)),
        (0u64..0xFFFFFFFFu64).prop_map(|v| DataElement::UnsignedInt(v, 4)),
        (0i64..0x7FFFFFFFi64).prop_map(|v| DataElement::SignedInt(v, 4)),
        bt_uuid().prop_map(DataElement::Uuid),
        proptest::collection::vec(any::<u8>(), 0..=32).prop_map(DataElement::String),
        proptest::collection::vec(any::<u8>(), 0..=32).prop_map(DataElement::Url),
    ];
    // Recursive: sequences containing data elements
    leaf.prop_recursive(
        2,    // depth
        16,   // max size
        10,   // items per collection
        |inner| prop_oneof![
            proptest::collection::vec(inner.clone(), 0..=4).prop_map(DataElement::Seq),
            proptest::collection::vec(inner, 0..=4).prop_map(DataElement::Alt),
        ],
    )
}

/// Generate a random SDP attribute ID.
pub fn sdp_attr_id() -> impl Strategy<Value = u16> {
    prop_oneof![
        0u16..=0x000Du16,             // standard IDs
        (0x0100u16..0xFFFFu16),       // user-defined
    ]
}

/// Generate a random SDP `ServiceRecord`.
pub fn service_record() -> impl Strategy<Value = ServiceRecord> {
    (0u32..0xFFFFFFFFu32, proptest::collection::vec(
        (sdp_attr_id(), data_element()),
        1..=8,
    )).prop_map(|(handle, attrs)| {
        let mut record = ServiceRecord::new(handle);
        for (id, value) in attrs {
            record.set_attr(id, value);
        }
        record
    })
}

/// Generate a minimal valid `ServiceRecord` with ServiceClassIDList.
pub fn minimal_service_record() -> impl Strategy<Value = ServiceRecord> {
    bt_uuid().prop_map(|uuid| {
        let mut record = ServiceRecord::new(0x10000);
        record.set_attr(
            SdpAttrId::SERVICE_CLASS_ID_LIST,
            DataElement::Seq(vec![DataElement::Uuid(uuid)]),
        );
        record
    })
}
