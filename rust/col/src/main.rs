//! Rust port of the MINIX/NetBSD `col` utility.
//!
//! Usage:
//!   col [-bfx] [-l nlines]
//!
//! Filters reverse line feeds (ESC-7) and half-line feeds (ESC-8, ESC-9)
//! from nroff/troff output, producing "normal" text.

use std::io::{self, Read, Write};
use std::process;

const USAGE: &str = "usage: col [-bfx] [-l nlines]";

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut backspace = false;
    let mut _fine = false;
    let mut pass_unknown = false;
    let mut _max_lines: usize = 128;

    while !argv.is_empty() && argv[0].starts_with('-') {
        let opt = argv[0].clone();
        if opt == "--" { break; }
        if opt.starts_with("-l") {
            let val = if opt.len() > 2 { opt[2..].to_string() } else {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("{USAGE}"); process::exit(1); }
                argv[0].clone()
            };
            _max_lines = val.parse().unwrap_or_else(|_| { eprintln!("{USAGE}"); process::exit(1); });
        } else {
            for ch in opt.chars().skip(1) {
                match ch {
                    'b' => backspace = true,
                    'f' => _fine = true,
                    'x' => pass_unknown = true,
                    _ => { eprintln!("{USAGE}"); process::exit(1); }
                }
            }
        }
        argv = &argv[1..];
    }

    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input).ok();

    let mut output = Vec::new();
    let mut i = 0;

    while i < input.len() {
        let b = input[i];
        if b == b'\x1b' {
            if i + 1 < input.len() {
                let esc_cmd = input[i + 1];
                match esc_cmd {
                    b'7' => {
                        if !backspace {
                            output.push(b'\n');
                        }
                        i += 2;
                        continue;
                    }
                    b'8' => {
                        i += 2;
                        continue;
                    }
                    b'9' => {
                        i += 2;
                        continue;
                    }
                    _ => {
                        if pass_unknown {
                            output.push(b);
                            output.push(esc_cmd);
                        }
                        i += 2;
                        continue;
                    }
                }
            }
            if pass_unknown {
                output.push(b);
            }
            i += 1;
        } else if b == 0x08 || b == b'\r' {
            i += 1;
        } else {
            output.push(b);
            i += 1;
        }
    }

    if backspace {
        io::stdout().write_all(&input).ok();
    } else {
        io::stdout().write_all(&output).ok();
    }
}
