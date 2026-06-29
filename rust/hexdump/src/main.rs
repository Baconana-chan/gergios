//! Rust port of the MINIX/NetBSD `hexdump` utility.
//!
//! Usage:
//!   hexdump [file ...]
//!
//! Displays file contents in hexadecimal (canonical format:
//! offset  hex_bytes  ASCII).

use std::fs::File;
use std::io::{self, Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let files: Vec<&str> = if args.len() < 2 {
        vec!["-"]
    } else {
        args[1..].iter().map(|s| s.as_str()).collect()
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut offset: usize = 0;

    for fname in &files {
        let mut reader: Box<dyn Read> = if *fname == "-" {
            Box::new(io::stdin().lock())
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(f),
                Err(e) => { eprintln!("hexdump: {fname}: {e}"); continue; }
            }
        };

        let mut buf = [0u8; 16];
        loop {
            let n = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(_) => break,
            };

            // Offset
            write!(out, "{:08x}  ", offset).ok();

            // Hex bytes
            for i in 0..16 {
                if i < n {
                    write!(out, "{:02x} ", buf[i]).ok();
                } else {
                    write!(out, "   ").ok();
                }
                if i == 7 { write!(out, " ").ok(); }
            }

            write!(out, " |").ok();

            // ASCII representation
            for i in 0..n {
                let c = buf[i];
                if c >= 0x20 && c <= 0x7e {
                    write!(out, "{}", c as char).ok();
                } else {
                    write!(out, ".").ok();
                }
            }

            writeln!(out, "|").ok();
            offset += n;
        }
    }

    // Print final offset
    writeln!(out, "{:08x}", offset).ok();
}
