#![no_main]

use libfuzzer_sys::fuzz_target;
use minix_rs::Message;

/// Fuzz the Message struct — validate that all methods handle arbitrary data
/// without panicking or causing UB.
///
/// This target fuzzes:
/// - `check_offset()` with random offsets and sizes
/// - `read_i32()`, `read_u64()`, `read_ptr()` at random offsets
/// - `is_pm_call()`, `is_vfs_call()`, `is_kernel_call()`, `is_notify()`
/// - `is_pm_reply()`, `is_vfs_reply()`
/// - `has_source()`, `is_valid_source()`
fuzz_target!(|data: &[u8]| {
    if data.len() < 64 {
        return; // need at least a full Message
    }

    // Interpret first 64 bytes as a Message
    let mut msg = Message::new();
    msg.m_source = i32::from_ne_bytes([data[0], data[1], data[2], data[3]]);
    msg.m_type   = i32::from_ne_bytes([data[4], data[5], data[6], data[7]]);
    // Copy remaining 56 bytes as payload
    let payload_len = (data.len() - 8).min(56);
    msg.payload[..payload_len].copy_from_slice(&data[8..8 + payload_len]);

    // Run all validation methods (must not panic)
    let _ = msg.has_source();
    let _ = msg.is_valid_source();
    let _ = msg.is_pm_call();
    let _ = msg.is_vfs_call();
    let _ = msg.is_kernel_call();
    let _ = msg.is_notify();
    let _ = msg.is_pm_reply();
    let _ = msg.is_vfs_reply();

    // Test bounds checking with fuzzed offsets
    let offset = (data[0] as usize) % 64;
    let size   = (data[1] as usize) % 64;
    let _ = msg.check_offset(offset, size);
    if msg.check_offset(offset, 4).is_some() {
        let _ = msg.read_i32(offset);
    }
    if msg.check_offset(offset, 8).is_some() {
        let _ = msg.read_u64(offset);
        let _ = msg.read_ptr(offset);
    }
});
