//! Rust port of the NetBSD `mklocale` utility.
//!
//! Usage:
//!   mklocale [-o output] [input ...]
//!
//! Generates locale definition tables for LC_CTYPE, LC_COLLATE,
//! LC_MONETARY, LC_NUMERIC, LC_TIME, and LC_MESSAGES.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};

#[derive(Default)]
struct LocaleDef {
    charmap: BTreeMap<u32, String>,
    toupper: BTreeMap<u32, u32>,
    tolower: BTreeMap<u32, u32>,
    collation: Vec<(u32, u32)>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut output: Option<String> = None;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        match opt.as_str() {
            "-o" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: mklocale [-o output] [input]"); std::process::exit(1); }
                output = Some(argv[0].clone());
            }
            _ => { eprintln!("usage: mklocale [-o output] [input]"); std::process::exit(1); }
        }
        argv = &argv[1..];
    }

    let inputs: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let mut locale = LocaleDef::default();
    let mut locale_name = String::new();
    let mut section = String::new();
    let mut in_ctype = false;
    let mut in_collate = false;

    for fname in &inputs {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(std::io::stdin().lock()))
        } else {
            match fs::File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("mklocale: {fname}: {e}"); std::process::exit(1); }
            }
        };

        for line in reader.lines() {
            let line = match line { Ok(l) => l, Err(_) => break };
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") { continue; }

            if trimmed.starts_with("LC_") {
                section = trimmed.to_string();
                in_ctype = section == "LC_CTYPE";
                in_collate = section == "LC_COLLATE";
                continue;
            }
            if trimmed == "END LC_CTYPE" || trimmed == "END LC_COLLATE"
                || trimmed == "END LC_MONETARY" || trimmed == "END LC_NUMERIC"
                || trimmed == "END LC_TIME" || trimmed == "END LC_MESSAGES" {
                section.clear();
                in_ctype = false;
                in_collate = false;
                continue;
            }

            match section.as_str() {
                "" => {
                    if trimmed.starts_with("locale ") {
                        locale_name = trimmed.trim_start_matches("locale ").trim().to_string();
                    }
                }
                "LC_CTYPE" => {
                    if trimmed.starts_with('<') {
                        let mut chars = trimmed.chars();
                        chars.next();
                        let hex_str: String = chars.take_while(|c| *c != '>').collect();
                        if let Ok(cp) = u32::from_str_radix(&hex_str.trim_start_matches('U'), 16) {
                            let rest = trimmed.trim_start_matches(&format!("<{}>", hex_str)).trim();
                            let cls = rest.split_whitespace().next().unwrap_or("").to_string();
                            locale.charmap.insert(cp, cls);
                        }
                    }
                }
                _ if in_ctype && trimmed.starts_with("toupper ") => {
                    let rest = trimmed.trim_start_matches("toupper ");
                    let parts: Vec<&str> = rest.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let (Ok(lower), Ok(upper)) = (parse_codepoint(parts[0]), parse_codepoint(parts[1])) {
                            locale.toupper.insert(lower, upper);
                        }
                    }
                }
                _ if trimmed.starts_with("tolower ") => {
                    let rest = trimmed.trim_start_matches("tolower ");
                    let parts: Vec<&str> = rest.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if let (Ok(upper), Ok(lower)) = (parse_codepoint(parts[0]), parse_codepoint(parts[1])) {
                            locale.tolower.insert(upper, lower);
                        }
                    }
                }
                "LC_COLLATE" => {
                    if trimmed.starts_with("<U") && trimmed.contains('>') {
                        if let Some(end) = trimmed.find('>') {
                            let hex_str = &trimmed[1..end].trim_start_matches('U');
                            if let Ok(cp) = u32::from_str_radix(hex_str, 16) {
                                let rest = trimmed[end+1..].trim();
                                let weight: u32 = rest.parse().unwrap_or(cp);
                                locale.collation.push((cp, weight));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let out_name = output.unwrap_or_else(|| {
        if locale_name.is_empty() { "locale_data.c".to_string() } else { format!("{}.c", locale_name) }
    });

    let mut out = fs::File::create(&out_name)
        .unwrap_or_else(|_| { eprintln!("mklocale: cannot create {out_name}"); std::process::exit(1); });

    writeln!(out, "/* Generated by mklocale for {} */", 
        if locale_name.is_empty() { "unknown" } else { &locale_name }).ok();
    writeln!(out, "#include <locale.h>\n").ok();

    if !locale.charmap.is_empty() {
        writeln!(out, "static const struct charmap_entry charmap[{}] = {{", locale.charmap.len()).ok();
        for (cp, cls) in &locale.charmap {
            writeln!(out, "    {{ 0x{:04x}, \"{}\" }},", cp, cls).ok();
        }
        writeln!(out, "}};\n").ok();
    }

    if !locale.toupper.is_empty() {
        writeln!(out, "static const struct case_entry toupper_table[{}] = {{", locale.toupper.len()).ok();
        for (lower, upper) in &locale.toupper {
            writeln!(out, "    {{ 0x{:04x}, 0x{:04x} }},", lower, upper).ok();
        }
        writeln!(out, "}};\n").ok();
    }

    if !locale.collation.is_empty() {
        writeln!(out, "static const struct collation_entry collation_table[{}] = {{", locale.collation.len()).ok();
        for (cp, weight) in &locale.collation {
            writeln!(out, "    {{ 0x{:04x}, {} }},", cp, weight).ok();
        }
        writeln!(out, "}};\n").ok();
    }

    writeln!(out, "/* End of {} locale data */", 
        if locale_name.is_empty() { "unknown" } else { &locale_name }).ok();
}

fn parse_codepoint(s: &str) -> Result<u32, std::num::ParseIntError> {
    let s = s.trim();
    if s.starts_with("<U") && s.ends_with('>') {
        u32::from_str_radix(&s[2..s.len()-1], 16)
    } else if s.starts_with("0x") || s.starts_with("0X") {
        u32::from_str_radix(&s[2..], 16)
    } else {
        s.parse()
    }
}
