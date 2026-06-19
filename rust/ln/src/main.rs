//! Rust port of the MINIX/NetBSD `ln` utility.
//!
//! Usage:
//!   ln [-f] [-s] source [target]
//!   ln [-f] [-s] source ... directory
//!
//! Creates hard links (-s creates symbolic links).

use std::fs;
use std::io;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut force = false;
    let mut symbolic = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        for c in argv[0].chars().skip(1) {
            match c {
                'f' => force = true,
                's' => symbolic = true,
                _ => {
                    eprintln!("ln: unknown option -- {c}");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    if argv.len() < 1 {
        eprintln!("usage: ln [-f] [-s] source [target]");
        eprintln!("       ln [-f] [-s] source ... directory");
        std::process::exit(1);
    }

    if argv.len() == 1 {
        // Create link in current directory with source's basename
        let src = &argv[0];
        let target = src.rsplit('/').next().unwrap_or(src);
        let _ = do_link(src, target, symbolic, force);
        return;
    }

    let target = argv.last().unwrap();
    let sources = &argv[..argv.len() - 1];

    // Check if target is a directory
    let is_dir = fs::metadata(target).ok().map_or(false, |m| m.is_dir());

    if is_dir {
        // Multiple sources: link each into the directory
        let mut had_error = false;
        for src in sources {
            let name = src.rsplit('/').next().unwrap_or(src);
            let dest = format!("{}/{}", target, name);
            if let Err(e) = do_link(src, &dest, symbolic, force) {
                eprintln!("ln: {e}");
                had_error = true;
            }
        }
        if had_error { std::process::exit(1); }
    } else {
        // Single source, named target
        if sources.len() > 1 {
            eprintln!("ln: {}: not a directory", target);
            std::process::exit(1);
        }
        if let Err(e) = do_link(sources[0], target, symbolic, force) {
            eprintln!("ln: {e}");
            std::process::exit(1);
        }
    }
}

fn do_link(src: &str, dest: &str, symbolic: bool, force: bool) -> io::Result<()> {
    // -f: remove dest first if it exists
    if force && fs::metadata(dest).is_ok() {
        if symbolic {
            let _ = fs::remove_file(dest);
        } else {
            fs::remove_file(dest).or_else(|_| fs::remove_dir(dest))?;
        }
    }

    if symbolic {
        std::os::unix::fs::symlink(src, dest)?;
    } else {
        fs::hard_link(src, dest)?;
    }
    Ok(())
}
