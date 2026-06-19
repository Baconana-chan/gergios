//! Rust port of the MINIX/NetBSD `tty` utility.
//!
//! Usage:
//!   tty [-s]
//!
//! Prints the filename of the terminal connected to standard input.
//! -s: silent mode — only return exit code.

use std::io::{self, Write};
use std::ffi::CStr;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let silent = args.get(1).map_or(false, |s| s == "-s");

    unsafe {
        let ptr = libc::ttyname(libc::STDIN_FILENO);
        if ptr.is_null() {
            if !silent {
                eprintln!("tty: not a tty");
            }
            std::process::exit(1);
        }

        if !silent {
            let name = CStr::from_ptr(ptr).to_str().unwrap_or("");
            let stdout = io::stdout();
            let mut handle = stdout.lock();
            let _ = writeln!(handle, "{name}");
        }
    }
}
