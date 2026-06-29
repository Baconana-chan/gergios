//! Rust port of the MINIX/NetBSD `colrm` utility.
//!
//! Usage:
//!   colrm [startcol [endcol]]
//!
//! Removes specified columns from each line of input.
//! Column numbering starts at 1.

use std::io::{self, BufRead, BufReader, Write};

fn usage() -> ! {
    eprintln!("usage: colrm [startcol [endcol]]");
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let argv = &args[1..];

    let (startcol, endcol) = match argv.len() {
        0 => (0usize, 0usize), // no columns removed
        1 => {
            let s: usize = argv[0].parse().unwrap_or_else(|_| usage());
            if s < 1 { usage(); }
            (s, s)
        }
        2 => {
            let s: usize = argv[0].parse().unwrap_or_else(|_| usage());
            let e: usize = argv[1].parse().unwrap_or_else(|_| usage());
            if s < 1 || (e > 0 && e < s) { usage(); }
            (s, e)
        }
        _ => usage(),
    };

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let stdin = io::stdin();
    let reader = BufReader::new(stdin.lock());

    for line_res in reader.lines() {
        let line = match line_res {
            Ok(l) => l,
            Err(_) => break,
        };

        if startcol == 0 {
            writeln!(out, "{line}").ok();
            continue;
        }

        // Convert to char vector for column-based operations
        let chars: Vec<char> = line.chars().collect();
        let len = chars.len();

        let result: String = if endcol == 0 {
            // Remove from startcol to end of line
            if startcol <= len {
                chars[..startcol - 1].iter().collect()
            } else {
                line.clone()
            }
        } else {
            // Remove from startcol to endcol
            if startcol > len {
                line.clone()
            } else if endcol >= len {
                chars[..startcol - 1].iter().collect()
            } else {
                let before: String = chars[..startcol - 1].iter().collect();
                let after: String = chars[endcol..].iter().collect();
                format!("{before}{after}")
            }
        };

        writeln!(out, "{result}").ok();
    }


}
