//! Rust port of the MINIX/NetBSD `newgrp` utility.
//!
//! Usage:
//!   newgrp [-] [group]
//!
//! Changes the real and effective group ID.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut _login_shell = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        if argv[0] == "-" || argv[0] == "--" {
            _login_shell = true;
        }
        argv = &argv[1..];
        break;
    }

    let target_group = argv.first().map(|s| s.as_str());

    #[cfg(unix)]
    {
        let gid = if let Some(group) = target_group {
            let c_str = std::ffi::CString::new(group).unwrap();
            let grp = unsafe { libc::getgrnam(c_str.as_ptr()) };
            if grp.is_null() {
                eprintln!("newgrp: unknown group: {group}");
                std::process::exit(1);
            }
            unsafe { (*grp).gr_gid }
        } else {
            // Default group from passwd
            let pw = unsafe { libc::getpwuid(libc::getuid()) };
            if pw.is_null() {
                eprintln!("newgrp: cannot find user");
                std::process::exit(1);
            }
            unsafe { (*pw).pw_gid }
        };

        if unsafe { libc::setgid(gid) } != 0 {
            eprintln!("newgrp: permission denied");
            std::process::exit(1);
        }

        // Start shell
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        let err = std::process::Command::new(&shell).status();
        match err {
            Ok(_) => {},
            Err(e) => { eprintln!("newgrp: {e}"); std::process::exit(1); }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = target_group;
        eprintln!("newgrp: not supported on this platform");
        std::process::exit(1);
    }
}
