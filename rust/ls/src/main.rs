//! Rust port of the MINIX/NetBSD `ls` utility.
//!
//! Usage:
//!   ls [-1aCRl] [file ...]
//!
//! Lists directory contents.

use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut all = false;
    let mut long = false;
    let mut recursive = false;
    let mut one_per_line = false;
    let mut dirs_only = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        for c in argv[0].chars().skip(1) {
            match c {
                'a' => all = true,
                'l' => long = true,
                'R' => recursive = true,
                '1' => one_per_line = true,
                'd' => dirs_only = true,
                _ => {
                    eprintln!("ls: unknown option -- {c}");
                    std::process::exit(1);
                }
            }
        }
        argv = &argv[1..];
    }

    let targets: Vec<&str> = if argv.is_empty() {
        vec!["."]
    } else {
        argv.iter().map(|s| s.as_str()).collect()
    };

    let mut had_error = false;
    let multiple = targets.len() > 1;

    for (idx, target) in targets.iter().enumerate() {
        let path = Path::new(target);
        let meta = match fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("ls: cannot access '{target}': {e}");
                had_error = true;
                continue;
            }
        };

        if meta.is_dir() && !dirs_only {
            if multiple { println!("{}:", target); }
            match list_dir(target, all, long, recursive, "") {
                Ok(()) => {},
                Err(e) => { eprintln!("ls: {target}: {e}"); had_error = true; }
            }
            if multiple && idx < targets.len() - 1 { println!(); }
        } else {
            print_entry(Path::new(target), &meta, long, "");
        }
    }

    if had_error { std::process::exit(1); }
}

fn list_dir(dir: &str, all: bool, long: bool, recursive: bool, prefix: &str) -> io::Result<()> {
    let entries: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .collect();

    let mut names: Vec<String> = entries.iter()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| all || !n.starts_with('.'))
        .collect();
    names.sort();

    if names.is_empty() {
        return Ok(());
    }

    let stdout = io::stdout();
    let mut out = stdout.lock();

    if long {
        // Calculate column widths
        let max_links = names.iter().map(|n| {
            let p = Path::new(dir).join(n);
            fs::metadata(&p).map(|m| m.nlink()).unwrap_or(0)
        }).max().unwrap_or(0);
        let max_size = names.iter().map(|n| {
            let p = Path::new(dir).join(n);
            fs::metadata(&p).map(|m| m.len()).unwrap_or(0)
        }).max().unwrap_or(0);

        for name in &names {
            let p = Path::new(dir).join(name);
            if let Ok(meta) = fs::metadata(&p) {
                let mode = format_mode(meta.mode());
                let nlink = meta.nlink();
                let size = meta.len();
                if let Ok(modified) = meta.modified() {
                    let secs = modified.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default().as_secs();
                    let time_str = format_timestamp(secs);
                    let _ = writeln!(out, "{mode} {nlink:>2} {size:>4} {time_str} {name}");
                } else {
                    let _ = writeln!(out, "{mode} {nlink:>2} {size:>4}             {name}");
                }
            }
        }
    } else {
        // Column or one-per-line output
        if one_per_line {
            for name in &names {
                let _ = writeln!(out, "{name}");
            }
        } else {
            // Simple column output
            let cols = 80 / (names.iter().map(|n| n.len()).max().unwrap_or(8) + 2).max(8);
            let cols = cols.max(1);
            for (i, name) in names.iter().enumerate() {
                let _ = write!(out, "{name:<19}");
                if (i + 1) % cols == 0 || i == names.len() - 1 {
                    let _ = writeln!(out);
                }
            }
        }
    }

    // Recursive
    if recursive {
        for name in &names {
            let p = Path::new(dir).join(name);
            if p.is_dir() {
                let sub = format!("{}/{}", prefix, name);
                let _ = writeln!(io::stdout(), "\n{}:", p.to_string_lossy());
                let _ = list_dir(&p.to_string_lossy(), all, long, true, &sub);
            }
        }
    }

    Ok(())
}

fn print_entry(path: &Path, meta: &fs::Metadata, long: bool, _prefix: &str) {
    let name = path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string());

    if long {
        let mode = format_mode(meta.mode());
        let nlink = meta.nlink();
        let size = meta.len();
        if let Ok(modified) = meta.modified() {
            let secs = modified.duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default().as_secs();
            let time_str = format_timestamp(secs);
            println!("{mode} {nlink:>2} {size:>4} {time_str} {name}");
        }
    } else {
        println!("{name}");
    }
}

fn format_mode(mode: u32) -> String {
    let file_type = match mode & libc::S_IFMT {
        libc::S_IFDIR => 'd',
        libc::S_IFCHR => 'c',
        libc::S_IFBLK => 'b',
        libc::S_IFIFO => 'p',
        libc::S_IFLNK => 'l',
        libc::S_IFSOCK => 's',
        _ => '-',
    };
    let user = format_triplet(mode, 6);
    let group = format_triplet(mode, 3);
    let other = format_triplet(mode, 0);
    format!("{file_type}{user}{group}{other}")
}

fn format_triplet(mode: u32, shift: u32) -> String {
    let r = if mode & (4 << shift) != 0 { 'r' } else { '-' };
    let w = if mode & (2 << shift) != 0 { 'w' } else { '-' };
    let x = if mode & (1 << shift) != 0 {
        if shift == 6 && mode & libc::S_ISUID != 0 { 's' }
        else if shift == 3 && mode & libc::S_ISGID != 0 { 's' }
        else if shift == 0 && mode & libc::S_ISVTX != 0 { 't' }
        else { 'x' }
    } else {
        if shift == 6 && mode & libc::S_ISUID != 0 { 'S' }
        else if shift == 3 && mode & libc::S_ISGID != 0 { 'S' }
        else if shift == 0 && mode & libc::S_ISVTX != 0 { 'T' }
        else { '-' }
    };
    format!("{r}{w}{x}")
}

fn format_timestamp(secs: u64) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();

    let dt = chrono::DateTime::from_timestamp(secs as i64, 0)
        .unwrap_or_default();

    if now - secs > 365 * 86400 / 2 || secs > now {
        // Older than 6 months or in future: show year
        dt.format("%b %e  %Y").to_string()
    } else {
        dt.format("%b %e %H:%M").to_string()
    }
}
