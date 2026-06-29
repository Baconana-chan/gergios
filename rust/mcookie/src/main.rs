//! Rust port of the MINIX/NetBSD `mcookie` utility.
//!
//! Usage:
//!   mcookie [count]
//!
//! Generates random 128-bit cookies in hexadecimal format.

use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let count = if args.len() > 1 {
        args[1].parse::<usize>().unwrap_or_else(|_| {
            eprintln!("usage: mcookie [count]");
            std::process::exit(1);
        })
    } else {
        1
    };

    for _ in 0..count {
        println!("{:032x}", rand_u128());
    }
}

fn rand_u128() -> u128 {
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    let mut state = seed as u64;
    let mut output: u128 = 0;

    for _ in 0..2 {
        for _ in 0..4 {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            output = (output << 16) | ((state >> 48) as u128);
        }
    }

    output
}
