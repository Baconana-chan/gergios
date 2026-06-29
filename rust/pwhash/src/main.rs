//! Rust port of the MINIX/NetBSD `pwhash` utility.
//!
//! Usage:
//!   pwhash [password]
//!
//! Hashes a password (simplified SHA-256 of stdin/arg).

use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let password = if args.len() > 1 {
        args[1].clone()
    } else {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf).ok();
        buf.trim().to_string()
    };

    if password.is_empty() {
        eprintln!("usage: pwhash [password]");
        std::process::exit(1);
    }

    let hash = simple_hash(&password);
    println!("$5${}", hash); // $5$ = SHA-256 crypt format
}

fn simple_hash(input: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}
