//! Rust port of the MINIX/NetBSD `shuffle` utility.
//!
//! Usage:
//!   shuffle [file ...]
//!
//! Randomly permutes lines of input.

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let files: Vec<&str> = if args.len() < 2 {
        vec!["-"]
    } else {
        args[1..].iter().map(|s| s.as_str()).collect()
    };

    let mut lines: Vec<String> = Vec::new();

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("shuffle: {fname}: {e}"); std::process::exit(1); }
            }
        };

        for line in reader.lines() {
            if let Ok(l) = line {
                lines.push(l);
            }
        }
    }

    // Fisher-Yates shuffle
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut state = seed as u32;

    let n = lines.len();
    for i in (1..n).rev() {
        state = state.wrapping_mul(1103515245).wrapping_add(12345);
        let j = (state >> 16) as usize % (i + 1);
        lines.swap(i, j);
    }

    for line in &lines {
        println!("{}", line);
    }
}
