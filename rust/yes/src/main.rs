//! Rust port of the MINIX/NetBSD `yes` utility.
//!
//! Usage:
//!   yes [string]
//!
//! Outputs "y" (or the given string) repeatedly until killed (SIGPIPE/SIGINT).

use std::io::{self, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let yes: &str = args.get(1).map_or("y", |s| s.as_str());

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    // Loop forever (or until broken pipe)
    loop {
        if writeln!(handle, "{yes}").is_err() {
            // Broken pipe / write error — POSIX yes exits with failure
            std::process::exit(1);
        }
    }
}
