//! Rust port of the MINIX/NetBSD `soelim` utility.
//!
//! Usage:
//!   soelim [file ...]
//!
//! Resolves .so requests in groff/nroff files, replacing
//! lines like '.so filename' with the contents of that file.

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let files: Vec<&str> = if args.len() < 2 {
        vec!["-"]
    } else {
        args[1..].iter().map(|s| s.as_str()).collect()
    };

    for fname in &files {
        process_file(fname, &mut std::collections::HashSet::new());
    }
}

fn process_file(fname: &str, seen: &mut std::collections::HashSet<String>) {
    let reader: Box<dyn BufRead> = if fname == "-" {
        Box::new(BufReader::new(io::stdin().lock()))
    } else {
        match File::open(fname) {
            Ok(f) => Box::new(BufReader::new(f)),
            Err(e) => { eprintln!("soelim: {fname}: {e}"); std::process::exit(1); }
        }
    };

    let dir = Path::new(fname).parent().map(|p| p.to_path_buf());

    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        if line.starts_with(".so ") {
            let include = line[4..].trim().to_string();
            if !seen.insert(include.clone()) {
                // Circular reference
                eprintln!("soelim: circular .so reference: {include}");
                std::process::exit(1);
            }
            let include_path = if include.starts_with('/') {
                include.clone()
            } else if let Some(ref d) = dir {
                d.join(&include).to_string_lossy().to_string()
            } else {
                include.clone()
            };
            process_file(&include_path, seen);
        } else {
            println!("{}", line);
        }
    }
}
