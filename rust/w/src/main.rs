//! Rust port of the MINIX/NetBSD `w` utility.
//!
//! Usage:
//!   w [-h] [-s] [-u] [user]
//!
//! Displays who is logged in and what they are doing.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut no_header = false;
    let mut short = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                'h' => no_header = true,
                's' => short = true,
                'u' => {},
                _ => { eprintln!("usage: w [-h] [-s] [-u] [user]"); std::process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    let user_filter = argv.first().map(|s| s.as_str());

    #[cfg(unix)]
    {
        let data = match std::fs::read("/var/run/utmp") {
            Ok(d) => d,
            Err(_) => { eprintln!("w: no utmp"); std::process::exit(1); }
        };

        fn format_time(ts: u64) -> String {
            if ts == 0 { return "00:00".to_string(); }
            let hours = (ts / 3600) % 24;
            let mins = (ts / 60) % 60;
            format!("{:02}:{:02}", hours, mins)
        }

        let rec_size = 64;
        if !no_header && !short {
            println!("{:8} {:12} {:10} {:10} {}", "USER", "TTY", "LOGIN@", "IDLE", "WHAT");
        }
        if !no_header && short {
            println!("{:8} {:12} {:10}", "USER", "TTY", "WHAT");
        }

        let mut i = 0;
        while i + rec_size <= data.len() {
            let rec = &data[i..i + rec_size];
            let user_end = rec.iter().position(|&b| b == 0).unwrap_or(rec.len().min(8));
            let user = String::from_utf8_lossy(&rec[..user_end]).to_string();
            if user.is_empty() || user == "LOGIN" || user == "shutdown" { i += rec_size; continue; }

            if let Some(filter) = user_filter {
                if user != filter { i += rec_size; continue; }
            }

            let line_end = rec[8..].iter().position(|&b| b == 0).unwrap_or(8.min(rec.len().saturating_sub(8)));
            let tty = String::from_utf8_lossy(&rec[8..8+line_end]).to_string();

            let time_val = if rec.len() >= 36 {
                u32::from_ne_bytes([rec[32], rec[33], rec[34], rec[35]]) as u64
            } else { 0 };

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default().as_secs();
            let idle = if time_val > 0 { now - time_val } else { 0 };
            let idle_str = if idle < 60 {
                "-".to_string()
            } else if idle < 3600 {
                format!("{}m", idle / 60)
            } else {
                format!("{}h{}m", idle / 3600, (idle % 3600) / 60)
            };

            let time_str = format_time(time_val);

            if short {
                println!("{:<8} {:<12} {}", user, tty, "-");
            } else {
                println!("{:<8} {:<12} {:<10} {:>4} {}", user, tty, time_str, idle_str, "-");
            }
            i += rec_size;
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (user_filter, short, no_header);
        eprintln!("w: not supported on this platform");
        std::process::exit(1);
    }
}
