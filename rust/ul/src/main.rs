//! Rust port of the MINIX/NetBSD `ul` utility.
//!
//! Usage:
//!   ul [-i] [file ...]
//!
//! Reads a file and translates overstrikes (e.g., `_\bX`) into terminal
//! escape sequences for underlining, bold, etc.
//! -i: underline using inverse video instead of _ \b sequences.

use std::fs::File;
use std::io::{self, Read};

const USAGE: &str = "usage: ul [-i] [file ...]";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut use_inverse = false;

    while !argv.is_empty() && argv[0].starts_with('-') {
        let opt = &argv[0];
        if *opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                'i' => use_inverse = true,
                _ => { eprintln!("{USAGE}"); std::process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    let files: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdin = io::stdin();
    let mut had_error = false;

    for fname in &files {
        let mut reader: Box<dyn Read> = if *fname == "-" {
            Box::new(stdin.lock())
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(f),
                Err(e) => { eprintln!("ul: {fname}: {e}"); had_error = true; continue; }
            }
        };

        let mut buf = Vec::new();
        if reader.read_to_end(&mut buf).is_err() {
            had_error = true;
            continue;
        }

        // Process raw bytes to handle overstrikes
        let mut output = String::new();
        let bytes = &buf;
        let mut i = 0;

        while i < bytes.len() {
            if i + 2 < bytes.len() && bytes[i] == b'_' && bytes[i+1] == b'\x08' {
                // Underline: _ \b X -> ANSI underline sequence
                let ch = bytes[i+2] as char;
                if use_inverse {
                    output.push_str(&format!("\x1b[7m{}\x1b[27m", ch));
                } else {
                    output.push_str(&format!("\x1b[4m{}\x1b[24m", ch));
                }
                i += 3;
            } else if i + 2 < bytes.len() && bytes[i+1] == b'\x08' {
                // Overstrike: X \b Y -> bold if X == Y, underline otherwise
                let prev = bytes[i] as char;
                let next = bytes[i+2] as char;
                if prev == next {
                    output.push_str(&format!("\x1b[1m{}\x1b[22m", prev));
                } else {
                    // Usually second char overstrikes first for underline
                    if next == '_' {
                        output.push_str(&format!("\x1b[4m{}\x1b[24m", prev));
                    } else {
                        output.push(next);
                    }
                }
                i += 3;
            } else {
                output.push(bytes[i] as char);
                i += 1;
            }
        }

        print!("{output}");
    }

    if had_error { std::process::exit(1); }
}
