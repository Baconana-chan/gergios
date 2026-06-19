//! Rust port of the MINIX/NetBSD `head` utility.
//!
//! Usage:
//!   head [-n count] [file ...]
//!
//! Displays the first count (default 10) lines of each file.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut nlines: usize = 10;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = &argv[0];
        if opt.starts_with("-n") {
            if opt.len() > 2 {
                // -n5 style
                if let Ok(n) = opt[2..].parse() {
                    nlines = n;
                } else {
                    eprintln!("head: invalid number: {}", &opt[2..]);
                    std::process::exit(1);
                }
            } else {
                // -n 5 style
                argv = &argv[1..];
                if argv.is_empty() {
                    eprintln!("head: option requires an argument: -n");
                    std::process::exit(1);
                }
                if let Ok(n) = argv[0].parse() {
                    nlines = n;
                } else {
                    eprintln!("head: invalid number: {}", argv[0]);
                    std::process::exit(1);
                }
            }
        } else if opt == "-q" || opt == "-v" {
            // Quiet/verbose — silently accepted for compatibility
        } else if opt == "--" {
            argv = &argv[1..];
            break;
        } else {
            eprintln!("head: unknown option: {opt}");
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

        let print_result = if *filename == "-" {
            let reader = BufReader::new(io::stdin().lock());
            let mut ok = true;
            for (i, line) in reader.lines().enumerate() {
                if i >= nlines { break; }
                match line {
                    Ok(l) => { let _ = writeln!(out, "{l}"); },
                    Err(e) => { eprintln!("head: stdin: {e}"); ok = false; break; },
                }
            }
            ok
        } else {
            match File::open(filename) {
                Ok(file) => {
                    let reader = BufReader::new(file);
                    let mut ok = true;
                    for (i, line) in reader.lines().enumerate() {
                        if i >= nlines { break; }
                        match line {
                            Ok(l) => { let _ = writeln!(out, "{l}"); },
                            Err(e) => { eprintln!("head: {filename}: {e}"); ok = false; break; },
                        }
                    }
                    ok
                }
                Err(e) => {
                    eprintln!("head: {filename}: {e}");
                    false
                }
            }
        };

        if !print_result {
            had_error = true;
        }
    }

    if had_error {
        std::process::exit(1);
    }
}
