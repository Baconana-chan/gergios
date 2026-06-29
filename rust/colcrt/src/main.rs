//! Rust port of the MINIX/NetBSD `colcrt` utility.
//!
//! Usage:
//!   colcrt [-2] [file ...]
//!
//! Filters nroff output for CRT previewing. Underlined characters
//! become `_` prefix, bold becomes repeated characters.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

const USAGE: &str = "usage: colcrt [-2] [file ...]";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut no_underline = false;

    while !argv.is_empty() && argv[0].starts_with('-') {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                '2' => no_underline = true,
                _ => { eprintln!("{USAGE}"); std::process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    let files: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("colcrt: {fname}: {e}"); continue; }
            }
        };

        for line_res in reader.lines() {
            let line = match line_res { Ok(l) => l, Err(_) => break };
            let bytes = line.as_bytes();
            let mut output = Vec::new();
            let mut under_chars: Vec<usize> = Vec::new();
            let mut i = 0;

            while i < bytes.len() {
                if i + 2 < bytes.len() && bytes[i + 1] == b'\x08' {
                    // Overstrike: X \b Y
                    let prev = bytes[i] as char;
                    let next = bytes[i + 2] as char;
                    if next == '_' || prev == '_' {
                        // Underline
                        let ch = if prev != '_' { prev } else { next };
                        if !no_underline {
                            under_chars.push(output.len());
                            output.push(ch as u8);
                        } else {
                            output.push(ch as u8);
                        }
                    } else if prev == next || prev == ' ' {
                        // Bold (overstrike same char)
                        output.push(prev as u8);
                    } else if next == ' ' {
                        output.push(prev as u8);
                    } else {
                        output.push(next as u8);
                    }
                    i += 3;
                } else {
                    output.push(bytes[i]);
                    i += 1;
                }
            }

            // Write the text line
            out.write_all(&output).ok();
            writeln!(out).ok();

            // Write underline line if needed
            if !no_underline && !under_chars.is_empty() {
                let mut uline: Vec<u8> = vec![b' '; output.len()];
                for &pos in &under_chars {
                    if pos < uline.len() {
                        uline[pos] = b'|';
                    }
                }
                // Trim trailing spaces
                while uline.last() == Some(&b' ') {
                    uline.pop();
                }
                // Only output if there are non-space chars and -2 wasn't given
                if !no_underline {
                    writeln!(out, "{}", String::from_utf8_lossy(&uline)).ok();
                }
            }
        }
    }
}
