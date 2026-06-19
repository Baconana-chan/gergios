//! Rust port of the MINIX/NetBSD `uniq` utility.
//!
//! Usage:
//!   uniq [-cdu] [input_file [output_file]]
//!
//! Reports or omits repeated lines in sorted input.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut count = false;
    let mut repeated = false;  // -d: only show duplicates
    let mut unique = false;    // -u: only show unique lines

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        for c in argv[0].chars().skip(1) {
            match c {
                'c' => count = true,
                'd' => repeated = true,
                'u' => unique = true,
                _ => {
                    eprintln!("uniq: unknown option -- {c}");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    let input: Box<dyn BufRead> = if let Some(file) = argv.first() {
        match File::open(file) {
            Ok(f) => Box::new(BufReader::new(f)),
            Err(e) => {
                eprintln!("uniq: {file}: {e}");
                std::process::exit(1);
            }
        }
    } else {
        Box::new(BufReader::new(io::stdin().lock()))
    };

    let output: Box<dyn Write> = if argv.len() > 1 {
        match File::create(argv[1]) {
            Ok(f) => Box::new(f),
            Err(e) => {
                eprintln!("uniq: {}: {e}", argv[1]);
                std::process::exit(1);
            }
        }
    } else {
        Box::new(io::stdout().lock())
    };

    process(input, output, count, repeated, unique);
}

fn process(
    input: Box<dyn BufRead>,
    mut output: Box<dyn Write>,
    count_flag: bool,
    repeated: bool,
    unique: bool,
) {
    let mut lines = input.lines();
    let mut prev_line: Option<String> = None;
    let mut run_count: u64 = 0;

    // Helper to output a line with optional count
    let mut flush = |line: &str, cnt: u64| {
        if repeated && cnt == 1 { return; } // -d: skip unique lines
        if unique && cnt > 1 { return; }    // -u: skip repeated lines
        if count_flag {
            let _ = writeln!(output, "{:>7} {line}", cnt);
        } else {
            let _ = writeln!(output, "{line}");
        }
    };

    for line_result in lines {
        let line = match line_result {
            Ok(l) => l,
            Err(e) => {
                eprintln!("uniq: read error: {e}");
                return;
            }
        };

        match prev_line {
            Some(ref prev) if *prev == line => {
                run_count += 1;
            }
            Some(ref prev) => {
                flush(prev, run_count);
                prev_line = Some(line);
                run_count = 1;
            }
            None => {
                prev_line = Some(line);
                run_count = 1;
            }
        }
    }

    if let Some(ref last) = prev_line {
        flush(last, run_count);
    }
}
