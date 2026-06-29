//! Rust port of the MINIX/NetBSD `sdiff` utility.
//!
//! Usage:
//!   sdiff [-l] [-s] [-w width] file1 file2
//!
//! Side-by-side diff of two files.

use std::fs;
use std::io::{self, BufRead, BufReader};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut width: usize = 130;
    let mut ignore_whitespace = false;
    let mut suppress_common = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                'l' => ignore_whitespace = true,
                's' => suppress_common = true,
                'w' => {
                    if opt.len() > 2 {
                        width = opt[2..].parse().unwrap_or_else(|_| {
                            eprintln!("usage: sdiff [-l] [-s] [-w width] file1 file2");
                            std::process::exit(1);
                        });
                    } else {
                        argv = &argv[1..];
                        if argv.is_empty() {
                            eprintln!("usage: sdiff [-l] [-s] [-w width] file1 file2");
                            std::process::exit(1);
                        }
                        width = argv[0].parse().unwrap_or_else(|_| {
                            eprintln!("sdiff: invalid width");
                            std::process::exit(1);
                        });
                    }
                }
                _ => {
                    eprintln!("usage: sdiff [-l] [-s] [-w width] file1 file2");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    if argv.len() < 2 {
        eprintln!("usage: sdiff [-l] [-s] [-w width] file1 file2");
        std::process::exit(1);
    }

    let file1 = argv[0].as_str();
    let file2 = argv[1].as_str();

    let lines1 = read_lines(file1);
    let lines2 = read_lines(file2);

    let col_width = width.saturating_sub(3) / 2;
    let max_len = lines1.len().max(lines2.len());

    for i in 0..max_len {
        let l1 = lines1.get(i).map(|s| s.as_str()).unwrap_or("");
        let l2 = lines2.get(i).map(|s| s.as_str()).unwrap_or("");

        let cmp1 = if ignore_whitespace { l1.trim() } else { l1 };
        let cmp2 = if ignore_whitespace { l2.trim() } else { l2 };
        let same = cmp1 == cmp2;

        if same && suppress_common {
            continue;
        }

        let sep = if same { "   " } else { " | " };
        let truncated1 = truncate(l1, col_width);
        let truncated2 = truncate(l2, col_width);

        println!("{}{}{}", truncated1, sep, truncated2);
    }
}

fn read_lines(path: &str) -> Vec<String> {
    let reader: Box<dyn BufRead> = if path == "-" {
        Box::new(BufReader::new(io::stdin().lock()))
    } else {
        match fs::File::open(path) {
            Ok(f) => Box::new(BufReader::new(f)),
            Err(e) => {
                eprintln!("sdiff: {path}: {e}");
                std::process::exit(1);
            }
        }
    };

    reader.lines().filter_map(|l| l.ok()).collect()
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        format!("{:width$}", s, width = max)
    } else {
        let end = max.saturating_sub(3);
        format!("{}...", &s[..end.min(s.len())])
    }
}
