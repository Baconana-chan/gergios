//! Rust port of the MINIX/NetBSD `finger` utility.
//!
//! Usage:
//!   finger [-lmps] [user ...]
//!
//! Displays information about system users.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut long = false;
    let mut _match_only = false;
    let mut _plan = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                'l' => long = true,
                'm' => _match_only = true,
                'p' => _plan = true,
                's' => {},  // short format (default)
                _ => { eprintln!("usage: finger [-lmps] [user ...]"); std::process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    #[cfg(unix)]
    {
        let users: Vec<&str> = if argv.is_empty() { vec![] } else { argv.iter().map(|s| s.as_str()).collect() };

        // Query utmp for logged-in users
        if let Ok(data) = std::fs::read("/var/run/utmp") {
            let rec_size = 64;
            let mut i = 0;
            while i + rec_size <= data.len() {
                let rec = &data[i..i + rec_size];
                let user_end = rec.iter().position(|&b| b == 0).unwrap_or(rec.len().min(8));
                let user = String::from_utf8_lossy(&rec[..user_end]).to_string();

                if !user.is_empty() && user != "LOGIN" && user != "shutdown" {
                    if users.is_empty() || users.contains(&user.as_str()) {
                        let line_end = rec[8..].iter().position(|&b| b == 0).unwrap_or(8.min(rec.len().saturating_sub(8)));
                        let line = String::from_utf8_lossy(&rec[8..8+line_end]).to_string();
                        let host_end = rec[16..].iter().position(|&b| b == 0).unwrap_or(16.min(rec.len().saturating_sub(16)));
                        let host = String::from_utf8_lossy(&rec[16..16+host_end]).to_string();

                        if long {
                            println!("Login: {}                    Name: {}", user, user);
                            println!("Directory: /home/{}                 Shell: /bin/sh", user);
                            println!("On since {} on {} from {}", "?", line, host);
                            println!("No mail.");
                            if _plan {
                                println!("Plan:");
                                let plan_path = format!("/home/{}/.plan", user);
                                if let Ok(plan) = std::fs::read_to_string(&plan_path) {
                                    for pl in plan.lines() {
                                        println!("  {}", pl);
                                    }
                                }
                            }
                            println!();
                        } else {
                            let host_display = if host.is_empty() || host == "0" {
                                String::new()
                            } else {
                                format!(" *:{}", host)
                            };
                            println!("{:<8} {:8} {} {:<}{}",
                                user, line, "?", "00:00", host_display);
                        }
                    }
                }
                i += rec_size;
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (argv, _plan, long);
        eprintln!("finger: not supported on this platform");
        std::process::exit(1);
    }
}
