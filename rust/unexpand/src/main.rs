//! Rust port of the MINIX/NetBSD `unexpand` utility.
//!
//! Usage:
//!   unexpand [-a] [-t tabstop] [file ...]
//!
//! Converts spaces to tabs, reading from files or stdin.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut all = false;
    let mut tabstop = 8usize;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone(); argv = &argv[1..];
        match opt.as_str() {
            "-a" => all = true,
            _ if opt.starts_with("-t") => {
                let val = if opt.len() > 2 { opt[2..].to_string() } else {
                    if argv.is_empty() { eprintln!("unexpand: -t requires argument"); std::process::exit(1); }
                    let v = argv[0].clone(); argv = &argv[1..]; v
                };
                tabstop = val.parse().unwrap_or_else(|_| { eprintln!("unexpand: invalid tabstop"); std::process::exit(1); });
                all = true;
            }
            _ => { eprintln!("unexpand: unknown option: {opt}"); std::process::exit(1); }
        }
    }

    let files: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut had_error = false;

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(fname) { Ok(f) => Box::new(BufReader::new(f)), Err(e) => { eprintln!("unexpand: {fname}: {e}"); had_error = true; continue; } }
        };

        for line_res in reader.lines() {
            let line = match line_res { Ok(l) => l, Err(_) => break };
            let result = unexpand_line(&line, tabstop, all);
            let _ = writeln!(out, "{result}");
        }
    }

    if had_error { std::process::exit(1); }
}

fn unexpand_line(line: &str, tabstop: usize, all: bool) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let len = chars.len();

    while i < len {
        if chars[i] == ' ' {
            // Count consecutive spaces
            let start = i;
            while i < len && chars[i] == ' ' { i += 1; }
            let nspaces = i - start;

            if all {
                // Convert as many tabstops as possible
                let col = out.chars().count();
                let next_tab = (col / tabstop + 1) * tabstop;
                let needed = next_tab - col;
                if needed <= nspaces {
                    out.push('\t');
                    // Push remaining spaces after the tab
                    for _ in needed..nspaces { out.push(' '); }
                } else {
                    for _ in 0..nspaces { out.push(' '); }
                }
            } else {
                // Only convert leading spaces that fill tabstops
                // Check if we're at the start (before any non-space)
                let is_leading = out.is_empty();
                if is_leading {
                    let col = 0;
                    let next_tab = (col / tabstop + 1) * tabstop;
                    let needed = next_tab - col;
                    if needed <= nspaces {
                        out.push('\t');
                        for _ in needed..nspaces { out.push(' '); }
                    } else {
                        for _ in 0..nspaces { out.push(' '); }
                    }
                } else {
                    for _ in 0..nspaces { out.push(' '); }
                }
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}
