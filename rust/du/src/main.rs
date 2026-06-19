use std::env;
use std::fs;
use std::io;
use std::path::Path;
use std::process;

/// Human-readable size
fn human_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["", "K", "M", "G", "T", "P"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;
    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }
    if unit_idx == 0 {
        format!("{}", bytes)
    } else {
        format!("{:.1}{}", size, UNITS[unit_idx])
    }
}

/// Recursively compute disk usage for a path.
/// Returns (size_in_bytes, file_count, dir_count).
fn du_path(path: &Path, all: bool, apparent: bool) -> io::Result<(u64, u64, u64)> {
    let meta = fs::symlink_metadata(path)?;

    // Determine size of this entry
    let size = if apparent {
        meta.len()
    } else {
        // Disk usage is rounded to block size (512 bytes for POSIX)
        let blocks = meta.len() / 512 + if meta.len() % 512 != 0 { 1 } else { 0 };
        blocks * 512
    };

    if meta.is_dir() {
        let mut total_size = if apparent { 0 } else { 512 }; // directory itself takes at least 1 block
        let mut file_count = 0u64;
        let mut dir_count = 1u64;

        match fs::read_dir(path) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(e) => {
                            match du_path(&e.path(), all, apparent) {
                                Ok((s, f, d)) => {
                                    total_size += s;
                                    file_count += f;
                                    dir_count += d;
                                }
                                Err(_) => {
                                    // Skip inaccessible entries
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
            Err(_) => {}
        }

        Ok((total_size, file_count, dir_count))
    } else if meta.is_file() || meta.is_symlink() {
        Ok((size, 1, 0))
    } else {
        Ok((0, 0, 0))
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut human = false;
    let mut summary = false;
    let mut all = false;
    let mut bytes = false;
    let mut si = false;
    let mut paths: Vec<String> = Vec::new();

    for arg in &args[1..] {
        if arg == "-h" {
            human = true;
        } else if arg == "-s" {
            summary = true;
        } else if arg == "-a" {
            all = true;
        } else if arg == "-b" {
            bytes = true;
        } else if arg == "--si" {
            si = true;
        } else if !arg.starts_with('-') {
            paths.push(arg.clone());
        }
    }

    if paths.is_empty() {
        paths.push(".".to_string());
    }

    // Apparent size if -b flag
    let apparent = bytes;

    for p in &paths {
        let path = Path::new(p);
        match du_path(path, all, apparent) {
            Ok((size, _files, _dirs)) => {
                if human {
                    println!("{}\t{}", human_size(size), p);
                } else {
                    // POSIX: 512-byte blocks
                    let blocks = (size + 256) / 512;
                    println!("{}\t{}", blocks, p);
                }
            }
            Err(e) => {
                eprintln!("du: {}: {}", p, e);
                process::exit(1);
            }
        }
    }
}
