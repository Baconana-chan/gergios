//! Rust port of the MINIX/NetBSD `tail` utility.
//!
//! Usage:
//!   tail [-n count] [file ...]
//!
//! Displays the last count (default 10) lines of each file.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut nlines: usize = 10;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = &argv[0];
        if opt == "-q" || opt == "-v" {
            // Quiet/verbose — silently accepted for compat
            argv = &argv[1..];
            continue;
        }
        if opt.starts_with("-n") {
            if opt.len() > 2 {
                nlines = opt[2..].parse().unwrap_or_else(|_| {
                    eprintln!("tail: invalid number: {}", &opt[2..]);
                    std::process::exit(1);
                });
            } else {
                argv = &argv[1..];
                if argv.is_empty() {
                    eprintln!("tail: option requires an argument: -n");
                    std::process::exit(1);
                }
                nlines = argv[0].parse().unwrap_or_else(|_| {
                    eprintln!("tail: invalid number: {}", argv[0]);
                    std::process::exit(1);
                });
            }
        } else {
            eprintln!("tail: unknown option: {opt}");
            std::process::exit(1);
        }
        argv = &argv[1..];
    }

    let files: Vec<&str> = if argv.is_empty() {
        vec!["-"]
    } else {
        argv.iter().map(|s| s.as_str()).collect()
    };

    let multiple = files.len() > 1;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut had_error = false;

    for (idx, filename) in files.iter().enumerate() {
        if multiple {
            if idx > 0 { let _ = writeln!(out); }
            let _ = writeln!(out, "==> {filename} <==");
        }

        let result: io::Result<()> = if *filename == "-" {
            let mut content = Vec::new();
            io::stdin().lock().read_to_end(&mut content)?;
            let text = String::from_utf8_lossy(&content);
            let lines: Vec<&str> = text.lines().collect();
            let start = if nlines >= lines.len() { 0 } else { lines.len() - nlines };
            for line in &lines[start..] {
                writeln!(out, "{line}")?;
            }
            Ok(())
        } else {
            let mut file = File::open(filename)?;
            let mut content = Vec::new();
            file.read_to_end(&mut content)?;
            let text = String::from_utf8_lossy(&content);
            let lines: Vec<&str> = text.lines().collect();
            let start = if nlines >= lines.len() { 0 } else { lines.len() - nlines };
            for line in &lines[start..] {
                writeln!(out, "{line}")?;
            }
            Ok(())
        };

        if let Err(e) = result {
            eprintln!("tail: {filename}: {e}");
            had_error = true;
        }
    }

    if had_error { std::process::exit(1); }
}
