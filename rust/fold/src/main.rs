//! Rust port of the MINIX/NetBSD `fold` utility.
//!
//! Usage:
//!   fold [-w width] [file ...]
//!
//! Wraps input lines to fit a specified width (default 80).

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut width = 80usize;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        if opt.starts_with("-w") {
            let val = if opt.len() > 2 { opt[2..].to_string() } else {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("fold: -w requires argument"); std::process::exit(1); }
                argv[0].clone()
            };
            width = val.parse().unwrap_or_else(|_| {
                eprintln!("fold: invalid width: {val}"); std::process::exit(1);
            });
        } else {
            eprintln!("fold: unknown option: {opt}");
            std::process::exit(1);
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
                Err(e) => { eprintln!("fold: {fname}: {e}"); continue; }
            }
        };

        for line_res in reader.lines() {
            let line = match line_res { Ok(l) => l, Err(_) => break };
            let mut pos = 0;
            for ch in line.chars() {
                if pos >= width {
                    let _ = writeln!(out);
                    pos = 0;
                }
                let _ = write!(out, "{ch}");
                if ch == '\t' {
                    pos = (pos + 8) / 8 * 8;
                } else {
                    pos += 1;
                }
            }
            let _ = writeln!(out);
        }
    }
}
