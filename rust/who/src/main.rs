//! Rust port of the MINIX/NetBSD `who` utility.
//!
//! Usage:
//!   who [-a] [-b] [-d] [-H] [-l] [-p] [-q] [-r] [-s] [-t] [-T] [-u] [file]
//!
//! Shows who is logged in.

use std::fs::File;
use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut show_headers = false;
    let mut utmp_file = "/var/run/utmp".to_string();
    let mut count_only = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                'H' => show_headers = true,
                'q' => count_only = true,
                'u' | 's' | 'a' | 'b' | 'd' | 'l' | 'p' | 'r' | 't' | 'T' => {},
                _ => { eprintln!("usage: who [-a] [-b] [-d] [-H] [-l] [-p] [-q] [-r] [-s] [-t] [-T] [-u] [file]"); std::process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    if !argv.is_empty() {
        utmp_file = argv[0].clone();
    }

    let entries = parse_utmp(&utmp_file);

    if count_only {
        println!("{}", entries.len());
        return;
    }

    if show_headers {
        println!("{:8} {:12} {:10} {:10}", "USER", "LINE", "TIME", "IDLE");
    }

    for entry in &entries {
        println!("{:8} {:12} {:10} {:10}",
            entry.user, entry.line, entry.time, entry.idle);
    }
}

struct UtmpEntry {
    user: String,
    line: String,
    time: String,
    idle: String,
}

fn parse_utmp(path: &str) -> Vec<UtmpEntry> {
    let mut entries = Vec::new();

    let data = match File::open(path) {
        Ok(mut f) => {
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).ok();
            buf
        }
        Err(_) => return entries,
    };

    let rec_size = 64;
    let mut i = 0;
    while i + rec_size <= data.len() {
        let rec = &data[i..i + rec_size];
        let user = extract_str(rec, 0, 8);
        let line = extract_str(rec, 8, 16);
        let _host = extract_str(rec, 16, 32);
        let _timeval: u64 = read_u32(rec, 32) as u64;

        if !user.is_empty() && !line.is_empty() && user != "LOGIN" {
            entries.push(UtmpEntry {
                user,
                line,
                time: format_time(_timeval),
                idle: ".".to_string(),
            });
        }

        i += rec_size;
    }

    entries
}

fn extract_str(data: &[u8], start: usize, len: usize) -> String {
    let end = data[start..start + len].iter()
        .position(|&b| b == 0)
        .unwrap_or(len);
    String::from_utf8_lossy(&data[start..start + end]).to_string()
}

fn read_u32(data: &[u8], pos: usize) -> u32 {
    if pos + 4 <= data.len() {
        u32::from_ne_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]])
    } else {
        0
    }
}

fn format_time(ts: u64) -> String {
    let days = ts / 86400;
    let remaining = ts % 86400;
    let hours = remaining / 3600;
    let mins = (remaining % 3600) / 60;

    if days > 0 {
        format!("{:2}+{:02}:{:02}", days, hours, mins)
    } else {
        format!("  {:02}:{:02}", hours, mins)
    }
}
