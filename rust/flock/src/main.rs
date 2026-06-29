//! Rust port of the MINIX/NetBSD `flock` utility.
//!
//! Usage:
//!   flock [-sxun] [-w timeout] lockfile command [args ...]
//!
//! Acquires a file lock and executes a command.

use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut shared = false;
    let mut exclusive = false;
    let mut nonblock = false;
    let mut unlock = false;
    let mut timeout: Option<u32> = None;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                's' => shared = true,
                'x' => exclusive = true,
                'n' => nonblock = true,
                'u' => unlock = true,
                'w' => {
                    if opt.len() > 2 {
                        timeout = Some(opt[2..].parse().unwrap_or_else(|_| { eprintln!("flock: invalid timeout"); process::exit(1); }));
                    } else {
                        argv = &argv[1..];
                        if argv.is_empty() { eprintln!("usage: flock [-sxun] [-w timeout] lockfile command"); process::exit(1); }
                        timeout = Some(argv[0].parse().unwrap_or_else(|_| { eprintln!("flock: invalid timeout"); process::exit(1); }));
                    }
                }
                _ => { eprintln!("usage: flock [-sxun] [-w timeout] lockfile command"); process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    if argv.len() < 2 {
        eprintln!("usage: flock [-sxun] [-w timeout] lockfile command");
        process::exit(1);
    }

    let lockfile = argv[0].to_string();
    let cmd: Vec<String> = argv[1..].to_vec();

    #[cfg(unix)]
    {
        use std::fs::File;
        use std::os::unix::io::AsRawFd;

        let lock_type = if unlock { libc::LOCK_UN }
            else if shared { libc::LOCK_SH }
            else { libc::LOCK_EX };

        let flags = lock_type | if nonblock { libc::LOCK_NB } else { 0 };

        let fd = match File::create(&lockfile) {
            Ok(f) => f,
            Err(e) => { eprintln!("flock: cannot open {lockfile}: {e}"); process::exit(1); }
        };

        if let Some(secs) = timeout {
            let start = std::time::Instant::now();
            loop {
                let result = unsafe { libc::flock(fd.as_raw_fd(), flags & !libc::LOCK_NB) };
                if result == 0 { break; }
                if start.elapsed().as_secs() >= secs as u64 {
                    eprintln!("flock: timeout waiting for lock on {lockfile}");
                    process::exit(1);
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        } else if unsafe { libc::flock(fd.as_raw_fd(), flags) } != 0 {
            if nonblock {
                eprintln!("flock: cannot acquire lock on {lockfile}");
                process::exit(1);
            }
            eprintln!("flock: error: {}", std::io::Error::last_os_error());
            process::exit(1);
        }

        let status = process::Command::new(&cmd[0])
            .args(&cmd[1..])
            .status();

        drop(fd);

        match status {
            Ok(s) => process::exit(s.code().unwrap_or(0)),
            Err(e) => { eprintln!("flock: {e}"); process::exit(1); }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (shared, exclusive, nonblock, unlock, timeout);
        eprintln!("flock: not supported on this platform");
        process::exit(1);
    }
}
