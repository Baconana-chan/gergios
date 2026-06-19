use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::process;

/// Parse a dd operand like "if=FILE", "of=FILE", "bs=N", "count=N", "seek=N", "skip=N"
fn parse_operand(arg: &str) -> Option<(&str, &str)> {
    let eq_pos = arg.find('=')?;
    if eq_pos == 0 || eq_pos == arg.len() - 1 {
        return None;
    }
    Some((&arg[..eq_pos], &arg[eq_pos + 1..]))
}

/// Parse a numeric value with optional multiplier suffix (b, k, m, g, w)
fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num_part, mult) = match s.chars().last() {
        Some(c) if c.is_ascii_digit() => (s, 1u64),
        Some('b') => (&s[..s.len() - 1], 512u64),
        Some('k') => (&s[..s.len() - 1], 1024u64),
        Some('K') => (&s[..s.len() - 1], 1024u64),
        Some('m') => (&s[..s.len() - 1], 1024u64 * 1024),
        Some('M') => (&s[..s.len() - 1], 1024u64 * 1024),
        Some('g') => (&s[..s.len() - 1], 1024u64 * 1024 * 1024),
        Some('G') => (&s[..s.len() - 1], 1024u64 * 1024 * 1024),
        Some('w') => (&s[..s.len() - 1], 2u64),
        Some(_) => return None,
        None => return None,
    };
    let num: u64 = num_part.parse().ok()?;
    Some(num * mult)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut ifile: Option<String> = None;
    let mut ofile: Option<String> = None;
    let mut bs: u64 = 512;
    let mut count: Option<u64> = None;
    let mut seek: u64 = 0;
    let mut skip: u64 = 0;
    let mut conv: Vec<String> = Vec::new();

    for arg in &args[1..] {
        if let Some((key, val)) = parse_operand(arg) {
            match key {
                "if" => ifile = Some(val.to_string()),
                "of" => ofile = Some(val.to_string()),
                "ibs" | "obs" | "bs" => {
                    if let Some(n) = parse_size(val) {
                        bs = n;
                    } else {
                        eprintln!("dd: invalid number: {}", val);
                        process::exit(1);
                    }
                }
                "count" => {
                    if let Some(n) = parse_size(val) {
                        count = Some(n);
                    } else {
                        eprintln!("dd: invalid number: {}", val);
                        process::exit(1);
                    }
                }
                "seek" => {
                    if let Some(n) = parse_size(val) {
                        seek = n;
                    } else {
                        eprintln!("dd: invalid number: {}", val);
                        process::exit(1);
                    }
                }
                "skip" => {
                    if let Some(n) = parse_size(val) {
                        skip = n;
                    } else {
                        eprintln!("dd: invalid number: {}", val);
                        process::exit(1);
                    }
                }
                "conv" => {
                    for c in val.split(',') {
                        conv.push(c.trim().to_lowercase());
                    }
                }
                _ => {
                    eprintln!("dd: unknown operand: {}", arg);
                    process::exit(1);
                }
            }
        } else {
            eprintln!("dd: invalid operand: {}", arg);
            process::exit(1);
        }
    }

    // Open input
    let input: Box<dyn Read> = match ifile {
        Some(ref name) if name == "-" => Box::new(io::stdin()),
        Some(ref name) => {
            match File::open(Path::new(name)) {
                Ok(f) => Box::new(f),
                Err(e) => {
                    eprintln!("dd: {}: {}", name, e);
                    process::exit(1);
                }
            }
        }
        None => Box::new(io::stdin()),
    };
    let mut reader = input;

    // Open output
    let output: Box<dyn Write> = match ofile {
        Some(ref name) if name == "-" => Box::new(io::stdout()),
        Some(ref name) => {
            match OpenOptions::new().write(true).create(true).open(Path::new(name)) {
                Ok(f) => Box::new(f),
                Err(e) => {
                    eprintln!("dd: {}: {}", name, e);
                    process::exit(1);
                }
            }
        }
        None => Box::new(io::stdout()),
    };
    let mut writer = output;

    // Skip input blocks
    if skip > 0 {
        let skip_bytes = skip * bs;
        let mut skipped = 0u64;
        let mut buf = vec![0u8; bs as usize];
        while skipped < skip_bytes {
            let to_read = (skip_bytes - skipped).min(bs);
            match reader.read(&mut buf[..to_read as usize]) {
                Ok(0) => break,
                Ok(n) => skipped += n as u64,
                Err(_) => break,
            }
        }
    }

    // Seek output
    if seek > 0 {
        let seek_bytes = seek * bs;
        // For regular files, use seek
        if let Some(ref name) = ofile {
            if name != "-" {
                if let Ok(f) = File::options().write(true).open(Path::new(name)) {
                    // We need to seek... but we already opened via OpenOptions.
                    // For simplicity, we pre-write zeros for seek.
                    // In a real dd, seek past blocks without writing.
                    // This is a limitation — proper implementation would use lseek.
                }
            }
        }
        // For simplicity, just write zeros for seek blocks
        let zeros = vec![0u8; bs as usize];
        for _ in 0..seek {
            let _ = writer.write(&zeros);
        }
    }

    let mut buf = vec![0u8; bs as usize];
    let mut total_read: u64 = 0;
    let mut total_written: u64 = 0;
    let mut blocks_read: u64 = 0;
    let mut blocks_written: u64 = 0;
    let max_blocks = count.unwrap_or(u64::MAX);

    loop {
        if blocks_read >= max_blocks {
            break;
        }
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                total_read += n as u64;
                blocks_read += 1;
                // Apply conv flags
                let mut out_buf = &buf[..n];
                // Write
                if let Err(e) = writer.write_all(out_buf) {
                    eprintln!("dd: write error: {}", e);
                    break;
                }
                total_written += n as u64;
                blocks_written += 1;
            }
            Err(e) => {
                eprintln!("dd: read error: {}", e);
                break;
            }
        }
    }

    // Flush output
    if let Err(e) = writer.flush() {
        eprintln!("dd: flush error: {}", e);
    }

    // Report summary to stderr
    eprintln!(
        "{} bytes ({} blocks) read, {} bytes ({} blocks) written",
        total_read, blocks_read, total_written, blocks_written
    );
}
