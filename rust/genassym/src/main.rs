//! Rust port of the NetBSD `genassym` utility.
//!
//! Usage:
//!   genassym [-d] [-o output] [input ...]
//!
//! Generates assembly symbol definitions from C header files.
//! Processes ASSYM(name, expression) macros and outputs .equ directives.

use std::fs;
use std::io::{BufRead, BufReader, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut output: Option<String> = None;
    let mut debug = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        match opt.as_str() {
            "-o" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: genassym [-d] [-o output] [input]"); std::process::exit(1); }
                output = Some(argv[0].clone());
            }
            "-d" => debug = true,
            _ => { eprintln!("usage: genassym [-d] [-o output] [input]"); std::process::exit(1); }
        }
        argv = &argv[1..];
    }

    let inputs: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let mut symbols: Vec<(String, String)> = Vec::new(); // (name, value_or_offset)

    for fname in &inputs {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(std::io::stdin().lock()))
        } else {
            match fs::File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("genassym: {fname}: {e}"); std::process::exit(1); }
            }
        };

        for line in reader.lines() {
            let line = match line { Ok(l) => l, Err(_) => break };
            let trimmed = line.trim();

            // ASSYM(name, expression)
            if trimmed.starts_with("ASSYM(") || trimmed.starts_with("assym(") {
                let inner = trimmed.trim_start_matches(|c: char| c.is_alphabetic())
                    .trim_start_matches('(').trim_end_matches(')');
                if let Some(comma) = inner.find(',') {
                    let name = inner[..comma].trim().to_string();
                    let expr = inner[comma+1..].trim().to_string();
                    symbols.push((name, expr));
                }
            }

            // STRUCT(name, type) for struct offset calculations
            if trimmed.starts_with("STRUCT(") || trimmed.starts_with("struct(") {
                let inner = trimmed.trim_start_matches(|c: char| c.is_alphabetic())
                    .trim_start_matches('(');
                if let Some(paren) = inner.rfind(')') {
                    let args = inner[..paren].to_string();
                    let parts: Vec<&str> = args.split(',').collect();
                    if parts.len() >= 2 {
                        let name = parts[0].trim();
                        let _type_name = parts[1].trim();
                        // Emit a placeholder offset (actual computation requires C compilation)
                        symbols.push((format!("{}_SIZE", name), format!("sizeof({})", _type_name)));
                    }
                }
            }

            // MEMBER(name, type, field) for struct member offset
            if trimmed.starts_with("MEMBER(") || trimmed.starts_with("member(") {
                let inner = trimmed.trim_start_matches(|c: char| c.is_alphabetic())
                    .trim_start_matches('(');
                if let Some(paren) = inner.rfind(')') {
                    let args = inner[..paren].to_string();
                    let parts: Vec<&str> = args.split(',').collect();
                    if parts.len() >= 3 {
                        let name = parts[0].trim();
                        let _type_name = parts[1].trim();
                        let field = parts[2].trim();
                        symbols.push((format!("{}_OFFSET", name), format!("offsetof({}, {})", _type_name, field)));
                    }
                }
            }
        }
    }

    let mut out: Box<dyn Write> = match output {
        Some(ref f) if f != "-" => Box::new(fs::File::create(f)
            .unwrap_or_else(|_| { eprintln!("genassym: cannot create {f}"); std::process::exit(1); })),
        _ => Box::new(std::io::stdout().lock()),
    };

    for (name, expr) in &symbols {
        if debug {
            writeln!(out, "/* {name} = {expr} */").ok();
        }
        writeln!(out, "#define\t{name}\t{expr}").ok();
    }
}
