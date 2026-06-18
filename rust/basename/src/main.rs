//! Rust port of the MINIX/NetBSD `basename` utility.
//!
//! Usage:
//!   basename string [suffix]
//!
//! Strips directory components and an optional suffix from a pathname.

use std::ffi::OsStr;
use std::path::Path;

const USAGE: &str = "usage: basename string [suffix]";

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // basename takes 1 or 2 arguments (plus program name)
    if args.len() < 2 || args.len() > 3 {
        eprintln!("{USAGE}");
        std::process::exit(1);
    }

    let path_str = &args[1];

    // If empty string, print newline and exit per POSIX
    if path_str.is_empty() {
        println!();
        std::process::exit(0);
    }

    // Extract the basename (last component of the path)
    let path = Path::new(path_str);
    let basename = path
        .file_name()
        .unwrap_or_else(|| OsStr::new(path_str));

    let result = if args.len() == 3 {
        let suffix = &args[2];
        let name = basename.to_str().unwrap_or("");
        if let Some(stripped) = name.strip_suffix(suffix) {
            // Only strip suffix if it's not the entire string
            if !stripped.is_empty() {
                stripped.to_string()
            } else {
                name.to_string()
            }
        } else {
            name.to_string()
        }
    } else {
        basename.to_str().unwrap_or("").to_string()
    };

    println!("{result}");
}
