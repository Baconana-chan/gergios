//! Property-based tests for `minix-rs` Message encoding/decoding.

use proptest::prelude::*;
use minix_rs::Message;

proptest! {
    fn write_read_i32_roundtrip(
        offset in 0usize..52usize,
        val in any::<i32>(),
    ) {
        let mut msg = Message::new();
        msg.write_i32(offset, val);
        prop_assert_eq!(msg.read_i32(offset), val);
    }

    fn writes_do_not_overlap(
        offset_a in 0usize..48usize,
        offset_b in 0usize..48usize,
        val_a in any::<i32>(),
        val_b in any::<i32>(),
    ) {
        let overlap = (offset_a < offset_b + 4) && (offset_b < offset_a + 4);
        prop_assume!(!overlap);
        let mut msg = Message::new();
        msg.write_i32(offset_a, val_a);
        msg.write_i32(offset_b, val_b);
        prop_assert_eq!(msg.read_i32(offset_a), val_a);
        prop_assert_eq!(msg.read_i32(offset_b), val_b);
    }    fn message_fields_roundtrip(
        source in 0i32..256i32,
        msg_type in 0i32..0x1001i32,
        payload_vec in proptest::collection::vec(any::<u8>(), 56),
    ) {
        let mut payload = [0u8; 56];
        payload.copy_from_slice(&payload_vec);
        let mut msg = Message::new();
        msg.m_source = source;
        msg.m_type = msg_type;
        msg.payload = payload;
        prop_assert_eq!(msg.source(), source);
        prop_assert_eq!(msg.msg_type(), msg_type);
        prop_assert_eq!(msg.payload, payload);
    }

    fn check_offset_bounds(offset in 0usize..64usize, size in 0usize..64usize) {
        let msg = Message::new();
        prop_assert_eq!(msg.check_offset(offset, size),
            if offset + size <= 56 { Some(()) } else { None });
    }

    fn write_read_u64_roundtrip(offset in 0usize..48usize, val in any::<u64>()) {
        let mut msg = Message::new();
        msg.write_u64(offset, val);
        prop_assert_eq!(msg.read_u64(offset), val);
    }

    fn write_read_ptr_roundtrip(offset in 0usize..48usize, val in any::<usize>()) {
        let mut msg = Message::new();
        msg.write_ptr(offset, val);
        prop_assert_eq!(msg.read_ptr(offset), val);
    }

    fn multiple_writes_preserve_values(
        vals in proptest::collection::vec(
            (0usize..48usize, any::<i32>()), 1..=4,
        ),
    ) {
        let overlap_free: Vec<_> = vals.iter()
            .filter(|(a, _)| vals.iter().all(|(b, _)| a == b || *a + 4 <= *b || *b + 4 <= *a))
            .cloned().collect();
        prop_assume!(!overlap_free.is_empty());
        let mut msg = Message::new();
        for (offset, val) in &overlap_free { msg.write_i32(*offset, *val); }
        for (offset, val) in &overlap_free { prop_assert_eq!(msg.read_i32(*offset), *val); }
    }
}

// ── Non-parameterized property tests (run outside proptest! macro) ─────
#[test]
fn message_size_invariant() {
    assert_eq!(std::mem::size_of::<Message>(), 64);
}

#[test]
fn fresh_message_is_zeroed() {
    let msg = Message::new();
    assert_eq!(msg.m_source, 0);
    assert_eq!(msg.m_type, 0);
    assert_eq!(msg.payload, [0u8; 56]);
}
