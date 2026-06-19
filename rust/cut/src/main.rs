//! Rust port of the MINIX/NetBSD `cut` utility.
//!
//! Usage:
//!   cut -b list [-n] [file ...]
//!   cut -c list [file ...]
//!   cut -f list [-d delim] [-s] [file ...]
//!
//! Cuts out selected portions of each line from files.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut mode: u8 = 0; // 0=none, 1=bytes, 2=chars, 3=fields
    let mut list_str = String::new();
    let mut delim = '\t';
    let mut only_delimited = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        argv = &argv[1..];
        match opt.as_str() {
            "-b" | "-c" | "-f" => {
                mode = if opt == "-b" { 1 } else if opt == "-c" { 2 } else { 3 };
                if argv.is_empty() {
                    eprintln!("cut: option requires an argument: {opt}");
                    std::process::exit(1);
                }
                list_str = argv[0].clone();
                argv = &argv[1..];
            }
            "-d" => {
                if argv.is_empty() {
                    eprintln!("cut: option requires an argument: -d");
                    std::process::exit(1);
                }
                delim = argv[0].chars().next().unwrap_or('\t');
                argv = &argv[1..];
            }
            "-s" => only_delimited = true,
            _ => {
                // Could be combined like -f1,2
                let mut chars = opt.chars();
                chars.next(); // skip '-'
                match chars.next() {
                    Some('b') => { mode = 1; list_str = chars.collect(); }
                    Some('c') => { mode = 2; list_str = chars.collect(); }
                    Some('f') => { mode = 3; list_str = chars.collect(); }
                    Some(_) | None => {
                        eprintln!("cut: unknown option: {opt}");
                        std::process::exit(1);
                    }
                }
            }
        }
    }

    if mode == 0 || list_str.is_empty() {
        eprintln!("usage: cut -b|-c|-f list [file ...]");
        std::process::exit(1);
    }

    if mode == 3 && only_delimited { /* -s flag */ }

    let ranges = parse_ranges(&list_str).unwrap_or_else(|| {
        eprintln!("cut: invalid list: {list_str}");
        std::process::exit(1);
    });

    let files: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut had_error = false;

    for filename in &files {
        let lines: Box<dyn BufRead> = if *filename == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(filename) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("cut: {filename}: {e}"); had_error = true; continue; }
            }
        };

        for line_res in lines.lines() {
            let line = match line_res { Ok(l) => l, Err(_) => break };

            if mode == 3 {
                let fields: Vec<&str> = line.split(delim).collect();
                if only_delimited && fields.len() == 1 { continue; }
                let selected: Vec<&str> = select_ranges(&fields, &ranges);
                let _ = writeln!(out, "{}", selected.join(&delim.to_string()));
            } else {
                let chars: Vec<char> = line.chars().collect();
                let selected: String = select_ranges(&chars, &ranges).iter().collect();
                let _ = writeln!(out, "{selected}");
            }
        }
    }

    if had_error { std::process::exit(1); }
}

type Range = (usize, Option<usize>); // (start, end) where end is inclusive, None = to end

fn parse_ranges(s: &str) -> Option<Vec<Range>> {
    let mut ranges = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() { return None; }
        if let Some(dash) = part.find('-') {
            let left = &part[..dash];
            let right = &part[dash + 1..];
            let start = if left.is_empty() { 1 } else { left.parse().ok()? };
            let end = if right.is_empty() { None } else { Some(right.parse::<usize>().ok()?) };
            ranges.push((start, end));
        } else {
            let n: usize = part.parse().ok()?;
            ranges.push((n, Some(n)));
        }
    }
    Some(ranges)
}

fn select_ranges<T: Clone>(items: &[T], ranges: &[Range]) -> Vec<T> {
    let mut result = Vec::new();
    for &(start, end) in ranges {
        let i = start.saturating_sub(1);
        let j = match end { Some(e) => e, None => items.len() };
        for k in i..j.min(items.len()) {
            result.push(items[k].clone());
        }
    }
    result
}
