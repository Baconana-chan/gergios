//! Rust port of the MINIX/NetBSD `renice` utility.
//!
//! Usage:
//!   renice [-n increment] [-g | -p | -u] ID ...
//!
//! Alters priority of running processes.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut increment: i32 = 0;
    let mut who: u8 = 0; // 0=pid, 1=pgid, 2=uid

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        match opt.as_str() {
            "-n" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: renice [-n incr] [-gpu] ID ..."); std::process::exit(1); }
                increment = argv[0].parse().unwrap_or_else(|_| { eprintln!("renice: invalid increment"); std::process::exit(1); });
            }
            "-g" => who = 1,
            "-p" => who = 0,
            "-u" => who = 2,
            _ => { eprintln!("usage: renice [-n incr] [-gpu] ID ..."); std::process::exit(1); }
        }
        argv = &argv[1..];
    }

    if argv.is_empty() {
        eprintln!("usage: renice [-n incr] [-gpu] ID ...");
        std::process::exit(1);
    }

    #[cfg(unix)]
    {
        for id_str in argv {
            let id: i32 = match id_str.parse() {
                Ok(n) => n,
                Err(_) => { eprintln!("renice: invalid id: {id_str}"); continue; }
            };

            let result = match who {
                0 => unsafe { libc::setpriority(libc::PRIO_PROCESS, id as u32, increment) },
                1 => unsafe { libc::setpriority(libc::PRIO_PGRP, id as u32, increment) },
                _ => unsafe { libc::setpriority(libc::PRIO_USER, id as u32, increment) },
            };

            if result != 0 {
                eprintln!("renice: {}: {}", id_str, std::io::Error::last_os_error());
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (increment, who);
        eprintln!("renice: not supported on this platform");
        std::process::exit(1);
    }
}
