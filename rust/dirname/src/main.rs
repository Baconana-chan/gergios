//! Rust port of the MINIX/NetBSD `dirname` utility.
//!
//! Usage:
//!   dirname path
//!
//! Returns the directory portion of a pathname (everything up to the last '/').

use std::path::Path;

const USAGE: &str = "usage: dirname path";

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        eprintln!("{USAGE}");
        std::process::exit(1);
    }

    let path_str = &args[1];

    // Special case: empty string returns "."
    if path_str.is_empty() {
        println!(".");
        std::process::exit(0);
    }

    // Use Path::parent() to get the directory portion
    let path = Path::new(path_str);
    let dir = path.parent().unwrap_or_else(|| Path::new("."));

    let result = if dir.as_os_str().is_empty() {
        // Root path "/" → parent is empty → return "/" per POSIX
        // For relative paths with no parent, return "."
        if path_str.starts_with('/') {
            "/"
        } else {
            "."
        }
    } else {
        dir.to_str().unwrap_or(".")
    };

    println!("{result}");
}
