//! Rust port of the MINIX/NetBSD `lam` utility.
//!
//! Usage:
//!   lam [-f min.max] [-s sep] [file ...]
//!
//! Laminate files side by side. Reads files line by line,
//! printing them as columns. -f: format string (min.max field widths).
//! -s: separator between columns (default tab).

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

const USAGE: &str = "usage: lam [-f min.max] [-s sep] [file ...]";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut separator = "\t".to_string();
    let mut formats: Vec<(usize, usize)> = Vec::new(); // (min, max) per column

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        if opt.starts_with("-f") {
            let val = if opt.len() > 2 { opt[2..].to_string() } else {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("{USAGE}"); std::process::exit(1); }
                argv[0].clone()
            };
            if let Some(dot) = val.find('.') {
                let min: usize = val[..dot].parse().unwrap_or(1);
                let max: usize = val[dot + 1..].parse().unwrap_or(min);
                formats.push((min, max));
            }
        } else if opt.starts_with("-s") {
            let val = if opt.len() > 2 { opt[2..].to_string() } else {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("{USAGE}"); std::process::exit(1); }
                argv[0].clone()
            };
            separator = val;
        } else {
            eprintln!("{USAGE}");
            std::process::exit(1);
        }
        argv = &argv[1..];
    }

    let file_names: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdin = io::stdin();
    let mut readers: Vec<Box<dyn BufRead>> = Vec::new();

    for fname in &file_names {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(stdin.lock()))
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("lam: {fname}: {e}"); std::process::exit(1); }
            }
        };
        readers.push(reader);
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();

    // Read lines from each reader
    let mut line_bufs: Vec<Vec<String>> = Vec::new();
    for reader in &mut readers {
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap_or_default()).collect();
        line_bufs.push(lines);
    }

    let max_rows = line_bufs.iter().map(|v| v.len()).max().unwrap_or(0);

    for row in 0..max_rows {
        for (col_idx, lines) in line_bufs.iter().enumerate() {
            if col_idx > 0 {
                write!(out, "{separator}").ok();
            }
            let text = if row < lines.len() {
                lines[row].clone()
            } else {
                String::new()
            };

            if col_idx < formats.len() {
                let (min, max) = formats[col_idx];
                let display = if text.len() > max {
                    format!("{:width$}", &text[..max], width = min)
                } else {
                    format!("{:width$}", text, width = min)
                };
                // Truncate or pad to max
                if display.len() > max {
                    write!(out, "{:width$}", &display[..max], width = max).ok();
                } else {
                    write!(out, "{display}").ok();
                }
            } else {
                write!(out, "{text}").ok();
            }
        }
        writeln!(out).ok();
    }
}
