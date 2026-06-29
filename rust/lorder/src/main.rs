//! Rust port of the MINIX/NetBSD `lorder` utility.
//!
//! Usage:
//!   lorder file ...
//!
//! Lists dependencies for library ordering (for ar/ld).

use std::fs;
use std::io::{BufRead, BufReader};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let files: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();

    if files.is_empty() {
        eprintln!("usage: lorder file ...");
        std::process::exit(1);
    }

    for fname in &files {
        let defined = find_symbols(fname);
        for d in &defined {
            println!("{} {}", d, fname);
        }
    }
}

fn find_symbols(path: &str) -> Vec<String> {
    let mut symbols = Vec::new();
    let file = match fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return symbols,
    };
    let reader = BufReader::new(file);

    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        if line.contains(" T ") || line.contains(" t ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 {
                symbols.push(parts[2].to_string());
            }
        }
    }
    symbols
}
