//! Rust port of the MINIX/NetBSD `hostname` utility.
//!
//! Usage:
//!   hostname [name]
//!
//! With no arguments, prints the current hostname.
//! With an argument, sets the hostname (requires root).

use std::io::{self, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() > 2 {
        eprintln!("usage: hostname [name]");
        std::process::exit(1);
    }

    if args.len() == 2 {
        // Set hostname
        let name = &args[1];
        let c_name = std::ffi::CString::new(name.as_str()).unwrap_or_else(|_| {
            eprintln!("hostname: invalid hostname");
            std::process::exit(1);
        });
        let ret = unsafe { libc::sethostname(c_name.as_ptr(), name.len()) };
        if ret != 0 {
            eprintln!("hostname: sethostname: {}", std::io::Error::last_os_error());
            std::process::exit(1);
        }
        return;
    }

    // Get hostname
    let mut buf = vec![0u8; 256];
    let ret = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if ret != 0 {
        eprintln!("hostname: gethostname: {}", std::io::Error::last_os_error());
        std::process::exit(1);
    }

    // Find null terminator
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    let hostname = std::str::from_utf8(&buf[..len]).unwrap_or("");

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let _ = writeln!(handle, "{hostname}");
}
