//! Rust port of the MINIX/NetBSD `csplit` utility.
//!
//! Usage:
//!   csplit [-ks] [-f prefix] [-n number] file arg ...
//!
//! Splits a file into sections based on context.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

const USAGE: &str = "usage: csplit [-ks] [-f prefix] [-n digits] file arg ...";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut keep = false;
    let mut quiet = false;
    let mut prefix = "xx".to_string();
    let mut digits = 2usize;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }

        if opt == "-f" {
            argv = &argv[1..];
            if argv.is_empty() { eprintln!("{USAGE}"); std::process::exit(1); }
            prefix = argv[0].clone();
        } else if opt == "-n" {
            argv = &argv[1..];
            if argv.is_empty() { eprintln!("{USAGE}"); std::process::exit(1); }
            digits = argv[0].parse().unwrap_or_else(|_| { eprintln!("{USAGE}"); std::process::exit(1); });
        } else {
            for ch in opt.chars().skip(1) {
                match ch {
                    'k' => keep = true,
                    's' => quiet = true,
                    _ => { eprintln!("{USAGE}"); std::process::exit(1); }
                }
            }
        }
        argv = &argv[1..];
    }

    if argv.len() < 2 {
        eprintln!("{USAGE}");
        std::process::exit(1);
    }

    let file_name = argv[0].clone();
    let argv_remaining = &argv[1..];

    let reader: Box<dyn BufRead> = if file_name == "-" {
        Box::new(BufReader::new(io::stdin().lock()))
    } else {
        match File::open(&file_name) {
            Ok(f) => Box::new(BufReader::new(f)),
            Err(e) => { eprintln!("csplit: {file_name}: {e}"); std::process::exit(1); }
        }
    };

    let lines: Vec<String> = match reader.lines().collect() {
        Ok(l) => l,
        Err(_) => { eprintln!("csplit: read error"); std::process::exit(1); }
    };

    // Parse patterns and find split points
    let mut splits: Vec<usize> = Vec::new();

    let mut arg_idx = 0;
    while arg_idx < argv_remaining.len() {
        let arg = &argv_remaining[arg_idx];
        if arg.starts_with('/') || arg.starts_with('%') {
            let delim = arg.chars().next().unwrap();
            let rest = &arg[1..];
            let mut pattern = rest.to_string();
            let mut repeat = 0usize;
            let mut pat_idx = 0;
            while pat_idx < rest.len() {
                if rest.as_bytes()[pat_idx] == delim as u8 {
                    pattern = rest[..pat_idx].to_string();
                    let remaining = &rest[pat_idx + 1..];
                    if remaining.starts_with('{') {
                        let brace_close = remaining.find('}').unwrap_or(0);
                        let num_str = &remaining[1..brace_close];
                        repeat = if num_str == "*" { usize::MAX } else { num_str.parse().unwrap_or(1) };
                        if repeat == 0 { repeat = 1; }
                    }
                    break;
                }
                pat_idx += 1;
            }

            for _ in 0..repeat {
                let mut found = false;
                for (i, line) in lines.iter().enumerate() {
                    if i <= *splits.last().unwrap_or(&0) { continue; }
                    if line.contains(&pattern) {
                        splits.push(i);
                        found = true;
                        break;
                    }
                }
                if !found { break; }
            }
        } else if let Ok(n) = arg.parse::<usize>() {
            // Line number
            if n <= lines.len() && n > 0 {
                splits.push(n - 1);
            }
        } else if let Some(num_str) = arg.strip_prefix('{') {
            let close = num_str.find('}').unwrap_or(0);
            let _rep: usize = num_str[..close].parse().unwrap_or(1);
            continue;
        }
        arg_idx += 1;
    }

    splits.sort();
    splits.dedup();

    if splits.first() == Some(&0) {
        splits.remove(0);
    }

    // Write output files
    let mut file_num = 0usize;
    let mut last_split = 0usize;

    for &split in &splits {
        if split >= lines.len() { break; }
        if split <= last_split { continue; }

        let name = format!("{}{:0width$}", prefix, file_num, width = digits);
        let mut out_file = match File::create(&name) {
            Ok(f) => f,
            Err(e) => {
                if keep {
                    eprintln!("csplit: {name}: {e}");
                    continue;
                } else {
                    eprintln!("csplit: {name}: {e}");
                    std::process::exit(1);
                }
            }
        };

        for i in last_split..split {
            if i < lines.len() {
                writeln!(out_file, "{}", lines[i]).ok();
            }
        }

        if !quiet {
            println!("{name}");
        }

        file_num += 1;
        last_split = split;
    }

    // Last chunk
    if last_split < lines.len() {
        let name = format!("{}{:0width$}", prefix, file_num, width = digits);
        let mut out_file = match File::create(&name) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("csplit: {name}: {e}");
                if keep { return; }
                std::process::exit(1);
            }
        };

        for i in last_split..lines.len() {
            writeln!(out_file, "{}", lines[i]).ok();
        }

        if !quiet {
            println!("{name}");
        }
    }
}
