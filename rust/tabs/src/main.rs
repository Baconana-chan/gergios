//! Rust port of the MINIX/NetBSD `tabs` utility.
//!
//! Usage:
//!   tabs [tabstop ...]
//!
//! Sets terminal tab stops. With no arguments, resets to default 8-column stops.
//! On modern terminals, this outputs the appropriate escape sequences.

use std::io::Write;

fn usage() -> ! {
    eprintln!("usage: tabs [tabstop ...]");
    std::process::exit(1);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let argv = &args[1..];

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    if argv.is_empty() {
        // Reset to default 8-column tabs
        write!(out, "\x1b[3g").ok();    // clear all tabs
        write!(out, "\x1b[1;8;15;22;29;36;43;50;57;64;71;78;85;92;99;106;113;120H").ok();
    } else {
        // Set custom tab stops
        // First clear all tabs, then set each one
        write!(out, "\x1b[3g").ok();

        let mut stops = Vec::new();
        for arg in argv {
            let n: usize = arg.parse().unwrap_or_else(|_| usage());
            if n < 1 {
                usage();
            }
            stops.push(n);
        }

        // Build the ANSI sequence: ESC [ col ; col ; ... H
        let seq = stops.iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(";");
        write!(out, "\x1b[{seq}H").ok();
    }

    out.flush().ok();
}
