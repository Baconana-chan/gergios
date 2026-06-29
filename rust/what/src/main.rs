//! Rust port of the MINIX/NetBSD `what` utility.
//!
//! Usage:
//!   what [file ...]
//!
//! Searches files for SCCS version strings (@(#) ...).
//! Prints everything between @(#) and the end of the string.

use std::fs::File;
use std::io::{self, Read};

const USAGE: &str = "usage: what [file ...]";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let files: Vec<&str> = if args.len() < 2 {
        eprintln!("{USAGE}");
        std::process::exit(1);
    } else {
        args[1..].iter().map(|s| s.as_str()).collect()
    };

    for fname in &files {
        let mut reader: Box<dyn Read> = if *fname == "-" {
            Box::new(io::stdin().lock())
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(f),
                Err(e) => { eprintln!("what: {fname}: {e}"); continue; }
            }
        };

        let mut data = Vec::new();
        reader.read_to_end(&mut data).ok();

        // Search for @(#) (SCCS marker)
        let mut i = 0;
        while i + 3 < data.len() {
            if data[i] == b'@' && data[i + 1] == b'(' && data[i + 2] == b'#' && data[i + 3] == b')' {
                // Found @(#) - read until null, newline, ">", or max 256 chars
                let start = i + 4;
                let mut end = start;
                while end < data.len() && end - start < 256 {
                    let c = data[end];
                    if c == 0 || c == b'\n' || c == b'"' || c == b'>' { break; }
                    end += 1;
                }
                let version_str = String::from_utf8_lossy(&data[start..end]);
                if !version_str.trim().is_empty() {
                    println!("\t{}", version_str);
                }
                i = end;
            }
            i += 1;
        }
    }
}
