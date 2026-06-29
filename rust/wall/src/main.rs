//! Rust port of the MINIX/NetBSD `wall` utility.
//!
//! Usage:
//!   wall [-g group] [file]
//!
//! Broadcasts a message to all logged-in users.
//! Requires appropriate privileges (setgid tty).

use std::fs::File;
use std::io::{self, Read};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        if opt == "-g" && opt.len() == 2 {
            argv = &argv[1..];
            if argv.is_empty() { eprintln!("usage: wall [-g group] [file]"); std::process::exit(1); }
        } else {
            eprintln!("usage: wall [-g group] [file]");
            std::process::exit(1);
        }
        argv = &argv[1..];
    }

    let fname = argv.first().cloned();

    let mut message = String::new();
    match fname {
        Some(ref f) if f != "-" => {
            match File::open(f) {
                Ok(mut file) => { file.read_to_string(&mut message).ok(); }
                Err(e) => { eprintln!("wall: {f}: {e}"); std::process::exit(1); }
            }
        }
        _ => {
            io::stdin().read_to_string(&mut message).ok();
        }
    }

    if message.trim().is_empty() {
        eprintln!("wall: no message");
        std::process::exit(1);
    }

    broadcast(&message);
}

#[cfg(unix)]
fn broadcast(message: &str) {
    use std::io::{BufRead, BufReader, Write};
    let utmp_paths = &["/var/run/utmp", "/var/log/wtmp", "/etc/utmp"];
    let mut tty_devices: Vec<String> = Vec::new();

    for path in utmp_paths {
        if let Ok(f) = File::open(path) {
            let reader = BufReader::new(f);
            for line in reader.split(b'\n') {
                if let Ok(chunk) = line {
                    if chunk.len() >= 64 {
                        let line_start = 32.min(chunk.len());
                        let line_end = chunk[line_start..].iter()
                            .position(|&b| b == 0)
                            .unwrap_or(chunk.len().saturating_sub(line_start))
                            + line_start;
                        let tty_name = String::from_utf8_lossy(&chunk[line_start..line_end])
                            .trim().to_string();
                        if !tty_name.is_empty() && tty_name != "~" {
                            let dev_path = format!("/dev/{}", tty_name);
                            if !tty_devices.contains(&dev_path) {
                                tty_devices.push(dev_path);
                            }
                        }
                    }
                }
            }
        }
    }

    if tty_devices.is_empty() {
        tty_devices.push("/dev/console".to_string());
    }

    let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
    let hostname = std::process::Command::new("hostname")
        .output().ok().and_then(|o| {
            String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
        }).unwrap_or_default();

    let header = format!("\r\rBroadcast Message from {} on {}\r\r", user, hostname);

    for tty in &tty_devices {
        if let Ok(mut file) = File::options().write(true).open(tty) {
            let _ = file.write_all(header.as_bytes());
            let _ = file.write_all(message.as_bytes());
            let _ = file.write_all(b"\r\n");
        }
    }
}

#[cfg(not(unix))]
fn broadcast(_message: &str) {
    eprintln!("wall: not supported on this platform");
    std::process::exit(1);
}
