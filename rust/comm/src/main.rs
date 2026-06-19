//! Rust port of the MINIX/NetBSD `comm` utility.
//!
//! Usage:
//!   comm [-123] file1 file2
//!
//! Compares two sorted files line by line. With no options, produces
//! three-column output: lines unique to file1, lines unique to file2,
//! and lines common to both.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut col1 = true;
    let mut col2 = true;
    let mut col3 = true;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        for c in argv[0].chars().skip(1) {
            match c {
                '1' => col1 = false,
                '2' => col2 = false,
                '3' => col3 = false,
                _ => {
                    eprintln!("comm: unknown option -- {c}");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    if argv.len() != 2 {
        eprintln!("usage: comm [-123] file1 file2");
        std::process::exit(1);
    }

    // Open files
    let lines1 = read_lines(&argv[0]);
    let lines2 = read_lines(&argv[1]);

    let stdout = io::stdout();
    let mut out = stdout.lock();

    let mut i = 0usize;
    let mut j = 0usize;

    while i < lines1.len() || j < lines2.len() {
        if i >= lines1.len() {
            if col2 { let _ = writeln!(out, "\t\t{}", lines2[j]); }
            j += 1;
        } else if j >= lines2.len() {
            if col1 { let _ = writeln!(out, "{}", lines1[i]); }
            i += 1;
        } else {
            match lines1[i].cmp(&lines2[j]) {
                std::cmp::Ordering::Less => {
                    if col1 { let _ = writeln!(out, "{}", lines1[i]); }
                    i += 1;
                }
                std::cmp::Ordering::Greater => {
                    if col2 { let _ = writeln!(out, "\t{}", lines2[j]); }
                    j += 1;
                }
                std::cmp::Ordering::Equal => {
                    if col3 { let _ = writeln!(out, "\t\t{}", lines1[i]); }
                    i += 1;
                    j += 1;
                }
            }
        }
    }
}

fn read_lines(path: &str) -> Vec<String> {
    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("comm: {path}: {e}");
            std::process::exit(1);
        }
    };
    BufReader::new(file).lines().filter_map(|l| l.ok()).collect()
}
