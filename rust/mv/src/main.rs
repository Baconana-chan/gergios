//! Rust port of the MINIX/NetBSD `mv` utility.
//!
//! Usage:
//!   mv [-f] [-i] source target
//!   mv [-f] [-i] source ... directory
//!
//! Renames or moves files and directories.

use std::fs;
use std::io;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut force = false;
    let mut interactive = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        for c in argv[0].chars().skip(1) {
            match c {
                'f' => force = true,
                'i' => interactive = true,
                _ => {
                    eprintln!("mv: unknown option -- {c}");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    if argv.len() < 2 {
        eprintln!("usage: mv [-f] [-i] source target");
        eprintln!("       mv [-f] [-i] source ... directory");
        std::process::exit(1);
    }

    let target = argv.last().unwrap();
    let sources = &argv[..argv.len() - 1];

    // Check if target is a directory
    let is_dir = fs::metadata(target).ok().map_or(false, |m| m.is_dir());
    let multiple = sources.len() > 1;

    let mut had_error = false;

    if multiple || is_dir {
        for src in sources {
            let name = Path::new(src).file_name().unwrap_or(std::ffi::OsStr::new(src));
            let dest = Path::new(target).join(name);
            let dest_str = dest.to_string_lossy().to_string();
            if let Err(e) = do_move(src, &dest_str, force, interactive) {
                eprintln!("mv: {e}");
                had_error = true;
            }
        }
    } else {
        if let Err(e) = do_move(sources[0], target, force, interactive) {
            eprintln!("mv: {e}");
            std::process::exit(1);
        }
    }

    if had_error {
        std::process::exit(1);
    }
}

fn do_move(src: &str, dest: &str, force: bool, interactive: bool) -> io::Result<()> {
    // Helper: prompt for overwrite
    fn prompt_overwrite(dest: &str) -> io::Result<bool> {
        eprint!("mv: overwrite `{dest}'? ");
        std::io::stderr().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        Ok(input.trim().eq_ignore_ascii_case("y"))
    }

    // Check if dest exists
    if fs::metadata(dest).is_ok() {
        if interactive && !prompt_overwrite(dest)? {
            return Ok(());
        }
        if !force && !interactive {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists,
                format!("cannot overwrite `{dest}': file exists (use -f to force)")));
        }
        if force {
            fs::remove_file(dest).or_else(|_| fs::remove_dir(dest))?;
        }
    }

    // Try rename first, then copy+remove
    match fs::rename(src, dest) {
        Ok(()) => Ok(()),
        Err(_) => {
            // Cross-device move
            if fs::metadata(src)?.is_dir() {
                return Err(io::Error::new(io::ErrorKind::Other,
                    format!("cannot move directory `{src}' across devices")));
            }
            fs::copy(src, dest)?;
            fs::remove_file(src)?;
            Ok(())
        }
    }
}
