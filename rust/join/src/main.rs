//! Rust port of the MINIX/NetBSD `join` utility.
//!
//! Usage:
//!   join [-a file_number] [-e string] [-o list] [-t char] file1 file2
//!
//! Relational join of two sorted files on a common field.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, BufRead, BufReader};

const USAGE: &str = "usage: join [-a f] [-e s] [-o list] [-t c] file1 file2";

fn usage() -> ! {
    eprintln!("{USAGE}");
    std::process::exit(1);
}

/// Split a line into fields using the given delimiter (or whitespace)
fn split_fields(line: &str, delimiter: Option<char>) -> Vec<String> {
    if let Some(delim) = delimiter {
        line.split(delim).map(|s| s.to_string()).collect()
    } else {
        // Whitespace: skip leading blanks, treat runs as single separator
        line.split_whitespace().map(|s| s.to_string()).collect()
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut print_unpaired_1 = false;
    let mut print_unpaired_2 = false;
    let mut empty_str: Option<String> = None;
    let mut delimiter: Option<char> = None;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        let mut chars = opt.chars();
        chars.next(); // skip '-'
        let flag = chars.next().unwrap_or_else(|| usage());
        match flag {
            'a' => {
                let val = if opt.len() > 2 { opt[2..].to_string() } else {
                    argv = &argv[1..];
                    if argv.is_empty() { usage() }
                    argv[0].clone()
                };
                match val.as_str() {
                    "1" => { print_unpaired_1 = true; }
                    "2" => { print_unpaired_2 = true; }
                    _ => usage()
                }
            }
            'e' => {
                let val = if opt.len() > 2 { opt[2..].to_string() } else {
                    argv = &argv[1..];
                    if argv.is_empty() { usage() }
                    argv[0].clone()
                };
                empty_str = Some(val);
            }
            'o' => {
                // Parse but ignore output list for now (basic -o is a stub)
                let _val = if opt.len() > 2 { opt[2..].to_string() } else {
                    argv = &argv[1..];
                    if argv.is_empty() { usage() }
                    argv[0].clone()
                };
            }
            't' => {
                let val = if opt.len() > 2 { opt[2..].to_string() } else {
                    argv = &argv[1..];
                    if argv.is_empty() { usage() }
                    argv[0].clone()
                };
                delimiter = val.chars().next();
            }
            _ => usage(),
        }
        argv = &argv[1..];
    }

    if argv.len() != 2 {
        usage();
    }

    let file1_name = &argv[0];
    let file2_name = &argv[1];

    // Read file1 into memory: map join_key -> Vec<fields>
    let mut file1_data: HashMap<String, Vec<Vec<String>>> = HashMap::new();
    let mut file1_order: Vec<String> = Vec::new(); // to preserve order

    let reader1: Box<dyn BufRead> = if file1_name == "-" {
        Box::new(BufReader::new(io::stdin().lock()))
    } else {
        match File::open(file1_name) {
            Ok(f) => Box::new(BufReader::new(f)),
            Err(e) => { eprintln!("join: {file1_name}: {e}"); std::process::exit(1); }
        }
    };

    for line_res in reader1.lines() {
        let line = match line_res { Ok(l) => l, Err(_) => break };
        if line.is_empty() { continue; }
        let fields = split_fields(&line, delimiter);
        if fields.is_empty() { continue; }
        let key = fields[0].clone();
        if !file1_data.contains_key(&key) {
            file1_order.push(key.clone());
        }
        file1_data.entry(key).or_default().push(fields);
    }

    // Read file2 and output joins
    let reader2: Box<dyn BufRead> = if file2_name == "-" {
        Box::new(BufReader::new(io::stdin().lock()))
    } else {
        match File::open(file2_name) {
            Ok(f) => Box::new(BufReader::new(f)),
            Err(e) => { eprintln!("join: {file2_name}: {e}"); std::process::exit(1); }
        }
    };

    let mut file2_records: Vec<(String, Vec<String>)> = Vec::new();

    for line_res in reader2.lines() {
        let line = match line_res { Ok(l) => l, Err(_) => break };
        if line.is_empty() { continue; }
        let fields = split_fields(&line, delimiter);
        if fields.is_empty() { continue; }
        let key = fields[0].clone();
        // Join immediately for matched keys
        if let Some(records1) = file1_data.get(&key) {
            for r1 in records1 {
                let empty = empty_str.as_deref().unwrap_or("");
                let mut parts = Vec::new();
                parts.push(key.clone());
                for f in &r1[1..] {
                    parts.push(if f.is_empty() { empty.to_string() } else { f.clone() });
                }
                for f in &fields[1..] {
                    parts.push(if f.is_empty() { empty.to_string() } else { f.clone() });
                }
                let delim = delimiter.unwrap_or(' ');
                println!("{}", parts.join(&delim.to_string()));
            }
        } else if print_unpaired_2 {
            // Unpaired line from file2
            let empty = empty_str.as_deref().unwrap_or("");
            let mut parts = Vec::new();
            parts.push(key.clone());
            for f in &fields[1..] {
                parts.push(if f.is_empty() { empty.to_string() } else { f.clone() });
            }
            let delim = delimiter.unwrap_or(' ');
            println!("{}", parts.join(&delim.to_string()));
        }
        file2_records.push((key, fields));
    }

    // Handle unpaired lines from file1
    if print_unpaired_1 {
        let file2_keys: Vec<String> = file2_records.iter().map(|(k,_)| k.clone()).collect();
        for key in &file1_order {
            let in_file2 = file2_keys.iter().any(|k| k == key);
            if !in_file2 {
                if let Some(records1) = file1_data.get(key) {
                    for r1 in records1 {
                        let delim = delimiter.unwrap_or(' ');
                        println!("{}", r1.join(&delim.to_string()));
                    }
                }
            }
        }
    }
}
