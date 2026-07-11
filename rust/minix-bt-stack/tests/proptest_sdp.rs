//! Property-based tests for Bluetooth SDP encoding.
//!
//! Uses the proptest API directly (TestRunner::run()) instead of the
//! proptest! macro, because the macro in v1.11.0 doesn't generate
//! #[test] functions in integration tests.

use proptest::prelude::*;
use proptest::test_runner::{TestRunner, Config};
use minix_bt_stack::{DataElement, BtUuid, ServiceRecord, SdpAttrId};

// ── Non-parameterized tests ───────────────────────────────────────────
// These run outside the TestRunner since they don't need random inputs.

#[test]
fn nil_encodes_as_single_byte() {
    assert_eq!(DataElement::Nil.encode(), vec![0x00]);
}

#[test]
fn nil_encoded_len_is_one() {
    assert_eq!(DataElement::Nil.encoded_len(), 1);
}

// ── Parameterized proptests (using TestRunner::run()) ──────────────────

#[test]
fn bool_encoding_correct() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(any::<bool>(),),
        |(value,)| {
            let encoded = DataElement::Bool(value).encode();
            prop_assert_eq!(encoded[0], 0x28);
            prop_assert_eq!(encoded[1], if value { 0x01 } else { 0x00 });
            prop_assert_eq!(encoded.len(), 2);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn unsigned_int_header_type() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(0u64..=0xFFFFFFFFu64,),
        |(val,)| {
            let encoded = DataElement::UnsignedInt(val, 4).encode();
            prop_assert_eq!(encoded[0] >> 3, 0x01);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn uuid_header_type() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(0u16..=0xFFFFu16,),
        |(short,)| {
            let encoded = DataElement::Uuid(BtUuid::from_uuid16(short)).encode();
            prop_assert_eq!(encoded[0] >> 3, 0x03);
            prop_assert_eq!(encoded[0] & 0x07, 1);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn string_header_type() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(proptest::collection::vec(any::<u8>(), 0..=32),),
        |(data,)| {
            let encoded = DataElement::String(data.clone()).encode();
            prop_assert_eq!(encoded[0] >> 3, 0x04);
            if data.len() <= 0xFF {
                prop_assert_eq!(encoded[0] & 0x07, 5);
                if !data.is_empty() {
                    prop_assert_eq!(encoded[1] as usize, data.len());
                }
            }
            Ok(())
        },
    ).unwrap();
}

#[test]
fn service_record_attr_roundtrip() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(any::<u32>(), 0u16..=0xFFFFu16, proptest::collection::vec(any::<u8>(), 0..=16)),
        |(handle, attr_id, value)| {
            let mut record = ServiceRecord::new(handle);
            let elem = DataElement::String(value);
            record.set_attr(attr_id, elem.clone());
            let retrieved = record.get_attr(attr_id);
            prop_assert!(retrieved.is_some());
            prop_assert_eq!(retrieved.unwrap().encode(), elem.encode());
            Ok(())
        },
    ).unwrap();
}

#[test]
fn missing_attr_returns_none() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(any::<u32>(), 0u16..=0xFFFFu16),
        |(handle, attr_id)| {
            let record = ServiceRecord::new(handle);
            prop_assert!(record.get_attr(attr_id).is_none());
            Ok(())
        },
    ).unwrap();
}

#[test]
fn url_type_descriptor() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(proptest::collection::vec(any::<u8>(), 0..=16),),
        |(data,)| {
            let encoded = DataElement::Url(data).encode();
            prop_assert_eq!(encoded[0] >> 3, 0x08);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn service_class_uuid_detection() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(0u16..=0xFFFFu16,),
        |(uuid_short,)| {
            let uuid = BtUuid::from_uuid16(uuid_short);
            let mut record = ServiceRecord::new(0x10000);
            record.set_attr(SdpAttrId::SERVICE_CLASS_ID_LIST,
                DataElement::Seq(vec![DataElement::Uuid(uuid)]));
            let uuids = record.service_class_uuids();
            prop_assert_eq!(uuids.len(), 1);
            prop_assert_eq!(uuids[0].as_uuid16(), Some(uuid_short));
            Ok(())
        },
    ).unwrap();
}

#[test]
fn encode_never_panics() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(any::<u64>(), proptest::collection::vec(any::<u8>(), 0..=64)),
        |(val, string_data)| {
            let _ = DataElement::UnsignedInt(val, 4).encode();
            let _ = DataElement::String(string_data).encode();
            let _ = DataElement::Uuid(BtUuid::from_uuid16(val as u16)).encode();
            let _ = DataElement::Url(vec![]).encode();
            Ok(())
        },
    ).unwrap();
}

#[test]
fn new_record_no_attributes() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(any::<u32>(),),
        |(handle,)| {
            let record = ServiceRecord::new(handle);
            prop_assert!(record.get_attr(0x0000).is_none());
            prop_assert!(record.get_attr(0x0001).is_none());
            prop_assert!(record.get_attr(0x0100).is_none());
            Ok(())
        },
    ).unwrap();
}
