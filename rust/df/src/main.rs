use std::env;
use std::path::Path;
use std::process;

extern crate libc;

/// Call statvfs on the given path and return (block_size, blocks_total, blocks_free, blocks_avail, files, files_free, f_namemax)
/// Returns None if statvfs fails.
fn get_statvfs(path: &str) -> Option<(u64, u64, u64, u64, u64, u64, u64)> {
    let cpath = std::ffi::CString::new(path).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::statvfs(cpath.as_ptr(), &mut stat) };
    if ret != 0 {
        return None;
    }
    Some((
        stat.f_frsize as u64,   // Fundamental file system block size
        stat.f_blocks as u64,   // Total blocks
        stat.f_bfree as u64,    // Free blocks
        stat.f_bavail as u64,   // Available blocks (non-root)
        stat.f_files as u64,    // Total inodes
        stat.f_ffree as u64,    // Free inodes
        stat.f_namemax as u64,  // Max filename length
    ))
}

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

/// One-block size (512 bytes for POSIX df)
fn block_size_512(bytes: u64) -> u64 {
    (bytes + 256) / 512
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut human = false;
    let mut _kilobytes = false;
    let mut files: Vec<String> = Vec::new();

    for arg in &args[1..] {
        if arg == "-h" {
            human = true;
        } else if arg == "-k" {
            _kilobytes = true;
        } else if !arg.starts_with('-') {
            files.push(arg.clone());
        }
    }

    if files.is_empty() {
        files.push(".".to_string());
    }

    for file in &files {
        let display_path = if file == "." {
            std::env::current_dir().map(|p| p.display().to_string()).unwrap_or_else(|_| ".".to_string())
        } else {
            file.clone()
        };

        match get_statvfs(file) {
            Some((bsize, total, free, avail, _files_cnt, _files_free, _namemax)) => {
                let fragment_size = bsize;
                let total_blocks = total;
                let free_blocks = free;
                let avail_blocks = avail;
                let used_blocks = total_blocks.saturating_sub(free_blocks);

                let total_bytes = total_blocks * fragment_size;
                let used_bytes = used_blocks * fragment_size;
                let avail_bytes = avail_blocks * fragment_size;

                if human {
                    let capacity = if total_bytes > 0 {
                        ((used_bytes as f64 / total_bytes as f64) * 100.0) as u64
                    } else {
                        0
                    };
                    println!(
                        "{:<14} {:>5} {:>5} {:>5} {:>3}%    {}",
                        "filesystem",
                        human_size(total_bytes),
                        human_size(used_bytes),
                        human_size(avail_bytes),
                        capacity,
                        display_path,
                    );
                } else {
                    let total_512 = block_size_512(total_bytes);
                    let used_512 = block_size_512(used_bytes);
                    let avail_512 = block_size_512(avail_bytes);
                    let capacity = if total_512 > 0 {
                        ((used_512 as f64 / total_512 as f64) * 100.0) as u64
                    } else {
                        0
                    };
                    println!(
                        "{:<14} {:>10} {:>10} {:>10} {:>3}%    {}",
                        "filesystem",
                        total_512,
                        used_512,
                        avail_512,
                        capacity,
                        display_path,
                    );
                }
            }
            None => {
                eprintln!("df: {}: No such file or directory", file);
                process::exit(1);
            }
        }
    }
}
