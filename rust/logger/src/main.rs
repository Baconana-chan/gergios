//! Rust port of the MINIX/NetBSD `logger` utility.
//!
//! Usage:
//!   logger [-p facility.priority] [-t tag] [message ...]
//!
//! Sends messages to syslog.

use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut priority = "user.notice".to_string();
    let mut tag = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        match opt.as_str() {
            "-p" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: logger [-p pri] [-t tag] [message]"); std::process::exit(1); }
                priority = argv[0].clone();
            }
            "-t" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: logger [-p pri] [-t tag] [message]"); std::process::exit(1); }
                tag = argv[0].clone();
            }
            _ => { eprintln!("usage: logger [-p pri] [-t tag] [message]"); std::process::exit(1); }
        }
        argv = &argv[1..];
    }

    let message = if argv.is_empty() {
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf).ok();
        buf.trim().to_string()
    } else {
        argv.join(" ")
    };

    // Try syslog command, fallback to stderr
    #[cfg(unix)]
    {
        let output = Command::new("logger")
            .args(&["-p", &priority, "-t", &tag, &message])
            .output();
        if output.is_ok() { return; }
    }

    // Fallback: write to console/stderr
    eprintln!("<{}> {}: {}", priority, tag, message);
}
