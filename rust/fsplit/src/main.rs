//! Rust port of the MINIX/NetBSD `fsplit` utility.
//!
//! Usage:
//!   fsplit [file ...]
//!
//! Splits fortune files (separated by %) into individual fortunes.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let files: Vec<&str> = if args.len() < 2 { vec!["-"] } else { args[1..].iter().map(|s| s.as_str()).collect() };

    let mut fortune_num = 0;

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("fsplit: {fname}: {e}"); std::process::exit(1); }
            }
        };

        let mut fortune_lines: Vec<String> = Vec::new();

        for line in reader.lines() {
            let line = match line { Ok(l) => l, Err(_) => break };
            if line.trim() == "%" {
                if !fortune_lines.is_empty() {
                    fortune_num += 1;
                    write_fortune(fortune_num, &fortune_lines);
                    fortune_lines.clear();
                }
            } else {
                fortune_lines.push(line);
            }
        }

        // Last fortune
        if !fortune_lines.is_empty() {
            fortune_num += 1;
            write_fortune(fortune_num, &fortune_lines);
        }
    }
}

fn write_fortune(num: usize, lines: &[String]) {
    let name = format!("fortune-{:04}", num);
    let mut file = match File::create(&name) {
        Ok(f) => f,
        Err(_) => return,
    };
    for line in lines {
        let _ = writeln!(file, "{}", line);
    }
}
