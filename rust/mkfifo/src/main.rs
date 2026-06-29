//! Rust port of the MINIX/NetBSD `mkfifo` utility.
//!
//! Usage:
//!   mkfifo [-m mode] name ...
//!
//! Creates FIFO (named pipe) files.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut mode: Option<u32> = None;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        if opt == "-m" && opt.len() == 2 {
            argv = &argv[1..];
            if argv.is_empty() { eprintln!("usage: mkfifo [-m mode] name ..."); std::process::exit(1); }
            mode = Some(u32::from_str_radix(argv[0].trim_start_matches('0'), 8)
                .unwrap_or_else(|_| { eprintln!("mkfifo: invalid mode: {}", argv[0]); std::process::exit(1); }));
        } else {
            eprintln!("usage: mkfifo [-m mode] name ...");
            std::process::exit(1);
        }
        argv = &argv[1..];
    }

    if argv.is_empty() {
        eprintln!("usage: mkfifo [-m mode] name ...");
        std::process::exit(1);
    }

    #[cfg(unix)]
    {
        use std::ffi::CString;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let final_mode = mode.unwrap_or(0o666);

        for name in argv {
            let c_str = CString::new(name.as_str()).unwrap_or_else(|_| {
                eprintln!("mkfifo: invalid filename: {name}");
                std::process::exit(1);
            });

            let result = unsafe { libc::mkfifo(c_str.as_ptr(), final_mode) };

            if result != 0 {
                eprintln!("mkfifo: {}: {}", name, std::io::Error::last_os_error());
                std::process::exit(1);
            }

            if mode.is_none() {
                if let Ok(meta) = fs::metadata(name) {
                    let _ = fs::set_permissions(name, fs::Permissions::from_mode(final_mode & !0o077));
                }
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = mode;
        eprintln!("mkfifo: not supported on this platform");
        std::process::exit(1);
    }
}
