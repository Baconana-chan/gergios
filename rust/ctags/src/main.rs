//! Rust port of the MINIX/NetBSD `ctags` utility.
//!
//! Usage:
//!   ctags [-a] [-f tagsfile] file ...
//!
//! Generates tag files for source code. Simplified implementation
//! supporting C, C++, and basic tag patterns.

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut tags_file = "tags".to_string();
    let mut append = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        match opt.as_str() {
            "-f" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: ctags [-a] [-f file] file ..."); std::process::exit(1); }
                tags_file = argv[0].clone();
            }
            "-a" => append = true,
            _ => { eprintln!("usage: ctags [-a] [-f file] file ..."); std::process::exit(1); }
        }
        argv = &argv[1..];
    }

    if argv.is_empty() {
        eprintln!("usage: ctags [-a] [-f file] file ...");
        std::process::exit(1);
    }

    let mut tags: HashMap<String, Vec<(String, u64)>> = HashMap::new();

    for fname in argv {
        let file = match fs::File::open(fname) {
            Ok(f) => f,
            Err(e) => { eprintln!("ctags: {fname}: {e}"); continue; }
        };
        let reader = BufReader::new(file);
        let mut line_num = 0u64;

        for line in reader.lines() {
            line_num += 1;
            let line = match line { Ok(l) => l, Err(_) => break };

            // C functions: type name(args) {
            if let Some(pos) = line.find('(') {
                let before = line[..pos].trim();
                // Check for function-like patterns
                if !before.is_empty() {
                    let parts: Vec<&str> = before.split_whitespace().collect();
                    if let Some(&name) = parts.last() {
                        if name.len() > 1 && !name.starts_with('"') && !name.starts_with('\'')
                            && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                        {
                            tags.entry(name.to_string())
                                .or_default()
                                .push((fname.to_string(), line_num));
                        }
                    }
                }
            }

            // C preprocessor #define
            if line.trim().starts_with("#define ") {
                let rest = line.trim().trim_start_matches("#define ");
                if let Some(name) = rest.split_whitespace().next() {
                    tags.entry(name.to_string())
                        .or_default()
                        .push((fname.to_string(), line_num));
                }
            }

            // struct/enum/union/class definitions
            for kw in &["struct ", "enum ", "union ", "class "] {
                if line.trim().starts_with(kw) {
                    let after = line.trim().trim_start_matches(kw);
                    if let Some(name) = after.split_whitespace().next() {
                        if name.starts_with('{') || name == "{" { continue; }
                        if name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                            tags.entry(name.to_string())
                                .or_default()
                                .push((fname.to_string(), line_num));
                        }
                    }
                }
            }

            // typedef
            if line.trim().starts_with("typedef ") {
                let after = line.trim().trim_start_matches("typedef ");
                // Look for the last identifier before ;
                if let Some(semi) = after.find(';') {
                    let def = after[..semi].trim();
                    let parts: Vec<&str> = def.split_whitespace().collect();
                    if let Some(&name) = parts.last() {
                        if name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                            tags.entry(name.to_string())
                                .or_default()
                                .push((fname.to_string(), line_num));
                        }
                    }
                }
            }
        }
    }

    let mut out_file: Box<dyn Write> = if append {
        Box::new(fs::OpenOptions::new().append(true).create(true).open(&tags_file)
            .unwrap_or_else(|_| { eprintln!("ctags: cannot open {tags_file}"); std::process::exit(1); }))
    } else {
        Box::new(fs::File::create(&tags_file)
            .unwrap_or_else(|_| { eprintln!("ctags: cannot create {tags_file}"); std::process::exit(1); }))
    };

    writeln!(out_file, "!_TAG_FILE_FORMAT\t2").ok();
    writeln!(out_file, "!_TAG_PROGRAM_NAME\tctags/GergiOS").ok();

    let mut sorted: Vec<_> = tags.keys().collect();
    sorted.sort();
    for name in sorted {
        if let Some(entries) = tags.get(name) {
            for (fname, line) in entries {
                writeln!(out_file, "{}\t{}\t{}", name, fname, line).ok();
            }
        }
    }
}
