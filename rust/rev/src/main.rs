//! Rust port of the MINIX/NetBSD `rev` utility.
//!
//! Usage:
//!   rev [file ...]
//!
//! Copies each file to stdout, reversing the characters of every line.
//! If no file is given, reads from stdin.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let files: Vec<&str> = if args.len() < 2 {
        vec!["-"]
    } else {
        args[1..].iter().map(|s| s.as_str()).collect()
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut had_error = false;

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => {
                    eprintln!("rev: {fname}: {e}");
                    had_error = true;
                    continue;
                }
            }
        };

        for line_res in reader.lines() {
            let line = match line_res {
                Ok(l) => l,
                Err(_) => break,
            };
            let reversed: String = line.chars().rev().collect();
            writeln!(out, "{reversed}").ok();
        }
    }

    if had_error {
        std::process::exit(1);
    }
}
