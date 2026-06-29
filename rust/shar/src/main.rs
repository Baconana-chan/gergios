//! Rust port of the MINIX/NetBSD `shar` utility.
//!
//! Usage:
//!   shar file ...
//!
//! Creates a shell archive (shar) that can be extracted with /bin/sh.
//! Uses heredoc encoding with X-prefix for safe transport.

use std::fs;
use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let files: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();

    if files.is_empty() {
        eprintln!("usage: shar file ...");
        std::process::exit(1);
    }

    println!("#!/bin/sh");
    println!("# This is a shell archive.");
    println!("# Created by shar (GergiOS)");
    println!();

    for fname in &files {
        let path = std::path::Path::new(fname);
        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(fname);

        let mut data = Vec::new();
        match fs::File::open(fname) {
            Ok(mut f) => { f.read_to_end(&mut data).ok(); }
            Err(e) => { eprintln!("shar: {fname}: {e}"); std::process::exit(1); }
        }

        println!("echo x - {name}");
        println!("sed 's/^X//' > {name} << 'END_OF_FILE'");

        let text = String::from_utf8_lossy(&data);
        for line in text.lines() {
            println!("X{}", line);
        }
        if data.last() != Some(&b'\n') && !data.is_empty() {
            // Add trailing newline for completeness
            println!("X");
        }

        println!("END_OF_FILE");
        println!();
    }

    println!("exit 0");
}
