//! Rust port of the MINIX/NetBSD `printenv` utility.
//!
//! Usage:
//!   printenv [name ...]
//!
//! Prints all or part of the environment.
//! If no arguments, prints all environment variables.
//! If arguments given, prints the value of each named variable.

use std::env;
use std::io::{self, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() == 1 {
        // Print all environment variables
        let stdout = io::stdout();
        let mut handle = stdout.lock();
        for (key, value) in env::vars() {
            let _ = writeln!(handle, "{key}={value}");
        }
    } else {
        // Print specific variables
        let mut found = false;
        for name in &args[1..] {
            match env::var(name) {
                Ok(val) => {
                    println!("{val}");
                    found = true;
                }
                Err(_) => {
                    // Not found — not an error per POSIX, just skip
                }
            }
        }
        if !found {
            std::process::exit(1);
        }
    }
}
