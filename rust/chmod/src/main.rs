//! Rust port of the MINIX/NetBSD `chmod` utility.
//!
//! Usage:
//!   chmod [-R] mode file ...
//!
//! Changes the mode (permissions) of files. -R for recursive.
//! Supports both numeric (755) and symbolic (u+w, +x, a=r) modes.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut recursive = false;

    // Parse options — stop at first non-option arg (the mode)
    while !argv.is_empty() {
        let arg = &argv[0];
        if arg == "--" {
            argv = &argv[1..];
            break;
        }
        if arg.starts_with('-') && arg.len() > 1 {
            let mut is_mode = false;
            for c in arg.chars().skip(1) {
                match c {
                    'R' => recursive = true,
                    'r' | 'w' | 'x' | 'X' | 's' | 't' => { is_mode = true; }
                    'u' | 'g' | 'o' | 'a' | '+' | '-' | '=' => { is_mode = true; }
                    _ => {
                        eprintln!("chmod: unknown option -- {c}");
                        std::process::exit(1);
                    }
                }
            }
            if is_mode { break; }
        } else {
            break;
        }
        argv = &argv[1..];
    }

    if argv.len() < 2 {
        eprintln!("usage: chmod [-R] mode file ...");
        std::process::exit(1);
    }

    let mode_str = argv[0].to_string();
    let files = &argv[1..];

    // Get current umask for symbolic modes that don't specify WHO
    let umask = unsafe { libc::umask(0o022) };
    unsafe { libc::umask(umask); }

    let mode = match parse_mode(&mode_str, umask) {
        Some(m) => m,
        None => {
            eprintln!("chmod: invalid mode: `{mode_str}'");
            std::process::exit(1);
        }
    };

    let mut had_error = false;
    for file in files {
        if let Err(e) = set_mode(file, mode, recursive) {
            eprintln!("chmod: {file}: {e}");
            had_error = true;
        }
    }

    if had_error {
        std::process::exit(1);
    }
}

fn parse_mode(s: &str, _umask: u16) -> Option<u32> {
    // Try numeric (octal) mode first
    if let Ok(oct) = u32::from_str_radix(s, 8) {
        return Some(oct & 0o7777);
    }

    // Symbolic mode: [who...][[+|-|=][perms...]][,...]
    let mut result: Option<u32> = None;

    for clause in s.split(',') {
        let clause = clause.trim();
        if clause.is_empty() { continue; }

        // Parse WHO
        let mut who: u32 = 0;
        let mut idx = 0;
        let chars: Vec<char> = clause.chars().collect();

        while idx < chars.len() {
            match chars[idx] {
                'u' => { who |= 0o700; idx += 1; }  // S_IRWXU: user rwx
                'g' => { who |= 0o070; idx += 1; }  // S_IRWXG: group rwx
                'o' => { who |= 0o007; idx += 1; }  // S_IRWXO: other rwx
                'a' => { who |= 0o777; idx += 1; }  // all rwx
                _ => break,
            }
        }

        if who == 0 { who = 0o7777; } // Default to 'a'

        // Parse operator
        if idx >= chars.len() { return None; }
        let op = chars[idx];
        idx += 1;

        // Parse permissions
        let mut add: u32 = 0;
        let mut remove: u32 = 0;

        while idx < chars.len() {
            match chars[idx] {
                'r' => { add |= 0o444; idx += 1; }
                'w' => { add |= 0o222; idx += 1; }
                'x' => { add |= 0o111; idx += 1; }
                's' => { add |= 0o4000 | 0o2000; idx += 1; }
                't' => { add |= 0o1000; idx += 1; }
                'X' | 'u' | 'g' | 'o' => { idx += 1; } // Silently skip complex cases
                _ => break,
            }
        }

        // Apply to who filter
        let add_applied = add & who;
        let remove_applied = remove & who;

        match op {
            '+' => { result = Some((result.unwrap_or(0) | add_applied) & !remove_applied); }
            '-' => { result = Some(result.unwrap_or(0) & !(add_applied | remove_applied)); }
            '=' => {
                // Set exact permissions: clear all bits for WHO, then set ADD
                let cleared = result.unwrap_or(0) & !who;
                result = Some(cleared | add_applied);
            }
            _ => return None,
        }
    }

    result
}

fn set_mode(path: &str, mode: u32, recursive: bool) -> io::Result<()> {
    let meta = fs::metadata(path)?;
    let perms = fs::Permissions::from_mode(mode);
    fs::set_permissions(path, perms)?;

    if recursive && meta.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                set_mode(entry_path.to_str().unwrap_or(""), mode, true)?;
            } else {
                fs::set_permissions(&entry_path, fs::Permissions::from_mode(mode))?;
            }
        }
    }
    Ok(())
}
