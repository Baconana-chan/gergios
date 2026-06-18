//! Rust port of the MINIX/NetBSD `echo` utility.
//!
//! Usage:
//!   echo [-n] [string ...]
//!
//! Writes arguments to standard output separated by spaces.
//! If -n is specified, no trailing newline is printed.
//! NOTE: This utility intentionally does NOT use getopt-style parsing
//! to maintain POSIX compatibility with strings like "-n".

use std::io::{self, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv: &[String] = &args[1..];

    // Check for -n flag (manual parsing, NOT getopt)
    let nflag = argv.first().map_or(false, |s| s == "-n");
    if nflag {
        argv = &argv[1..];
    }

    let stdout = io::stdout();
    let mut handle = stdout.lock();

    for (i, arg) in argv.iter().enumerate() {
        if i > 0 {
            let _ = write!(handle, " ");
        }
        let _ = write!(handle, "{arg}");
    }

    if !nflag {
        let _ = writeln!(handle);
    }

    // POSIX requires exit 1 on write errors (fflush/ferror equivalent)
    if handle.flush().is_err() {
        std::process::exit(1);
    }
}
