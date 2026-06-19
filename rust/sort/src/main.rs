//! Rust port of the MINIX/NetBSD `sort` utility.
//!
//! Usage:
//!   sort [-n] [-r] [-u] [-o output] [file ...]
//!
//! Sorts lines of text files.

use std::cmp::Ordering;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut numeric = false;
    let mut reverse = false;
    let mut unique = false;
    let mut output: Option<String> = None;
    let mut files: Vec<String> = Vec::new();

    while !argv.is_empty() {
        let arg = argv[0].clone();
        if arg == "--" {
            argv = &argv[1..];
            files.extend(argv.iter().cloned());
            break;
        }
        if arg.starts_with('-') && arg.len() > 1 {
            let mut chars = arg.chars();
            chars.next(); // skip '-'
            match chars.next() {
                Some('n') => numeric = true,
                Some('r') => reverse = true,
                Some('u') => unique = true,
                Some('o') => {
                    argv = &argv[1..];
                    if argv.is_empty() {
                        eprintln!("sort: option requires an argument: -o");
                        std::process::exit(1);
                    }
                    output = Some(argv[0].clone());
                }
                Some(c) => {
                    eprintln!("sort: unknown option -- {c}");
                    std::process::exit(1);
                }
                None => {
                    files.push(arg[1..].to_string()); // treat as filename
                }
            }
        } else {
            files.push(arg);
        }
        argv = &argv[1..];
    }

    if files.is_empty() {
        files.push("-".to_string());
    }

    // Read all lines
    let mut lines: Vec<String> = Vec::new();
    let mut had_error = false;

    for file in &files {
        let result: io::Result<Vec<String>> = if file == "-" {
            let stdin = io::stdin().lock();
            stdin.lines().collect()
        } else {
            let f = File::open(file)?;
            let reader = BufReader::new(f);
            reader.lines().collect()
        };

        match result {
            Ok(mut file_lines) => lines.append(&mut file_lines),
            Err(e) => {
                eprintln!("sort: {file}: {e}");
                had_error = true;
            }
        }
    }

    if had_error {
        std::process::exit(1);
    }

    // Sort
    lines.sort_by(|a, b| {
        let cmp = if numeric {
            let an: f64 = a.trim().parse().unwrap_or(0.0);
            let bn: f64 = b.trim().parse().unwrap_or(0.0);
            an.partial_cmp(&bn).unwrap_or(Ordering::Equal)
        } else {
            a.cmp(b)
        };
        if reverse { cmp.reverse() } else { cmp }
    });

    // Unique
    if unique {
        lines.dedup();
    }

    // Write output
    let write_result = if let Some(ref out_file) = output {
        let mut f = File::create(out_file)?;
        for line in &lines {
            writeln!(f, "{line}")?;
        }
        Ok(())
    } else {
        let stdout = io::stdout();
        let mut out = stdout.lock();
        for line in &lines {
            writeln!(out, "{line}")?;
        }
        Ok(())
    };

    if let Err(e) = write_result {
        eprintln!("sort: write error: {e}");
        std::process::exit(1);
    }
}
