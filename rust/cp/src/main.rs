//! Rust port of the MINIX/NetBSD `cp` utility.
//!
//! Usage:
//!   cp [-R] source target
//!   cp [-R] source ... directory
//!
//! Copies files and directories.

use std::fs;
use std::io;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut recursive = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        for c in argv[0].chars().skip(1) {
            match c {
                'R' | 'r' => recursive = true,
                _ => {
                    eprintln!("cp: unknown option -- {c}");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    if argv.len() < 2 {
        eprintln!("usage: cp [-R] source target");
        eprintln!("       cp [-R] source ... directory");
        std::process::exit(1);
    }

    let target = argv.last().unwrap();
    let sources = &argv[..argv.len() - 1];

    let is_dir = fs::metadata(target).ok().map_or(false, |m| m.is_dir());
    let multiple = sources.len() > 1;

    let mut had_error = false;

    if multiple || is_dir {
        for src in sources {
            let name = Path::new(src).file_name().unwrap_or(std::ffi::OsStr::new(src));
            let dest = Path::new(target).join(name);
            if let Err(e) = copy_entry(src, &dest.to_string_lossy(), recursive) {
                eprintln!("cp: {e}");
                had_error = true;
            }
        }
    } else {
        if let Err(e) = copy_entry(sources[0], target, recursive) {
            eprintln!("cp: {e}");
            std::process::exit(1);
        }
    }

    if had_error {
        std::process::exit(1);
    }
}

fn copy_entry(src: &str, dest: &str, recursive: bool) -> io::Result<()> {
    let meta = fs::metadata(src)?;

    if meta.is_dir() {
        if !recursive {
            return Err(io::Error::new(io::ErrorKind::Other,
                format!("cannot copy directory `{src}' without -R")));
        }
        fs::create_dir_all(dest)?;

        // Preserve permissions
        let perms = meta.permissions();
        fs::set_permissions(dest, perms)?;

        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let name = entry.file_name();
            let src_child = Path::new(src).join(&name);
            let dst_child = Path::new(dest).join(&name);
            copy_entry(
                &src_child.to_string_lossy(),
                &dst_child.to_string_lossy(),
                true,
            )?;
        }
        Ok(())
    } else {
        fs::copy(src, dest)?;
        Ok(())
    }
}
