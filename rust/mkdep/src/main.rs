//! Rust port of the NetBSD `mkdep` utility.
//!
//! Usage:
//!   mkdep [-ap] [-f depend_file] [-s suffixes] [-v variable] [cc_command] file ...
//!
//! Generates Makefile dependency lists from C source files.
//! Parses #include directives and outputs Makefile rules.

use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut dep_file = ".depend".to_string();
    let mut variable = String::new();
    let mut append = false;
    let mut preprocess = false;
    let mut suffixes: Vec<String> = vec![".c".to_string(), ".cc".to_string(), ".cpp".to_string(), ".cxx".to_string(), ".m".to_string(), ".mm".to_string(), ".h".to_string()];

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        match opt.as_str() {
            "-f" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: mkdep [-ap] [-f file] [-s suffixes] [-v var] [cc] file ..."); std::process::exit(1); }
                dep_file = argv[0].clone();
            }
            "-v" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: mkdep [-ap] [-f file] ..."); std::process::exit(1); }
                variable = argv[0].clone();
            }
            "-s" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: mkdep [-ap] [-f file] ..."); std::process::exit(1); }
                suffixes = argv[0].split_whitespace().map(|s| s.to_string()).collect();
            }
            "-a" => append = true,
            "-p" => preprocess = true,
            _ => { eprintln!("usage: mkdep [-ap] [-f file] [-s suffixes] [-v var] [cc] file ..."); std::process::exit(1); }
        }
        argv = &argv[1..];
    }

    // If there's a command before the files (like "cc -E"), skip it
    let files: Vec<&str> = if preprocess {
        // Skip cc command, take remaining as files
        let mut i = 0;
        while i < argv.len() && !argv[i].ends_with(".c") && !argv[i].ends_with(".cc")
            && !argv[i].ends_with(".cpp") && !argv[i].ends_with(".h") {
            i += 1;
        }
        argv[i..].iter().map(|s| s.as_str()).collect()
    } else {
        argv.iter().map(|s| s.as_str()).collect()
    };

    let mut deps: Vec<(String, BTreeSet<String>)> = Vec::new();

    for fname in &files {
        let ext = Path::new(fname).extension().and_then(|s| s.to_str()).unwrap_or("");
        if !suffixes.iter().any(|s| s.ends_with(ext)) { continue; }

        let file = match fs::File::open(fname) {
            Ok(f) => f,
            Err(_) => continue,
        };
        let reader = BufReader::new(file);
        let mut includes: BTreeSet<String> = BTreeSet::new();

        for line in reader.lines() {
            let line = match line { Ok(l) => l, Err(_) => break };
            let trimmed = line.trim();

            // #include "file.h" or #include <file.h>
            if trimmed.starts_with("#include ") {
                let rest = trimmed.trim_start_matches("#include ").trim();
                if rest.starts_with('"') {
                    if let Some(end) = rest[1..].find('"') {
                        includes.insert(rest[1..=end].to_string());
                    }
                }
            }
        }

        // Also detect #include_next, #import, etc.
        // Only include non-system headers (those in quotes)
        let obj = if ext == "c" || ext == "cc" || ext == "cpp" || ext == "cxx" {
            fname.replace(&format!(".{}", ext), ".o")
        } else {
            fname.to_string()
        };

        deps.push((obj, includes));
    }

    // Write dependency file
    let mut out: Box<dyn Write> = if dep_file == "-" {
        Box::new(std::io::stdout().lock())
    } else if append {
        Box::new(fs::OpenOptions::new().append(true).create(true).open(&dep_file)
            .unwrap_or_else(|_| { eprintln!("mkdep: cannot open {dep_file}"); std::process::exit(1); }))
    } else {
        Box::new(fs::File::create(&dep_file)
            .unwrap_or_else(|_| { eprintln!("mkdep: cannot create {dep_file}"); std::process::exit(1); }))
    };

    if !variable.is_empty() {
        write!(out, "{} =", variable).ok();
        for (_, includes) in &deps {
            for inc in includes {
                write!(out, " {}", inc).ok();
            }
        }
        writeln!(out).ok();
    } else {
        for (obj, includes) in &deps {
            write!(out, "{}:", obj).ok();
            for inc in includes {
                write!(out, " {}", inc).ok();
            }
            writeln!(out).ok();
        }
    }
}
