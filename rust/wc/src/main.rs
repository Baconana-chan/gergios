//! Rust port of the MINIX/NetBSD `wc` utility.
//!
//! Usage:
//!   wc [-clmw] [file ...]
//!
//! Counts lines, words, characters, and/or bytes in files.
//! Default output: lines, words, bytes (POSIX).

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut cflag = false; // bytes
    let mut lflag = false; // lines
    let mut mflag = false; // chars
    let mut wflag = false; // words

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        for ch in argv[0].chars().skip(1) {
            match ch {
                'c' => cflag = true,
                'l' => lflag = true,
                'm' => mflag = true,
                'w' => wflag = true,
                _ => {
                    eprintln!("wc: unknown option -- {ch}");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    // Default: -l -w -c (lines, words, bytes)
    if !cflag && !lflag && !mflag && !wflag {
        lflag = true;
        wflag = true;
        cflag = true;
    }

    // -m overrides -c per POSIX
    if mflag {
        cflag = false;
    }

    let files: Vec<&str> = if argv.is_empty() {
        vec!["-"]
    } else {
        argv.iter().map(|s| s.as_str()).collect()
    };

    let mut total_lines: u64 = 0;
    let mut total_words: u64 = 0;
    let mut total_chars: u64 = 0;
    let mut total_bytes: u64 = 0;
    let multiple = files.len() > 1;
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut had_error = false;

    for filename in &files {
        let result = if *filename == "-" {
            let mut stdin = io::stdin().lock();
            let mut content = Vec::new();
            if stdin.read_to_end(&mut content).is_err() {
                eprintln!("wc: stdin: read error");
                had_error = true;
                continue;
            }
            Some(count_content(&content, mflag))
        } else {
            match File::open(filename) {
                Ok(mut file) => {
                    let mut content = Vec::new();
                    if file.read_to_end(&mut content).is_err() {
                        eprintln!("wc: {filename}: read error");
                        had_error = true;
                        None
                    } else {
                        Some(count_content(&content, mflag))
                    }
                }
                Err(e) => {
                    eprintln!("wc: {filename}: {e}");
                    had_error = true;
                    None
                }
            }
        };

        if let Some((lines, words, chars, bytes)) = result {
            let _ = write!(out, " ");
            if lflag { let _ = write!(out, "{:>7} ", lines); }
            if wflag { let _ = write!(out, "{:>7} ", words); }
            if mflag { let _ = write!(out, "{:>7} ", chars); }
            if cflag { let _ = write!(out, "{:>7} ", bytes); }
            let _ = writeln!(out, "{filename}");

            total_lines += lines;
            total_words += words;
            total_chars += chars;
            total_bytes += bytes;
        }
    }

    if multiple {
        let _ = write!(out, " ");
        if lflag { let _ = write!(out, "{:>7} ", total_lines); }
        if wflag { let _ = write!(out, "{:>7} ", total_words); }
        if mflag { let _ = write!(out, "{:>7} ", total_chars); }
        if cflag { let _ = write!(out, "{:>7} ", total_bytes); }
        let _ = writeln!(out, "total");
    }

    if had_error { std::process::exit(1); }
}

fn count_content(content: &[u8], count_chars: bool) -> (u64, u64, u64, u64) {
    let bytes = content.len() as u64;
    let mut lines: u64 = 0;
    let mut words: u64 = 0;
    let mut chars: u64 = 0;
    let mut in_word = false;

    if count_chars {
        // UTF-8 character count
        let text = String::from_utf8_lossy(content);
        chars = text.chars().count() as u64;

        for ch in text.chars() {
            if ch == '\n' { lines += 1; }
            if ch.is_whitespace() {
                in_word = false;
            } else if !in_word {
                in_word = true;
                words += 1;
            }
        }
    } else {
        // Byte count
        for &b in content {
            if b == b'\n' { lines += 1; }
            if b.is_ascii_whitespace() {
                in_word = false;
            } else if !in_word {
                in_word = true;
                words += 1;
            }
        }
    }

    // If last char is not newline, POSIX wc still counts lines as number of newlines
    (lines, words, chars, bytes)
}
