//! Rust port of the MINIX/NetBSD `whereis` utility.
//!
//! Usage:
//!   whereis [-bms] [file ...]
//!
//! Locates binary, source, and manual page files for a command.
//! -b: search for binaries only
//! -m: search for manual pages only
//! -s: search for source only

use std::fs;
use std::io::Write;
use std::path::Path;

const USAGE: &str = "usage: whereis [-bms] [file ...]";

const DEFAULT_BIN_DIRS: &[&str] = &[
    "/usr/bin", "/bin", "/usr/sbin", "/sbin",
    "/usr/local/bin", "/usr/local/sbin",
    "/usr/pkg/bin", "/usr/pkg/sbin",
];

const DEFAULT_MAN_DIRS: &[&str] = &[
    "/usr/share/man/man1", "/usr/share/man/man2",
    "/usr/share/man/man3", "/usr/share/man/man4",
    "/usr/share/man/man5", "/usr/share/man/man6",
    "/usr/share/man/man7", "/usr/share/man/man8",
];

const DEFAULT_SRC_DIRS: &[&str] = &[
    "/usr/src", "/usr/local/src",
];

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut search_bin = true;
    let mut search_man = true;
    let mut search_src = true;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        let mut found_flag = false;
        for ch in opt.chars().skip(1) {
            found_flag = true;
            match ch {
                'b' => { search_man = false; search_src = false; }
                'm' => { search_bin = false; search_src = false; }
                's' => { search_bin = false; search_man = false; }
                _ => { eprintln!("{USAGE}"); std::process::exit(1); }
            }
        }
        if !found_flag {
            eprintln!("{USAGE}");
            std::process::exit(1);
        }
        argv = &argv[1..];
    }

    if argv.is_empty() {
        eprintln!("{USAGE}");
        std::process::exit(1);
    }

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    for name in argv {
        let mut results: Vec<String> = Vec::new();

        if search_bin {
            for dir in DEFAULT_BIN_DIRS {
                let path = format!("{dir}/{name}");
                if Path::new(&path).exists() {
                    results.push(path);
                }
            }
        }

        if search_man {
            for dir in DEFAULT_MAN_DIRS {
                // Check for name.ext where ext is 1-9
                if let Ok(entries) = fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        let fname = entry.file_name().to_string_lossy().to_string();
                        if let Some(dot) = fname.rfind('.') {
                            let base = &fname[..dot];
                            let ext = &fname[dot + 1..];
                            if base == name && ext.len() == 1 && ext.as_bytes()[0].is_ascii_digit() {
                                results.push(format!("{dir}/{fname}"));
                                break;
                            }
                        }
                    }
                }
            }
        }

        if search_src {
            for dir in DEFAULT_SRC_DIRS {
                let path = format!("{dir}/{name}");
                if Path::new(&path).exists() {
                    results.push(path);
                }
            }
        }

        if results.is_empty() {
            writeln!(out, "{name}:").ok();
        } else {
            writeln!(out, "{}: {}", name, results.join(" ")).ok();
        }
    }
}
