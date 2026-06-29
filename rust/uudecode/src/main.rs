//! Rust port of the MINIX/NetBSD `uudecode` utility.
//!
//! Usage:
//!   uudecode [file ...]
//!
//! Decodes uuencoded files. If no file is given, reads from stdin.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let files: Vec<&str> = if args.len() < 2 {
        vec!["-"]
    } else {
        args[1..].iter().map(|s| s.as_str()).collect()
    };

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("uudecode: {fname}: {e}"); continue; }
            }
        };

        let mut in_body = false;
        let mut output_name = String::new();
        let mut decoded: Vec<u8> = Vec::new();

        for line_res in reader.lines() {
            let line = match line_res { Ok(l) => l, Err(_) => break };
            let trimmed = line.trim().to_string();

            if trimmed.starts_with("begin ") {
                // Parse: begin 644 filename
                in_body = true;
                let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();
                if parts.len() >= 3 {
                    output_name = parts[2].to_string();
                }
                continue;
            }

            if trimmed == "end" {
                break;
            }

            if !in_body { continue; }
            if trimmed.is_empty() { continue; }

            // Decode uuencoded line
            let bytes = trimmed.as_bytes();
            let line_len = (bytes[0] as usize - 32) & 0x3f;

            let mut chunk = Vec::new();
            let mut i = 1;
            while i < bytes.len() && chunk.len() < line_len {
                if bytes[i] < 32 || bytes[i] > 96 { i += 1; continue; }

                if i + 3 < bytes.len() {
                    let c0 = (bytes[i] - 32) & 0x3f;
                    let c1 = (bytes[i + 1] - 32) & 0x3f;
                    let c2 = (bytes[i + 2] - 32) & 0x3f;
                    let c3 = (bytes[i + 3] - 32) & 0x3f;

                    chunk.push((c0 << 2) | (c1 >> 4));
                    if chunk.len() < line_len {
                        chunk.push(((c1 & 0x0f) << 4) | (c2 >> 2));
                    }
                    if chunk.len() < line_len {
                        chunk.push(((c2 & 0x03) << 6) | c3);
                    }
                }
                i += 4;
            }

            decoded.extend(chunk);
        }

        // Write output file
        if !output_name.is_empty() && !decoded.is_empty() {
            let out_name = if output_name == "-" { "output.bin".to_string() } else { output_name };
            match File::create(&out_name) {
                Ok(mut f) => { f.write_all(&decoded).ok(); }
                Err(e) => { eprintln!("uudecode: {out_name}: {e}"); }
            }
        }
    }
}
