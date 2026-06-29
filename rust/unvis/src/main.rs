//! Rust port of the MINIX/NetBSD `unvis` utility.
//!
//! Usage:
//!   unvis [file ...]
//!
//! Decodes visually encoded characters (opposite of `vis`).
//! Handles: \\^C (control), \\xXX (hex), \\M-X (meta), \\s (space), \\t (tab)

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

const USAGE: &str = "usage: unvis [file ...]";

/// Decode a visual string back to raw bytes.
/// Returns the decoded bytes and the new index.
fn unvis_decode(line: &str) -> Vec<u8> {
    let bytes: Vec<u8> = line.as_bytes().to_vec();
    let mut result = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'\\' => { result.push(b'\\'); i += 2; }
                b's' => { result.push(b' '); i += 2; }
                b't' => { result.push(b'\t'); i += 2; }
                b'^' => {
                    // \^C
                    if i + 2 < bytes.len() {
                        let c = bytes[i + 2];
                        // Convert to control character
                        let code = if c >= b'@' && c <= b'_' { c - b'@' } else { c };
                        result.push(code);
                        i += 3;
                    } else {
                        result.push(b'\\');
                        i += 1;
                    }
                }
                b'x' | b'X' => {
                    // \xHH
                    if i + 3 < bytes.len() {
                        let hex = std::str::from_utf8(&bytes[i+2..=i+3]).unwrap_or("00");
                        if let Ok(val) = u8::from_str_radix(hex, 16) {
                            result.push(val);
                            i += 4;
                        } else {
                            result.push(b'\\');
                            i += 1;
                        }
                    } else {
                        result.push(b'\\');
                        i += 1;
                    }
                }
                b'M' if i + 2 < bytes.len() && bytes[i + 2] == b'-' => {
                    // \M-X
                    if i + 3 < bytes.len() {
                        if bytes[i + 3] == b'\\' && i + 4 < bytes.len() && bytes[i + 4] == b'^' {
                            // \M-\^C
                            if i + 5 < bytes.len() {
                                let c = bytes[i + 5];
                                let code = if c >= b'@' && c <= b'_' { c - b'@' } else { c };
                                result.push(code | 0x80);
                                i += 6;
                            } else {
                                result.push(b'\\');
                                i += 1;
                            }
                        } else {
                            result.push(bytes[i + 3] | 0x80);
                            i += 4;
                        }
                    } else {
                        result.push(b'\\');
                        i += 1;
                    }
                }
                _ => {
                    // Just pass through the backslash
                    result.push(b'\\');
                    i += 1;
                }
            }
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }

    result
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let argv = &args[1..];
    
    // Check for help flags
    if argv.len() == 1 && (argv[0] == "-h" || argv[0] == "--help") {
        eprintln!("{USAGE}");
        std::process::exit(0);
    }

    let files: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut had_error = false;

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("unvis: {fname}: {e}"); had_error = true; continue; }
            }
        };

        for line_res in reader.lines() {
            let line = match line_res { Ok(l) => l, Err(_) => break };
            let decoded = unvis_decode(&line);
            out.write_all(&decoded).ok();
            writeln!(out).ok();
        }
    }

    if had_error { std::process::exit(1); }
}
