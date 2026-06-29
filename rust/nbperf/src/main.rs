//! Rust port of the MINIX/NetBSD `nbperf` utility.
//!
//! Usage:
//!   nbperf [-p num] [-s seed] [keyfile]
//!
//! Generates a minimal perfect hash function for a set of keys.
//! This is a simplified implementation that outputs a C function
//! using a CHD (Compress, Hash, Displace) algorithm.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::{self, BufRead, BufReader};
use std::fs::File;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut num_slots: Option<usize> = None;
    let mut seed: u64 = 12345;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        match opt.as_str() {
            "-p" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: nbperf [-p num] [-s seed] [keyfile]"); std::process::exit(1); }
                num_slots = Some(argv[0].parse().unwrap_or_else(|_| { eprintln!("nbperf: invalid -p value"); std::process::exit(1); }));
            }
            "-s" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: nbperf [-p num] [-s seed] [keyfile]"); std::process::exit(1); }
                seed = argv[0].parse().unwrap_or_else(|_| { eprintln!("nbperf: invalid -s value"); std::process::exit(1); });
            }
            _ => { eprintln!("usage: nbperf [-p num] [-s seed] [keyfile]"); std::process::exit(1); }
        }
        argv = &argv[1..];
    }

    let keyfile = argv.first().map(|s| s.as_str());

    let reader: Box<dyn BufRead> = match keyfile {
        Some("-") | None => Box::new(BufReader::new(io::stdin().lock())),
        Some(f) => {
            match File::open(f) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("nbperf: {f}: {e}"); std::process::exit(1); }
            }
        }
    };

    let keys: Vec<String> = reader.lines()
        .filter_map(|l| l.ok())
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let n = keys.len();
    if n == 0 {
        eprintln!("nbperf: no keys");
        std::process::exit(1);
    }

    let m = num_slots.unwrap_or(n);
    let r = (n as f64 * 1.3).ceil() as usize;

    let mut displacement = vec![0u32; r];
    let mut occupied = vec![false; m];

    for (_i, key) in keys.iter().enumerate() {
        let h1 = hash_key(key, seed) as usize % m;
        let h2 = hash_key(key, seed + 1) as usize % r;

        let mut d = 0u32;
        loop {
            let pos = (h1 + (h2.wrapping_mul(d as usize))) % m;
            if !occupied[pos] {
                occupied[pos] = true;
                displacement[h2] = d;
                break;
            }
            d = d.wrapping_add(1);
        }
    }

    // Generate C output
    println!("/* perfect hash function for {} keys */", n);
    println!("#include <stdint.h>");
    println!("#include <stddef.h>");
    println!();
    println!("static const uint32_t displacement[{}] = {{", r);
    for (i, d) in displacement.iter().enumerate() {
        if i % 8 == 0 { print!("    "); }
        print!("{:>3},", d);
        if i % 8 == 7 { println!(); }
    }
    if r % 8 != 0 { println!(); }
    println!("}};");
    println!();
    println!("static inline size_t");
    println!("hash(const char *key, size_t len) {{");
    println!("    uint32_t h1 = {};", seed);
    println!("    for (size_t i = 0; i < len; i++) {{");
    println!("        h1 = h1 * 33 ^ (unsigned char)key[i];");
    println!("    }}");
    println!("    size_t slot = h1 % {};", m);
    println!("    size_t idx = (h1 >> 16) % {};", r);
    println!("    return (slot + (idx * displacement[idx])) % {};", m);
    println!("}}");
}

fn hash_key(key: &str, seed: u64) -> u64 {
    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    key.hash(&mut hasher);
    hasher.finish()
}
