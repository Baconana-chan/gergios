//! Rust port of the MINIX/NetBSD `from` utility.
//!
//! Usage:
//!   from [-s sender] [-f file]
//!
//! Shows who sent mail (reads mbox format).

use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut sender_filter: Option<String> = None;
    let mut mail_file: Option<String> = None;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        match opt.as_str() {
            "-f" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: from [-s sender] [-f file]"); std::process::exit(1); }
                mail_file = Some(argv[0].clone());
            }
            "-s" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: from [-s sender] [-f file]"); std::process::exit(1); }
                sender_filter = Some(argv[0].clone().to_lowercase());
            }
            _ => { eprintln!("usage: from [-s sender] [-f file]"); std::process::exit(1); }
        }
        argv = &argv[1..];
    }

    let path = mail_file.unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        format!("{}/mbox", home)
    });

    if !Path::new(&path).exists() {
        return;
    }

    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(_) => return,
    };

    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = match line { Ok(l) => l, Err(_) => break };
        // Parse "From sender@host date" (mbox format)
        if let Some(rest) = line.strip_prefix("From ") {
            let parts: Vec<&str> = rest.splitn(2, ' ').collect();
            if !parts.is_empty() {
                let sender = parts[0];
                let date = if parts.len() > 1 { parts[1] } else { "" };

                if let Some(ref filter) = sender_filter {
                    if !sender.to_lowercase().contains(filter) {
                        continue;
                    }
                }

                let sender_name = sender.split('@').next().unwrap_or(sender);
                println!("{} {}", date, sender_name);
            }
        }
    }
}
