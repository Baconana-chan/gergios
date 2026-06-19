//! Rust port of the MINIX/NetBSD `expand` utility.
//!
//! Usage:
//!   expand [-t tabstop] [file ...]
//!
//! Converts tabs to spaces, reading from files or stdin.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut tabstop = 8usize;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        if opt.starts_with("-t") {
            let val = if opt.len() > 2 { opt[2..].to_string() } else {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("expand: -t requires argument"); std::process::exit(1); }
                argv[0].clone()
            };
            tabstop = val.parse().unwrap_or_else(|_| {
                eprintln!("expand: invalid tabstop: {val}"); std::process::exit(1);
            });
        } else {
            eprintln!("expand: unknown option: {opt}");
            std::process::exit(1);
        }
        argv = &argv[1..];
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
                Err(e) => { eprintln!("expand: {fname}: {e}"); had_error = true; continue; }
            }
        };

        for line_res in reader.lines() {
            let line = match line_res { Ok(l) => l, Err(_) => break };
            let mut col = 0usize;
            for ch in line.chars() {
                if ch == '\t' {
                    let spaces = tabstop - (col % tabstop);
                    for _ in 0..spaces { let _ = write!(out, " "); }
                    col += spaces;
                } else {
                    let _ = write!(out, "{ch}");
                    col += 1;
                }
            }
            let _ = writeln!(out);
        }
    }

    if had_error { std::process::exit(1); }
}
