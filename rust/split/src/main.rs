//! Rust port of the MINIX/NetBSD `split` utility.
//!
//! Usage:
//!   split [-l lines] [-a suffix_len] [file [prefix]]
//!
//! Splits a file into equal-sized segments.

use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut lines_per = 1000usize;
    let mut suffix_len = 2usize;
    let mut prefix = String::from("x");

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone(); argv = &argv[1..];
        if opt.starts_with("-l") {
            let val = if opt.len() > 2 { opt[2..].to_string() } else {
                if argv.is_empty() { eprintln!("split: -l requires argument"); std::process::exit(1); }
                let v = argv[0].clone(); argv = &argv[1..]; v
            };
            lines_per = val.parse().unwrap_or_else(|_| { eprintln!("split: invalid number"); std::process::exit(1); });
        } else if opt.starts_with("-a") {
            let val = if opt.len() > 2 { opt[2..].to_string() } else {
                if argv.is_empty() { eprintln!("split: -a requires argument"); std::process::exit(1); }
                let v = argv[0].clone(); argv = &argv[1..]; v
            };
            suffix_len = val.parse().unwrap_or_else(|_| { eprintln!("split: invalid number"); std::process::exit(1); });
        } else {
            eprintln!("split: unknown option: {opt}"); std::process::exit(1);
        }
    }

    let file = if argv.is_empty() { "-".to_string() } else { argv[0].clone() };
    if argv.len() > 1 { prefix = argv[1].clone(); }

    let reader: Box<dyn BufRead> = if file == "-" {
        Box::new(BufReader::new(io::stdin().lock()))
    } else {
        match File::open(&file) { Ok(f) => Box::new(BufReader::new(f)), Err(e) => { eprintln!("split: {file}: {e}"); std::process::exit(1); } }
    };

    let mut part = 0u64;
    let mut line_count = 0usize;
    let mut out_file: Option<File> = None;

    for line_res in reader.lines() {
        let line = match line_res { Ok(l) => l, Err(_) => break };

        if line_count == 0 {
            // Open new output file
            let suffix = make_suffix(part, suffix_len);
            let name = format!("{prefix}{suffix}");
            out_file = Some(File::create(&name).unwrap_or_else(|e| {
                eprintln!("split: {name}: {e}"); std::process::exit(1);
            }));
        }

        if let Some(ref mut f) = out_file {
            let _ = writeln!(f, "{line}");
        }

        line_count += 1;
        if line_count >= lines_per {
            line_count = 0;
            part += 1;
        }
    }
}

fn make_suffix(n: u64, len: usize) -> String {
    let chars: Vec<char> = "abcdefghijklmnopqrstuvwxyz".chars().collect();
    let base = chars.len() as u64;
    let mut result = String::new();
    let mut m = n;
    loop {
        result.push(chars[(m % base) as usize]);
        m /= base;
        if m == 0 { break; }
    }
    while result.len() < len { result.push(chars[0]); }
    result.chars().rev().collect()
}
