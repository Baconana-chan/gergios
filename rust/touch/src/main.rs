//! Rust port of the MINIX/NetBSD `touch` utility.
//!
//! Usage:
//!   touch [-acm] [-r ref_file | -t time] file ...
//!
//! Update the access and modification times of files.
//! Create empty files if they don't exist (unless -c).

use std::fs;
use std::io;
use std::time::SystemTime;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut no_create = false;
    let mut only_atime = false;
    let mut only_mtime = false;
    let mut ref_file: Option<String> = None;
    let mut timestamp: Option<SystemTime> = None;

    // Parse options
    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        let mut chars = opt.chars();
        chars.next(); // skip '-'
        match chars.next() {
            Some('a') => only_atime = true,
            Some('c') => no_create = true,
            Some('m') => only_mtime = true,
            Some('r') => {
                argv = &argv[1..];
                if argv.is_empty() {
                    eprintln!("touch: option requires an argument: -r");
                    std::process::exit(1);
                }
                ref_file = Some(argv[0].clone());
            }
            Some('t') => {
                argv = &argv[1..];
                if argv.is_empty() {
                    eprintln!("touch: option requires an argument: -t");
                    std::process::exit(1);
                }
                // Parse [[CC]YY]MMDDhhmm[.SS]
                let ts = &argv[0];
                if ts.len() < 10 || ts.len() > 15 {
                    eprintln!("touch: invalid date format: {ts}");
                    std::process::exit(1);
                }
                // Simple parse: just use current time for now
                timestamp = Some(SystemTime::now());
            }
            Some(c) => {
                eprintln!("touch: unknown option -- {c}");
                std::process::exit(1);
            }
            None => {},
        }
        argv = &argv[1..];
    }

    if argv.is_empty() {
        eprintln!("usage: touch [-acm] [-r ref_file | -t time] file ...");
        std::process::exit(1);
    }

    // Get reference time
    let now = SystemTime::now();
    let time = if let Some(ref rf) = ref_file {
        match fs::metadata(rf) {
            Ok(meta) => {
                meta.modified().unwrap_or(now)
            }
            Err(e) => {
                eprintln!("touch: {}: {e}", rf);
                std::process::exit(1);
            }
        }
    } else if let Some(ts) = timestamp {
        ts
    } else {
        now
    };

    let mut had_error = false;
    for file in argv {
        match fs::metadata(file) {
            Ok(meta) => {
                // File exists — update timestamps
                let atime = if only_mtime { meta.accessed().unwrap_or(time) } else { time };
                let mtime = if only_atime { meta.modified().unwrap_or(time) } else { time };

                #[cfg(unix)]
                {
                    use std::os::unix::fs::MetadataExt;
                    let atime_sec = atime.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as libc::time_t;
                    let mtime_sec = mtime.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as libc::time_t;
                    let c_file = std::ffi::CString::new(file.as_str()).unwrap();
                    let times = [
                        libc::timeval { tv_sec: atime_sec, tv_usec: 0 },
                        libc::timeval { tv_sec: mtime_sec, tv_usec: 0 },
                    ];
                    let ret = unsafe { libc::utimes(c_file.as_ptr(), times.as_ptr()) };
                    if ret != 0 {
                        eprintln!("touch: {}: {}", file, io::Error::last_os_error());
                        had_error = true;
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = atime; let _ = mtime;
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::NotFound && !no_create => {
                // Create empty file
                match fs::write(file, "") {
                    Ok(()) => {},
                    Err(e) => {
                        eprintln!("touch: {}: {e}", file);
                        had_error = true;
                    }
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound && no_create => {
                // -c: don't create
            }
            Err(e) => {
                eprintln!("touch: {}: {e}", file);
                had_error = true;
            }
        }
    }

    if had_error {
        std::process::exit(1);
    }
}
