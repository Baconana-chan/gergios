//! Rust port of the NetBSD `xinstall` utility.
//!
//! Usage:
//!   xinstall [-bCcMpSsv] [-B suffix] [-f flags] [-g group] [-m mode]
//!            [-o owner] [-s stripcmd] source ... target
//!
//! Installs files with proper permissions, ownership, and stripping.

use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut mode: u32 = 0o755;
    let mut owner: Option<String> = None;
    let mut group: Option<String> = None;
    let mut strip = false;
    let mut backup = false;
    let mut backup_suffix = ".bak".to_string();
    let mut preserve = false;
    let mut strip_cmd = "strip".to_string();

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        match opt.as_str() {
            "-m" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: xinstall ..."); std::process::exit(1); }
                mode = u32::from_str_radix(argv[0].trim_start_matches('0'), 8)
                    .unwrap_or_else(|_| { eprintln!("xinstall: invalid mode: {}", argv[0]); std::process::exit(1); });
            }
            "-o" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: xinstall ..."); std::process::exit(1); }
                owner = Some(argv[0].clone());
            }
            "-g" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: xinstall ..."); std::process::exit(1); }
                group = Some(argv[0].clone());
            }
            "-s" => strip = true,
            "-S" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: xinstall ..."); std::process::exit(1); }
                strip_cmd = argv[0].clone();
            }
            "-B" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: xinstall ..."); std::process::exit(1); }
                backup_suffix = argv[0].clone();
            }
            "-b" => backup = true,
            "-c" => {},  // copy (default)
            "-p" => preserve = true,
            "-v" => {},  // verbose
            "-M" | "-C" | "-S" => {},
            _ => { eprintln!("usage: xinstall [-bCcMpSsv] [-B suffix] [-f flags] [-g group] [-m mode] [-o owner] [-s stripcmd] source ... target"); std::process::exit(1); }
        }
        argv = &argv[1..];
    }

    if argv.len() < 2 {
        eprintln!("usage: xinstall source ... target");
        std::process::exit(1);
    }

    let target = argv.last().unwrap();
    let sources: Vec<&str> = argv[..argv.len()-1].iter().map(|s| s.as_str()).collect();
    let is_dir = Path::new(target).is_dir() || sources.len() > 1;

    for src in sources {
        let dest = if is_dir {
            let fname = Path::new(src).file_name().unwrap();
            Path::new(target).join(fname)
        } else {
            Path::new(target).to_path_buf()
        };

        // Read source file
        let mut data = Vec::new();
        match fs::File::open(src) {
            Ok(mut f) => { f.read_to_end(&mut data).ok(); }
            Err(e) => { eprintln!("xinstall: {src}: {e}"); std::process::exit(1); }
        }

        // Backup existing file
        if backup && dest.exists() {
            let backup_name = format!("{}{}", dest.display(), backup_suffix);
            let _ = fs::rename(&dest, &backup_name);
        }

        // Create parent directories if needed
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }

        // Write destination file
        let mut out = match fs::File::create(&dest) {
            Ok(f) => f,
            Err(e) => { eprintln!("xinstall: {}: {e}", dest.display()); std::process::exit(1); }
        };
        out.write_all(&data).ok();
        drop(out);

        // Set permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&dest, fs::Permissions::from_mode(mode));
        }

        // Set ownership (requires privileges)
        #[cfg(unix)]
        {
            if let Some(ref o) = owner {
                let c_owner = std::ffi::CString::new(o.as_str()).unwrap();
                let pw = unsafe { libc::getpwnam(c_owner.as_ptr()) };
                if !pw.is_null() {
                    let uid = unsafe { (*pw).pw_uid };
                    let gid = if let Some(ref g) = group {
                        let c_group = std::ffi::CString::new(g.as_str()).unwrap();
                        let gr = unsafe { libc::getgrnam(c_group.as_ptr()) };
                        if !gr.is_null() { unsafe { (*gr).gr_gid } } else { !0 }
                    } else { !0 };
                    unsafe { libc::chown(dest.to_str().unwrap().as_ptr() as *const i8, uid, gid); }
                }
            } else if let Some(ref g) = group {
                let c_group = std::ffi::CString::new(g.as_str()).unwrap();
                let gr = unsafe { libc::getgrnam(c_group.as_ptr()) };
                if !gr.is_null() {
                    let gid = unsafe { (*gr).gr_gid };
                    unsafe { libc::chown(dest.to_str().unwrap().as_ptr() as *const i8, !0, gid); }
                }
            }
        }

        // Strip
        if strip {
            let _ = Command::new(&strip_cmd).arg(&dest).status();
        }
    }
}
