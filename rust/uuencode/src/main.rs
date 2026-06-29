//! Rust port of the MINIX/NetBSD `uuencode` utility.
//!
//! Usage:
//!   uuencode [file] name
//!
//! Encodes binary file for email transmission using uuencode format.

use std::fs::File;
use std::io::{self, Read, Write};
use std::process;

const USAGE: &str = "usage: uuencode [file] name";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let argv = &args[1..];

    let (file_name, output_name) = match argv.len() {
        1 => ("-", argv[0].as_str()),
        2 => (argv[0].as_str(), argv[1].as_str()),
        _ => { eprintln!("{USAGE}"); process::exit(1); }
    };

    let mut reader: Box<dyn Read> = if file_name == "-" {
        Box::new(io::stdin().lock())
    } else {
        match File::open(file_name) {
            Ok(f) => Box::new(f),
            Err(e) => { eprintln!("uuencode: {file_name}: {e}"); process::exit(1); }
        }
    };

    let mut data = Vec::new();
    reader.read_to_end(&mut data).ok();

    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Header
    writeln!(out, "begin 644 {output_name}").ok();

    // Encode data
    let mut i = 0;
    while i < data.len() {
        let chunk_size = std::cmp::min(45, data.len() - i);
        let chunk = &data[i..i + chunk_size];

        // Length byte: encode as (len & 0x3f) + 32
        write!(out, "{}", (chunk_size as u8 + 32) as char).ok();

        // Process 3 bytes at a time
        let mut j = 0;
        while j < chunk.len() {
            let b0 = chunk[j];
            let b1 = if j + 1 < chunk.len() { chunk[j + 1] } else { 0 };
            let b2 = if j + 2 < chunk.len() { chunk[j + 2] } else { 0 };

            let c0 = b0 >> 2;
            let c1 = ((b0 & 0x03) << 4) | (b1 >> 4);
            let c2 = ((b1 & 0x0f) << 2) | (b2 >> 6);
            let c3 = b2 & 0x3f;

            write!(out, "{}{}{}{}",
                (c0 + 32) as u8 as char,
                (c1 + 32) as u8 as char,
                (c2 + 32) as u8 as char,
                (c3 + 32) as u8 as char,
            ).ok();
            j += 3;
        }

        writeln!(out).ok();
        i += chunk_size;
    }

    // Footer
    writeln!(out, "end").ok();
}
