//! Rust port of the MINIX/NetBSD `nice` utility.
//!
//! Usage:
//!   nice [-n increment] command [args ...]
//!
//! Runs a program with modified scheduling priority.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut increment: i32 = 10;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        if opt.starts_with("-n") {
            if opt.len() > 2 {
                increment = opt[2..].parse().unwrap_or_else(|_| { eprintln!("nice: invalid increment"); std::process::exit(1); });
            } else {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: nice [-n increment] command"); std::process::exit(1); }
                increment = argv[0].parse().unwrap_or_else(|_| { eprintln!("nice: invalid increment"); std::process::exit(1); });
            }
        } else {
            // Old-style: -n means increment = n
            let digits: String = opt.chars().skip(1).collect();
            if let Ok(n) = digits.parse::<i32>() {
                increment = n;
            }
        }
        argv = &argv[1..];
    }

    if argv.is_empty() {
        eprintln!("usage: nice [-n increment] command");
        std::process::exit(1);
    }

    #[cfg(unix)]
    {
        unsafe { libc::nice(increment); }
    }
    #[cfg(not(unix))]
    {
        let _ = increment;
    }

    let status = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .status();

    match status {
        Ok(s) => std::process::exit(s.code().unwrap_or(0)),
        Err(e) => { eprintln!("nice: {e}"); std::process::exit(1); }
    }
}
