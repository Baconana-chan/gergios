//! Rust port of the MINIX/NetBSD `kill` utility.
//!
//! Usage:
//!   kill [-s signal] pid ...
//!   kill -l [number]
//!   kill -signal pid ...
//!
//! Sends a signal to a process, or lists signal names/numbers.

use std::io::{self, Write};

static SIGNAMES: &[(&str, i32)] = &[
    ("HUP", 1), ("INT", 2), ("QUIT", 3), ("ILL", 4), ("TRAP", 5),
    ("ABRT", 6), ("IOT", 6), ("FPE", 8), ("KILL", 9), ("BUS", 10),
    ("SEGV", 11), ("SYS", 12), ("PIPE", 13), ("ALRM", 14), ("TERM", 15),
    ("URG", 16), ("STOP", 17), ("TSTP", 18), ("CONT", 19), ("CHLD", 20),
    ("TTIN", 21), ("TTOU", 22), ("IO", 23), ("XCPU", 24), ("XFSZ", 25),
    ("VTALRM", 26), ("PROF", 27), ("WINCH", 28), ("INFO", 29), ("USR1", 30),
    ("USR2", 31),
];

fn sig_num(name: &str) -> Option<i32> {
    let upper = name.to_uppercase();
    if let Ok(n) = upper.parse::<i32>() {
        return Some(n);
    }
    let upper = upper.strip_prefix("SIG").unwrap_or(&upper);
    SIGNAMES.iter().find(|&&(n, _)| n == upper).map(|&(_, s)| s)
}

fn sig_name(num: i32) -> Option<&'static str> {
    SIGNAMES.iter().find(|&&(_, n)| n == num).map(|&(s, _)| s)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv: &[String] = &args[1..];

    if argv.is_empty() {
        eprintln!("usage: kill [-s signal] pid ...");
        eprintln!("       kill -l [number]");
        std::process::exit(1);
    }

    // -l: list signals
    if argv[0] == "-l" {
        if argv.len() >= 2 {
            // List specific signal
            if let Ok(num) = argv[1].parse::<i32>() {
                if let Some(name) = sig_name(num) {
                    println!("{name}");
                } else {
                    eprintln!("kill: unknown signal: SIG{}", argv[1]);
                    std::process::exit(1);
                }
            } else if let Some(num) = sig_num(&argv[1]) {
                println!("{num}");
            } else {
                eprintln!("kill: unknown signal: {}", argv[1]);
                std::process::exit(1);
            }
        } else {
            // List all signals
            for (name, num) in SIGNAMES {
                let col = format!("SIG{name}");
                print!("{col:9} {num:2}  ");
                if num % 4 == 0 { println!(); }
            }
            println!();
        }
        return;
    }

    // Parse signal specification
    let mut sig = libc::SIGTERM;
    let mut pids: &[String] = argv;

    if argv[0] == "-s" {
        // kill -s signal pid ...
        if argv.len() < 3 {
            eprintln!("usage: kill -s signal pid ...");
            std::process::exit(1);
        }
        if let Some(n) = sig_num(&argv[1]) {
            sig = n;
        } else {
            eprintln!("kill: unknown signal: {}", argv[1]);
            std::process::exit(1);
        }
        pids = &argv[2..];
    } else if argv[0].starts_with('-') && argv[0].len() > 1 {
        // kill -signal pid ...
        let sig_str = &argv[0][1..];
        if let Some(n) = sig_num(sig_str) {
            sig = n;
        } else {
            eprintln!("kill: unknown signal: {sig_str}");
            std::process::exit(1);
        }
        pids = &argv[1..];
    }

    // Send signal to each PID
    let mut had_error = false;
    for pid_str in pids {
        let pid: i32 = match pid_str.parse() {
            Ok(n) => n,
            Err(_) => {
                eprintln!("kill: invalid pid: `{pid_str}'");
                had_error = true;
                continue;
            }
        };
        let ret = unsafe { libc::kill(pid, sig) };
        if ret != 0 {
            eprintln!("kill: kill {}: {}", pid, std::io::Error::last_os_error());
            had_error = true;
        }
    }

    if had_error {
        std::process::exit(1);
    }
}
