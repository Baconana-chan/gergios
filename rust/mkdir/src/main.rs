//! Rust port of the MINIX/NetBSD `mkdir` utility.
//!
//! Usage:
//!   mkdir [-p] [-m mode] directory ...
//!
//! Creates directories with optional parent creation and mode.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut parents = false;
    let mut mode: Option<u32> = None;

    // Parse options
    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = &argv[0];
        let mut chars = opt.chars();
        chars.next(); // skip '-'
        match chars.next() {
            Some('p') => parents = true,
            Some('m') => {
                argv = &argv[1..];
                if argv.is_empty() {
                    eprintln!("mkdir: option requires an argument: -m");
                    std::process::exit(1);
                }
                mode = match u32::from_str_radix(argv[0], 8) {
                    Ok(m) => Some(m),
                    Err(_) => {
                        eprintln!("mkdir: invalid mode: {}", argv[0]);
                        std::process::exit(1);
                    }
                };
            }
            Some(c) => {
                eprintln!("mkdir: unknown option -- {c}");
                std::process::exit(1);
            }
            None => break,
        }
        argv = &argv[1..];
    }

    if argv.is_empty() {
        eprintln!("usage: mkdir [-p] [-m mode] directory ...");
        std::process::exit(1);
    }

    let mut had_error = false;
    for dir in argv {
        let result = if parents {
            fs::create_dir_all(dir)
        } else {
            fs::create_dir(dir)
        };

        match result {
            Ok(()) => {
                // Set mode if specified (after creating)
                if let Some(m) = mode {
                    use std::os::unix::fs::PermissionsExt;
                    if let Err(e) = fs::set_permissions(dir, fs::Permissions::from_mode(m & 0o777)) {
                        eprintln!("mkdir: {}: {e}", dir);
                        had_error = true;
                    }
                }
            }
            Err(e) if parents && e.kind() == io::ErrorKind::AlreadyExists => {
                // -p: existing dir is not an error
            }
            Err(e) => {
                eprintln!("mkdir: {}: {e}", dir);
                had_error = true;
            }
        }
    }

    if had_error {
        std::process::exit(1);
    }
}
