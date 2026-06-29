//! Rust port of the MINIX/NetBSD `last` utility.
//!
//! Usage:
//!   last [-n count] [-f file] [user ...]
//!
//! Shows last login sessions from wtmp file.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut count: usize = 20;
    let mut wtmp_file = "/var/log/wtmp".to_string();
    let mut user_filter: Option<String> = None;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        match opt.as_str() {
            "-n" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: last [-n count] [-f file] [user ...]"); std::process::exit(1); }
                count = argv[0].parse().unwrap_or_else(|_| { eprintln!("last: invalid count"); std::process::exit(1); });
            }
            "-f" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: last [-n count] [-f file] [user ...]"); std::process::exit(1); }
                wtmp_file = argv[0].clone();
            }
            _ => { eprintln!("usage: last [-n count] [-f file] [user ...]"); std::process::exit(1); }
        }
        argv = &argv[1..];
    }

    if !argv.is_empty() {
        user_filter = Some(argv[0].to_string());
    }

    #[cfg(unix)]
    {
        let data = match std::fs::read(&wtmp_file) {
            Ok(d) => d,
            Err(_) => { eprintln!("last: {wtmp_file}: no such file"); std::process::exit(1); }
        };

        // Simple wtmp parsing (utmp structs)
        let rec_size = 64;
        let mut entries: Vec<(String, String, String, String)> = Vec::new();
        let mut i = 0;

        while i + rec_size <= data.len() {
            let rec = &data[i..i + rec_size];
            let user_end = rec.iter().position(|&b| b == 0).unwrap_or(rec.len().min(8));
            let user = String::from_utf8_lossy(&rec[..user_end]).to_string();

            if !user.is_empty() && user != "shutdown" && user != "reboot" {
                let line_end = rec[8..].iter().position(|&b| b == 0).unwrap_or(8.min(rec.len().saturating_sub(8)));
                let line = String::from_utf8_lossy(&rec[8..8+line_end]).to_string();

                let host_end = rec[16..].iter().position(|&b| b == 0).unwrap_or(16.min(rec.len().saturating_sub(16)));
                let host = String::from_utf8_lossy(&rec[16..16+host_end]).to_string();

                let time_val = if rec.len() >= 36 {
                    u32::from_ne_bytes([rec[32], rec[33], rec[34], rec[35]]) as u64
                } else { 0 };

                if let Some(ref filter) = user_filter {
                    if !user.contains(filter.as_str()) { continue; }
                }

                let ts = format_time(time_val);
                entries.push((user, line, host, ts));
            }
            i += rec_size;
        }

        // Show last N entries (reversed)
        let start = entries.len().saturating_sub(count);
        for entry in entries.iter().rev().take(count) {
            println!("{:<8} {:<12} {:<16} {}", entry.0, entry.1, entry.2, entry.3);
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (count, user_filter);
        eprintln!("last: not supported on this platform");
        std::process::exit(1);
    }
}

fn format_time(ts: u64) -> String {
    if ts == 0 { return "still logged in".to_string(); }
    let days = ts / 86400;
    let remaining = ts % 86400;
    let hours = remaining / 3600;
    let mins = (remaining % 3600) / 60;
    let year = 1970 + (days as f64 / 365.25) as u64;
    format!("{:02}/{:02}/{:02} {:02}:{:02}",
        (days % 365 / 30) + 1, days % 30 + 1, year % 100, hours, mins)
}
