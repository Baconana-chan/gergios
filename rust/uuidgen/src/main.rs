//! Rust port of the MINIX/NetBSD `uuidgen` utility.
//!
//! Usage:
//!   uuidgen [count]
//!
//! Generates UUIDs (Universally Unique Identifiers) using
//! random-based (v4) UUIDs.

const USAGE: &str = "usage: uuidgen [count]";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let argv = &args[1..];

    let count = if argv.is_empty() {
        1
    } else if argv.len() == 1 {
        argv[0].parse().unwrap_or_else(|_| {
            eprintln!("{USAGE}"); std::process::exit(1);
        })
    } else {
        eprintln!("{USAGE}"); std::process::exit(1);
    };

    for _ in 0..count {
        let uuid = generate_uuid_v4();
        println!("{uuid}");
    }
}

fn generate_uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    // Simple PRNG based on time + hashed
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let seed = now.as_nanos();

    // Simple LCG
    let mut state = seed as u64;
    let mut next = || -> u64 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        state >> 33
    };

    let mut bytes = [0u8; 16];
    for b in &mut bytes {
        *b = next() as u8;
    }

    // Set version 4 (random)
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    // Set variant (RFC 4122)
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3],
        bytes[4], bytes[5],
        bytes[6], bytes[7],
        bytes[8], bytes[9],
        bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    )
}
