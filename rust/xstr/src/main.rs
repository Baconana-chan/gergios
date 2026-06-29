//! Rust port of the MINIX/NetBSD `xstr` utility.
//!
//! Usage:
//!   xstr [-c] [-d] [file]
//!
//! Extracts strings from C source code to implement shared strings.
//! -c: extract strings from C source, write to strings file
//! -d: display strings from an object file

use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::process;

const USAGE: &str = "usage: xstr [-c] [-d] [file]";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut extract = false;
    let mut display = false;

    while !argv.is_empty() && argv[0].starts_with('-') {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                'c' => extract = true,
                'd' => display = true,
                _ => { eprintln!("{USAGE}"); process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    let file_name = if argv.is_empty() { None } else { Some(&argv[0][..]) };

    if display {
        // Display strings from an object file
        let reader: Box<dyn BufRead> = if let Some(fname) = file_name {
            Box::new(BufReader::new(fs::File::open(fname).unwrap_or_else(|e| {
                eprintln!("xstr: {fname}: {e}"); process::exit(1);
            })))
        } else {
            Box::new(BufReader::new(io::stdin().lock()))
        };

        for line in reader.lines() {
            if let Ok(l) = line {
                println!("{l}");
            }
        }
    } else if extract || file_name.is_some() {
        // Extract strings from C source
        let reader: Box<dyn BufRead> = if let Some(fname) = file_name {
            Box::new(BufReader::new(fs::File::open(fname).unwrap_or_else(|e| {
                eprintln!("xstr: {fname}: {e}"); process::exit(1);
            })))
        } else {
            Box::new(BufReader::new(io::stdin().lock()))
        };

        // Collect all string literals from C source
        let mut strings: Vec<String> = Vec::new();
        for line_res in reader.lines() {
            let line = match line_res { Ok(l) => l, Err(_) => break };
            // Find quoted strings
            let bytes = line.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'"' {
                    let mut s = String::new();
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'"' {
                        if bytes[i] == b'\\' && i + 1 < bytes.len() {
                            match bytes[i + 1] {
                                b'n' => s.push('\n'),
                                b't' => s.push('\t'),
                                b'r' => s.push('\r'),
                                b'\\' => s.push('\\'),
                                b'"' => s.push('"'),
                                b'0' => s.push('\0'),
                                _ => s.push(bytes[i + 1] as char),
                            }
                            i += 2;
                        } else {
                            s.push(bytes[i] as char);
                            i += 1;
                        }
                    }
                    if i < bytes.len() && bytes[i] == b'"' {
                        i += 1;
                    }
                    if !s.is_empty() {
                        strings.push(s);
                    }
                } else {
                    i += 1;
                }
            }
        }

        // Write to strings file
        let out_name = "strings";
        let mut out_file = fs::File::create(out_name).unwrap_or_else(|e| {
            eprintln!("xstr: cannot create {out_name}: {e}"); process::exit(1);
        });

        for s in &strings {
            writeln!(out_file, "{s}").ok();
        }

        // Also output the mesg directive lines for each string
        for s in &strings {
            println!("xstr::{}", s);
        }
    } else {
        eprintln!("{USAGE}");
        process::exit(1);
    }
}
