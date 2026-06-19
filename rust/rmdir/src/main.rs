//! Rust port of the MINIX/NetBSD `rmdir` utility.
//!
//! Usage:
//!   rmdir [-p] directory ...
//!
//! Removes empty directories. -p removes ancestor directories too.

use std::fs;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut parents = false;

    // Parse options
    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = &argv[0];
        for c in opt.chars().skip(1) {
            match c {
                'p' => parents = true,
                _ => {
                    eprintln!("rmdir: unknown option -- {c}");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    if argv.is_empty() {
        eprintln!("usage: rmdir [-p] directory ...");
        std::process::exit(1);
    }

    let mut had_error = false;
    for dir in argv {
        if let Err(e) = fs::remove_dir(dir) {
            eprintln!("rmdir: {}: {e}", dir);
            had_error = true;
        } else if parents {
            // Remove parent directories (use owned PathBuf to avoid dangling refs)
            let mut path = Path::new(dir).to_path_buf();
            loop {
                let parent = match path.parent() {
                    Some(p) => p.to_path_buf(),
                    None => break,
                };
                if parent.as_os_str().is_empty()
                    || parent == Path::new("/")
                    || parent == Path::new(".")
                {
                    break;
                }
                if fs::remove_dir(&parent).is_err() {
                    break;
                }
                path = parent;
            }
        }
    }

    if had_error {
        std::process::exit(1);
    }
}
