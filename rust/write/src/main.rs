//! Rust port of the MINIX/NetBSD `write` utility.
//!
//! Usage:
//!   write user [tty]
//!
//! Sends a message to another user's terminal.

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 || args.len() > 3 {
        eprintln!("usage: write user [tty]");
        std::process::exit(1);
    }

    let target_user = &args[1];
    let target_tty = args.get(2);

    #[cfg(unix)]
    {
        use std::fs::File;
        use std::io::{self, BufRead, BufReader, Read, Write};

        let tty_path = find_user_tty(target_user, target_tty);

        let tty_path = match tty_path {
            Some(p) => p,
            None => {
                if target_tty.is_some() {
                    eprintln!("write: {} is not logged in on {}", target_user, target_tty.unwrap());
                } else {
                    eprintln!("write: {} is not logged in", target_user);
                }
                std::process::exit(1);
            }
        };

        if !tty_writable(&tty_path) {
            eprintln!("write: {} has messages disabled on {}", target_user, tty_path);
            std::process::exit(1);
        }

        let sender = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());

        let hostname = std::process::Command::new("hostname")
            .output().ok().and_then(|o| {
                String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
            }).unwrap_or_default();

        let mut tty = match File::options().write(true).open(&tty_path) {
            Ok(f) => f,
            Err(_) => { eprintln!("write: cannot open {}", tty_path); std::process::exit(1); }
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs();
        let hours = (secs / 3600) % 24;
        let mins = (secs / 60) % 60;

        let header = format!(
            "\r\rMessage from {}@{} on {} at {:02}:{:02}\r\r",
            sender, hostname, tty_path, hours, mins
        );

        let _ = tty.write_all(header.as_bytes());

        let mut message = String::new();
        let _ = io::stdin().read_to_string(&mut message);

        for line in message.lines() {
            let _ = writeln!(tty, "\r{}", line);
        }
        let _ = writeln!(tty, "\rEOF\r");
    }

    #[cfg(not(unix))]
    {
        let _ = (target_user, target_tty);
        eprintln!("write: not supported on this platform");
        std::process::exit(1);
    }
}

#[cfg(unix)]
fn find_user_tty(user: &str, specific_tty: Option<&String>) -> Option<String> {
    use std::fs::File;
    use std::io::{BufRead, BufReader};

    let utmp_paths = &["/var/run/utmp", "/var/log/wtmp", "/etc/utmp"];

    for path in utmp_paths {
        if let Ok(f) = File::open(path) {
            let reader = BufReader::new(f);
            for line in reader.split(b'\n') {
                if let Ok(chunk) = line {
                    if chunk.len() >= 64 {
                        let end = chunk.iter().position(|&b| b == 0).unwrap_or(chunk.len().min(8));
                        let ut_user = String::from_utf8_lossy(&chunk[..end.min(8)]).to_string();
                        if ut_user == user {
                            let line_start = 8;
                            let line_end = chunk[line_start..].iter()
                                .position(|&b| b == 0)
                                .map(|p| p + line_start)
                                .unwrap_or(line_start + 16.min(chunk.len().saturating_sub(line_start)));
                            let ut_line = String::from_utf8_lossy(&chunk[line_start..line_end.min(chunk.len())])
                                .trim().to_string();
                            if !ut_line.is_empty() && ut_line != "~" {
                                let dev = format!("/dev/{}", ut_line);
                                if let Some(ref spec) = specific_tty {
                                    if dev.ends_with(spec) || dev.contains(spec) {
                                        return Some(dev);
                                    }
                                } else {
                                    return Some(dev);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(unix)]
fn tty_writable(path: &str) -> bool {
    if let Ok(meta) = std::fs::metadata(path) {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o002 != 0
    } else {
        false
    }
}
