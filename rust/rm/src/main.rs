//! Rust port of the MINIX/NetBSD `rm` utility.
//!
//! Usage:
//!   rm [-f] [-i] [-Rr] file ...
//!
//! Removes files. -r or -R removes directories recursively.

use std::fs;
use std::io;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut force = false;
    let mut interactive = false;
    let mut recursive = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        for c in argv[0].chars().skip(1) {
            match c {
                'f' => force = true,
                'i' => interactive = true,
                'r' | 'R' => recursive = true,
                '-' => { argv = &argv[1..]; break; }
                _ => {
                    eprintln!("rm: unknown option -- {c}");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    if argv.is_empty() {
        if force {
            std::process::exit(0); // rm -f with no args = success
        }
        eprintln!("usage: rm [-f] [-i] [-Rr] file ...");
        std::process::exit(1);
    }

    let mut had_error = false;
    for file in argv {
        let result = remove_path(file, recursive, force, interactive);
        match result {
            Ok(()) => {},
            Err(e) => {
                if !force || e.kind() != io::ErrorKind::NotFound {
                    eprintln!("rm: {file}: {e}");
                    had_error = true;
                }
                // -f: ignore non-existent files
            }
        }
    }

    if had_error {
        std::process::exit(1);
    }
}

fn remove_path(path: &str, recursive: bool, force: bool, interactive: bool) -> io::Result<()> {
    // Interactive check
    if interactive {
        // Don't prompt if -f is also given (POSIX: -f overrides -i)
        if !force {
            eprint!("rm: remove `{path}'? ");
            use std::io::Write;
            std::io::stderr().flush()?;
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if !input.trim().eq_ignore_ascii_case("y") {
                return Ok(());
            }
        }
    }

    // Check if it's a directory
    match fs::metadata(path) {
        Ok(meta) => {
            if meta.is_dir() {
                if !recursive {
                    return Err(io::Error::new(io::ErrorKind::PermissionDenied,
                        "is a directory (use -r to remove)"));
                }
                fs::remove_dir_all(path)?;
            } else {
                fs::remove_file(path)?;
            }
        }
        Err(e) => {
            if force && e.kind() == io::ErrorKind::NotFound {
                return Ok(()); // -f: ignore missing
            }
            return Err(e);
        }
    }
    Ok(())
}
