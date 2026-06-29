//! Rust port of the MINIX/NetBSD `lock` utility.
//!
//! Usage:
//!   lock [-p] [-t timeout]
//!
//! Locks the terminal until the password is entered.

use std::io::{self, Read, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut use_passwd = false;
    let mut timeout: Option<u32> = None;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                'p' => use_passwd = true,
                't' => {
                    argv = &argv[1..];
                    if argv.is_empty() { eprintln!("usage: lock [-p] [-t timeout]"); std::process::exit(1); }
                    timeout = Some(argv[0].parse().unwrap_or_else(|_| { eprintln!("lock: invalid timeout"); std::process::exit(1); }));
                }
                _ => { eprintln!("usage: lock [-p] [-t timeout]"); std::process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    let password = if use_passwd {
        // Use system password
        std::env::var("USER").unwrap_or_default()
    } else {
        // Prompt for password
        let pw1 = getpass("Key: ");
        let pw2 = getpass("Again: ");
        if pw1 != pw2 {
            eprintln!("lock: passwords don't match");
            std::process::exit(1);
        }
        pw1
    };

    if password.is_empty() {
        eprintln!("lock: no password set");
        std::process::exit(1);
    }

    eprintln!("lock: terminal locked by {}", std::env::var("USER").unwrap_or_else(|_| "unknown".to_string()));

    let start = std::time::Instant::now();
    loop {
        print!("Key: ");
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input).ok();
        if input.trim() == password {
            eprintln!("lock: terminal unlocked");
            break;
        }
        eprintln!("lock: incorrect password");

        if let Some(t) = timeout {
            if start.elapsed().as_secs() >= t as u64 {
                eprintln!("\nlock: timed out");
                std::process::exit(1);
            }
        }
    }
}

fn getpass(prompt: &str) -> String {
    print!("{}", prompt);
    io::stdout().flush().ok();
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    input.trim().to_string()
}
