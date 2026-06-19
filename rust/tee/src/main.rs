//! Rust port of the MINIX/NetBSD `tee` utility.
//!
//! Usage:
//!   tee [-ai] [file ...]
//!
//! Reads from stdin and writes to stdout and one or more files.

use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut append = false;
    let mut ignore_intr = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        for c in argv[0].chars().skip(1) {
            match c {
                'a' => append = true,
                'i' => ignore_intr = true,
                _ => {
                    eprintln!("tee: unknown option -- {c}");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    // Open output files
    let mut files: Vec<File> = Vec::new();
    for name in argv {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(append)
            .truncate(!append)
            .open(name)
            .unwrap_or_else(|e| {
                eprintln!("tee: {name}: {e}");
                std::process::exit(1);
            });
        files.push(file);
    }

    // Read stdin and write to stdout + files
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut stdin = io::stdin().lock();
    let mut buf = [0u8; 8192];

    loop {
        let n = match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted && ignore_intr => continue,
            Err(e) => {
                eprintln!("tee: {e}");
                std::process::exit(1);
            }
        };

        let data = &buf[..n];

        // Write to stdout
        if out.write_all(data).is_err() {
            // SIGPIPE — exit silently
            std::process::exit(0);
        }

        // Write to files
        for file in &mut files {
            if let Err(e) = file.write_all(data) {
                eprintln!("tee: write error: {e}");
                std::process::exit(1);
            }
        }
    }
}
