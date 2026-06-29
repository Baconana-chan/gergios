//! Rust port of the MINIX/NetBSD `gencat` utility.
//!
//! Usage:
//!   gencat outputfile inputfile ...
//!
//! Generates message catalog files from source.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let argv = &args[1..];

    if argv.len() < 2 {
        eprintln!("usage: gencat outputfile inputfile ...");
        std::process::exit(1);
    }

    let output = &argv[0];
    let inputs = &argv[1..];
    let mut catalog: HashMap<u32, String> = HashMap::new();
    let mut set_num: u32 = 1;

    for fname in inputs {
        if !Path::new(fname).exists() {
            eprintln!("gencat: {fname}: no such file");
            continue;
        }

        let file = match File::open(fname) {
            Ok(f) => f,
            Err(e) => { eprintln!("gencat: {fname}: {e}"); continue; }
        };

        let reader = BufReader::new(file);
        let mut msg_num: u32 = 1;

        for line in reader.lines() {
            let line = match line { Ok(l) => l, Err(_) => break };
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('$') {
                if trimmed.starts_with("$set ") {
                    if let Some(n) = trimmed.trim_start_matches("$set ").trim().split_whitespace().next() {
                        set_num = n.parse().unwrap_or(set_num);
                    }
                }
                continue;
            }

            let key = set_num * 10000 + msg_num;
            // Remove quotes if present
            let msg = if (trimmed.starts_with('"') && trimmed.ends_with('"')) || 
                        (trimmed.starts_with('\'') && trimmed.ends_with('\'')) {
                trimmed[1..trimmed.len()-1].to_string()
            } else {
                trimmed.to_string()
            };
            catalog.insert(key, msg);
            msg_num += 1;
        }
    }

    // Write output as simple text catalog
    let mut out = Box::new(io::stdout().lock()) as Box<dyn Write>;
    if output != "-" {
        out = Box::new(File::create(output).unwrap_or_else(|_| {
            eprintln!("gencat: cannot create {output}");
            std::process::exit(1);
        }));
    }

    let mut keys: Vec<_> = catalog.keys().collect();
    keys.sort();
    for &key in &keys {
        if let Some(msg) = catalog.get(&key) {
            writeln!(out, "{} {}", key, msg).ok();
        }
    }
}
