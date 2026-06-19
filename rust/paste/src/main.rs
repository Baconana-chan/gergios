//! Rust port of the MINIX/NetBSD `paste` utility.
//!
//! Usage:
//!   paste [-s] [-d delimiters] file ...
//!
//! Merges lines of files side by side (tabular output).

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut serial = false;
    let mut delim = '\t';

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone(); argv = &argv[1..];
        match opt.as_str() {
            "-s" => serial = true,
            "-d" => {
                if argv.is_empty() { eprintln!("paste: -d requires argument"); std::process::exit(1); }
                delim = argv[0].chars().next().unwrap_or('\t');
                argv = &argv[1..];
            }
            _ => { eprintln!("paste: unknown option: {opt}"); std::process::exit(1); }
        }
    }

    let names: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdout = io::stdout();
    let mut out = stdout.lock();

    if serial {
        // -s: merge sequentially (parallel per file)
        for fname in &names {
            let reader: Box<dyn BufRead> = if *fname == "-" {
                Box::new(BufReader::new(io::stdin().lock()))
            } else {
                match File::open(fname) { Ok(f) => Box::new(BufReader::new(f)), Err(e) => { eprintln!("paste: {fname}: {e}"); continue; } }
            };
            let mut first = true;
            for line_res in reader.lines() {
                let line = match line_res { Ok(l) => l, Err(_) => break };
                if !first { let _ = write!(out, "{delim}"); }
                let _ = write!(out, "{line}");
                first = false;
            }
            let _ = writeln!(out);
        }
    } else {
        // Merge in parallel: one line from each file per output line
        let mut readers: Vec<Box<dyn BufRead>> = Vec::new();
        for fname in &names {
            let r: Box<dyn BufRead> = if *fname == "-" {
                Box::new(BufReader::new(io::stdin().lock()))
            } else {
                match File::open(fname) { Ok(f) => Box::new(BufReader::new(f)), Err(e) => { eprintln!("paste: {fname}: {e}"); continue; } }
            };
            readers.push(r);
        }

        loop {
            let mut had_output = false;
            let mut first_col = true;
            for reader in &mut readers {
                let mut buf = String::new();
                match reader.read_line(&mut buf) {
                    Ok(0) => {}
                    Ok(_) => {
                        if !first_col { let _ = write!(out, "{delim}"); }
                        let line = buf.trim_end_matches('\n').trim_end_matches('\r');
                        let _ = write!(out, "{line}");
                        first_col = false;
                        had_output = true;
                    }
                    Err(_) => break,
                }
            }
            if !had_output { break; }
            let _ = writeln!(out);
        }
    }
}
