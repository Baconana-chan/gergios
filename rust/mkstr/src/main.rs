//! Rust port of the MINIX/NetBSD `mkstr` utility.
//!
//! Usage:
//!   mkstr [-o output] [input ...]
//!
//! Creates error message files by processing C source files.
//! Replaces string literals with references to the message file.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut output = "messages".to_string();

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        if opt == "-o" {
            argv = &argv[1..];
            if argv.is_empty() { eprintln!("usage: mkstr [-o output] [input ...]"); std::process::exit(1); }
            output = argv[0].clone();
        } else {
            eprintln!("usage: mkstr [-o output] [input ...]");
            std::process::exit(1);
        }
        argv = &argv[1..];
    }

    let inputs: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let mut messages: Vec<String> = Vec::new();
    let mut msg_map: HashMap<String, usize> = HashMap::new();

    for fname in &inputs {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(std::io::stdin().lock()))
        } else {
            match fs::File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("mkstr: {fname}: {e}"); std::process::exit(1); }
            }
        };

        for line in reader.lines() {
            let line = match line { Ok(l) => l, Err(_) => break };
            let mut i = 0;
            let bytes = line.as_bytes();
            while i < bytes.len() {
                if bytes[i] == b'"' {
                    i += 1;
                    let start = i;
                    while i < bytes.len() && bytes[i] != b'"' {
                        if bytes[i] == b'\\' { i += 1; }
                        i += 1;
                    }
                    let msg = String::from_utf8_lossy(&bytes[start..i]).to_string();
                    if !msg.is_empty() && !msg_map.contains_key(&msg) {
                        msg_map.insert(msg.clone(), messages.len());
                        messages.push(msg);
                    }
                }
                i += 1;
            }
        }
    }

    // Write message file
    let mut msg_file = fs::File::create(&output)
        .unwrap_or_else(|_| { eprintln!("mkstr: cannot create {output}"); std::process::exit(1); });
    for (i, msg) in messages.iter().enumerate() {
        writeln!(msg_file, "{}\t{}", i, msg).ok();
    }
}
