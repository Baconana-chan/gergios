//! Rust port of the MINIX/NetBSD `vis` utility.
//!
//! Usage:
//!   vis [-cst] [file ...]
//!
//! Encodes non-printable characters in a visual format.
//! -c: encode spaces as `\\s` and tabs as `\\t`
//! -s: encode only non-printable characters (not space/tab)
//! -t: encode tabs as `\\t` (like -c but only for tabs)

use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};

const USAGE: &str = "usage: vis [-cst] [file ...]";

/// Encode a byte into visual representation
fn vis_byte(b: u8, flag_c: bool, flag_t: bool) -> String {
    match b {
        b'\n' => "\n".to_string(),
        b'\t' => {
            if flag_c || flag_t {
                "\\t".to_string()
            } else {
                "\t".to_string()
            }
        }
        b' ' => {
            if flag_c {
                "\\s".to_string()
            } else {
                " ".to_string()
            }
        }
        0x00..=0x08 | 0x0b..=0x1f | 0x7f => {
            // Non-printable: encode as \^C (control char) or \xXX
            let c = b ^ 0x40;
            if c.is_ascii_uppercase() || c == b'?' || c == b'@' || c == b'[' || c == b'\\' || c == b']' || c == b'^' || c == b'_' {
                format!("\\^{}", c as char)
            } else {
                format!("\\x{:02X}", b)
            }
        }
        0x20..=0x7e => (b as char).to_string(),
        0x80..=0xff => format!("\\M-{}", vis_byte(b & 0x7f, flag_c, flag_t)),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut flag_c = false;
    let mut flag_s = false;
    let mut flag_t = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                'c' => flag_c = true,
                's' => flag_s = true,
                't' => flag_t = true,
                _ => { eprintln!("{USAGE}"); std::process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    let files: Vec<&str> = if argv.is_empty() { vec!["-"] } else { argv.iter().map(|s| s.as_str()).collect() };
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut had_error = false;

    for fname in &files {
        let reader: Box<dyn BufRead> = if *fname == "-" {
            Box::new(BufReader::new(io::stdin().lock()))
        } else {
            match File::open(fname) {
                Ok(f) => Box::new(BufReader::new(f)),
                Err(e) => { eprintln!("vis: {fname}: {e}"); had_error = true; continue; }
            }
        };

        for line_res in reader.lines() {
            let line = match line_res { Ok(l) => l, Err(_) => break };
            let mut result = String::new();
            for b in line.bytes() {
                if flag_s && (b == b' ' || b == b'\t') {
                    result.push(b as char);
                } else {
                    result.push_str(&vis_byte(b, flag_c, flag_t));
                }
            }
            writeln!(out, "{result}").ok();
        }
    }

    if had_error { std::process::exit(1); }
}
