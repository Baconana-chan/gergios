//! Rust port of the MINIX/NetBSD `fpr` utility.
//!
//! Usage:
//!   fpr [file ...]
//!
//! Formats phone records from input.

use std::fs::File;
use std::io::{self, BufRead, BufReader};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let files: Vec<&str> = if args.len() < 2 { vec!["-"] } else { args[1..].iter().map(|s| s.as_str()).collect() };

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("fpr: {fname}: {e}"); std::process::exit(1); }
            }
        };

        for line in reader.lines() {
            let line = match line { Ok(l) => l, Err(_) => break };
            let formatted = format_phone(&line);
            println!("{}", formatted);
        }
    }
}

fn format_phone(s: &str) -> String {
    let digits: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.len() {
        0 => s.to_string(),
        7 => format!("{}-{}", &digits[..3], &digits[3..]),
        10 => format!("({}) {}-{}", &digits[..3], &digits[3..6], &digits[6..]),
        11 if digits.starts_with('1') => format!("1-({}) {}-{}", &digits[1..4], &digits[4..7], &digits[7..]),
        _ => {
            // Extension or international
            if digits.len() > 10 {
                let main = &digits[..10];
                let ext = &digits[10..];
                format!("({}) {}-{} x{}", &main[..3], &main[3..6], &main[6..], ext)
            } else {
                s.to_string()
            }
        }
    }
}
