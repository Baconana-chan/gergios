//! Rust port of the MINIX/NetBSD `nl` utility.
//!
//! Usage:
//!   nl [-ba] [-n fmt] [-w width] [-s sep] [file ...]
//!
//! Numbers lines of files. With no file, reads stdin.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut body_num = true;  // -b a: number all lines, -b t: only non-empty
    let mut num_fmt = 'n';    // -n ln|rn|rz: left, right, right-zero
    let mut width = 6usize;
    let mut sep = '\t';

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone(); argv = &argv[1..];
        match opt.as_str() {
            "-b" => {
                if argv.is_empty() { eprintln!("nl: -b requires argument"); std::process::exit(1); }
                let bval = argv[0].clone(); argv = &argv[1..];
                body_num = bval == "a";
            }
            "-n" => {
                if argv.is_empty() { eprintln!("nl: -n requires argument"); std::process::exit(1); }
                let f = argv[0].clone(); argv = &argv[1..];
                num_fmt = match f.as_str() { "ln" => 'l', "rn" => 'r', "rz" => 'z', _ => { eprintln!("nl: invalid format: {f}"); std::process::exit(1); } };
            }
            "-w" => {
                if argv.is_empty() { eprintln!("nl: -w requires argument"); std::process::exit(1); }
                width = argv[0].parse().unwrap_or_else(|_| { eprintln!("nl: invalid width"); std::process::exit(1); });
                argv = &argv[1..];
            }
            "-s" => {
                if argv.is_empty() { eprintln!("nl: -s requires argument"); std::process::exit(1); }
                sep = argv[0].chars().next().unwrap_or('\t');
                argv = &argv[1..];
            }
            o if o.starts_with("-ba") => body_num = true,
            o if o.starts_with("-bt") => body_num = false,
            _ => { eprintln!("nl: unknown option: {opt}"); std::process::exit(1); }
        }
    }

    let files: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut lineno = 0u64;

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(fname) { Ok(f) => Box::new(BufReader::new(f)), Err(e) => { eprintln!("nl: {fname}: {e}"); continue; } }
        };
        for line_res in reader.lines() {
            let line = match line_res { Ok(l) => l, Err(_) => break };
            if body_num || !line.trim().is_empty() {
                lineno += 1;
                let num_str = match num_fmt {
                    'l' => format!("{lineno}"),
                    'z' => format!("{lineno:0>w$}", w = width),
                    _ => format!("{lineno:>w$}", w = width),
                };
                let _ = writeln!(out, "{num_str}{sep}{line}");
            } else {
                let _ = writeln!(out, "{line}");
            }
        }
    }
}
