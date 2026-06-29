//! Rust port of the MINIX/NetBSD `shlock` utility.
//!
//! Usage:
//!   shlock -f lockfile [-p pid] [-u]
//!
//! Creates or checks lock files for shell scripts.

use std::fs;
use std::io::Write;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut lockfile: Option<String> = None;
    let mut pid: Option<u32> = None;
    let mut _unlock = false;

    while !argv.is_empty() && argv[0].starts_with('-') {
        let opt = argv[0].clone();
        if opt == "--" { break; }
        match opt.as_str() {
            "-f" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: shlock -f lockfile [-p pid]"); std::process::exit(1); }
                lockfile = Some(argv[0].clone());
            }
            "-p" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: shlock -f lockfile [-p pid]"); std::process::exit(1); }
                pid = Some(argv[0].parse().unwrap_or_else(|_| { eprintln!("shlock: invalid pid"); std::process::exit(1); }));
            }
            "-u" => _unlock = true,
            _ => { eprintln!("usage: shlock -f lockfile [-p pid] [-u]"); std::process::exit(1); }
        }
        if argv.is_empty() { break; }
        argv = &argv[1..];
    }

    let lockfile = lockfile.unwrap_or_else(|| {
        eprintln!("usage: shlock -f lockfile [-p pid] [-u]");
        std::process::exit(1);
    });

    let path = Path::new(&lockfile);

    if path.exists() {
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(existing_pid) = content.trim().parse::<u32>() {
                #[cfg(unix)]
                {
                    if unsafe { libc::kill(existing_pid as i32, 0) } == 0 {
                        std::process::exit(1);
                    }
                }
                #[cfg(not(unix))]
                {
                    if existing_pid != pid.unwrap_or(0) {
                        std::process::exit(1);
                    }
                }
            }
        }
    }

    if let Some(p) = pid {
        let mut file = match fs::File::create(path) {
            Ok(f) => f,
            Err(_) => { std::process::exit(1); }
        };
        let _ = writeln!(file, "{}", p);
    }
}
