//! Rust port of the MINIX/NetBSD `mesg` utility.
//!
//! Usage:
//!   mesg [y | n]
//!
//! Controls write access to the terminal (chmod of tty device).

use std::fs;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let tty_path = resolve_tty();

    if args.len() < 2 {
        // Query mode
        match fs::metadata(&tty_path) {
            Ok(_meta) => {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = meta.permissions().mode();
                    if mode & 0o002 != 0 {
                        println!("is y");
                    } else {
                        println!("is n");
                    }
                }
                #[cfg(not(unix))]
                {
                    println!("is y");
                }
            }
            Err(_) => {
                println!("is n");
            }
        }
    } else {
        match args[1].as_str() {
            "y" => set_write_mode(&tty_path, true),
            "n" => set_write_mode(&tty_path, false),
            _ => {
                eprintln!("usage: mesg [y | n]");
                std::process::exit(1);
            }
        }
    }
}

fn resolve_tty() -> String {
    if let Ok(tty) = std::env::var("TTY") {
        return tty;
    }
    // Try standard tty paths
    for p in &["/dev/tty", "/dev/console"] {
        if Path::new(p).exists() {
            return p.to_string();
        }
    }
    String::new()
}

fn set_write_mode(path: &str, allow: bool) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = fs::metadata(path) {
            let mut perms = meta.permissions();
            let mut mode = perms.mode();
            if allow {
                mode |= 0o002;
            } else {
                mode &= !0o002;
            }
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, allow);
    }
}
