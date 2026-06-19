//! Rust port of the MINIX/NetBSD `pwd` utility.
//!
//! Usage:
//!   pwd
//!
//! Prints the current working directory to standard output.

use std::env;
use std::io::{self, Write};

fn main() {
    let cwd = match env::current_dir() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("pwd: {e}");
            std::process::exit(1);
        }
    };

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = writeln!(handle, "{}", cwd.display());
}
