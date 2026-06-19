//! Rust port of the MINIX/NetBSD `pathchk` utility.
//!
//! Usage:
//!   pathchk [-p] path ...
//!
//! Checks that pathnames are valid (portable) and not too long.

use std::io::{self, Write};
use std::path::Path;

const PATH_MAX: usize = 1024;
const NAME_MAX: usize = 255;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut portable = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        for c in argv[0].chars().skip(1) {
            match c {
                'p' => portable = true,
                _ => { eprintln!("pathchk: unknown option -- {c}"); std::process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    if argv.is_empty() {
        eprintln!("usage: pathchk [-p] path ...");
        std::process::exit(1);
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut had_error = false;

    for path_str in argv {
        let path = Path::new(path_str);

        // Check total length
        if path_str.len() > PATH_MAX {
            let _ = writeln!(out, "pathchk: `{path_str}': longer than {PATH_MAX} bytes");
            had_error = true;
        }

        // Check component lengths
        for component in path.components() {
            let name = component.as_os_str().to_string_lossy();
            if name.len() > NAME_MAX {
                let _ = writeln!(out, "pathchk: `{path_str}': component `{name}' longer than {NAME_MAX} bytes");
                had_error = true;
            }
            if portable && name.is_empty() {
                let _ = writeln!(out, "pathchk: `{path_str}': zero-length component");
                had_error = true;
            }
            if portable {
                for ch in name.chars() {
                    if !ch.is_ascii_alphanumeric() && ch != '.' && ch != '_' && ch != '-' && ch != '/' {
                        let _ = writeln!(out, "pathchk: `{path_str}': portable character `{ch}' not allowed");
                        had_error = true;
                    }
                }
            }
        }

        if !portable && path_str.contains('\0') {
            let _ = writeln!(out, "pathchk: `{path_str}': embedded null byte");
            had_error = true;
        }
    }

    if had_error { std::process::exit(1); }
}
