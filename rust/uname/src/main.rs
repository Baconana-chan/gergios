//! Rust port of the MINIX/NetBSD `uname` utility.
//!
//! Usage:
//!   uname [-amnprsv]
//!
//! Print system information. Default is -s.

use std::io::{self, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut opts = [false; 6]; // a, m, n, p, r, s, v
    let want_all = args.iter().any(|a| a == "-a");
    let has_opt = args.iter().skip(1).any(|a| a.starts_with('-'));

    for arg in &args[1..] {
        if arg.starts_with('-') && arg != "--" {
            for c in arg.chars().skip(1) {
                match c {
                    'a' => opts = [true; 6],
                    'm' => opts[0] = true,
                    'n' => opts[1] = true,
                    'r' => opts[2] = true,
                    'p' => opts[3] = false, // Not supported on MINIX (processor type)
                    's' => opts[4] = true,
                    'v' => opts[5] = true,
                    _ => {
                        eprintln!("uname: unknown option -- {c}");
                        std::process::exit(1);
                    }
                }
            }
        }
    }

    if !has_opt {
        opts[4] = true; // default: -s
    }
    if want_all {
        // Try to detect OS from environment or compile-time
        opts[3] = false;
        opts[0] = true;
        opts[1] = true;
        opts[2] = true;
        opts[4] = true;
        opts[5] = true;
    }

    // Build uname data from env-based detection and OS constants
    let sysname = option_env!("CARGO_CFG_TARGET_OS").unwrap_or("GergiOS");
    let nodename = std::env::var("HOSTNAME").unwrap_or_else(|_| {
        let mut buf = vec![0u8; 256];
        unsafe {
            libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len());
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        std::str::from_utf8(&buf[..len]).unwrap_or("").to_string()
    });
    let release = option_env!("CARGO_PKG_VERSION").unwrap_or("1.0.0");
    let version = concat!("GergiOS ", env!("CARGO_PKG_VERSION"));

    let stdout = io::stdout();
    let mut handle = stdout.lock();
    let parts: [&str; 6] = [
        if opts[0] { "x86_64" } else { "" },     // -m: machine
        if opts[1] { &nodename } else { "" },      // -n: nodename
        if opts[2] { release } else { "" },        // -r: release (os version)
        "",                                          // -p: processor (skipped)
        if opts[4] { sysname } else { "" },         // -s: sysname (OS name)
        if opts[5] { version } else { "" },         // -v: version
    ];
    let out: Vec<&str> = parts.iter().filter(|s| !s.is_empty()).copied().collect();
    let _ = writeln!(handle, "{}", out.join(" "));
}
