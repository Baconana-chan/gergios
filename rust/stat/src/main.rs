use std::env;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::process;

extern crate libc;

/// Mode to string like "drwxr-xr-x"
fn mode_to_string(mode: u32) -> String {
    let file_type = match mode & libc::S_IFMT {
        libc::S_IFREG => '-',
        libc::S_IFDIR => 'd',
        libc::S_IFCHR => 'c',
        libc::S_IFBLK => 'b',
        libc::S_IFIFO => 'p',
        libc::S_IFLNK => 'l',
        libc::S_IFSOCK => 's',
        _ => '?',
    };

    let mut s = String::with_capacity(10);
    s.push(file_type);

    // User
    s.push(if mode & libc::S_IRUSR != 0 { 'r' } else { '-' });
    s.push(if mode & libc::S_IWUSR != 0 { 'w' } else { '-' });
    s.push(match (mode & libc::S_IXUSR, mode & libc::S_ISUID) {
        (0, 0) => '-',
        (_, 0) => 'x',
        (0, _) => 'S',
        (_, _) => 's',
    });

    // Group
    s.push(if mode & libc::S_IRGRP != 0 { 'r' } else { '-' });
    s.push(if mode & libc::S_IWGRP != 0 { 'w' } else { '-' });
    s.push(match (mode & libc::S_IXGRP, mode & libc::S_ISGID) {
        (0, 0) => '-',
        (_, 0) => 'x',
        (0, _) => 'S',
        (_, _) => 's',
    });

    // Other
    s.push(if mode & libc::S_IROTH != 0 { 'r' } else { '-' });
    s.push(if mode & libc::S_IWOTH != 0 { 'w' } else { '-' });
    s.push(match (mode & libc::S_IXOTH, mode & libc::S_ISVTX) {
        (0, 0) => '-',
        (_, 0) => 'x',
        (0, _) => 'T',
        (_, _) => 't',
    });

    s
}

/// Format a Unix timestamp
fn format_time(ts: i64) -> String {
    // Simplified: show as "Mon DD YYYY" or "Mon DD HH:MM" depending on age
    // For a proper implementation, we'd use localtime/strftime
    // For now, show the raw timestamp
    format!("{}", ts)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut fmt_flag = false;
    let mut fmt_str = String::new();
    let mut files: Vec<String> = Vec::new();
    let mut show_links = false;
    let mut _terse = false;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];
        if arg == "--" {
            i += 1;
            break;
        }
        if arg.starts_with('-') && arg.len() > 1 {
            let chars: Vec<char> = arg.chars().collect();
            for j in 1..chars.len() {
                match chars[j] {
                    'f' => {
                        fmt_flag = true;
                        if j + 1 < chars.len() {
                            fmt_str = arg[j + 1..].to_string();
                            break;
                        } else if i + 1 < args.len() {
                            i += 1;
                            fmt_str = args[i].clone();
                        }
                        break;
                    }
                    'L' => show_links = true,
                    't' => _terse = true,
                    'x' => {} // Linux-style format
                    _ => {
                        eprintln!("stat: unknown option -- {}", chars[j]);
                        process::exit(1);
                    }
                }
            }
        } else if !arg.starts_with('-') {
            files.push(arg.clone());
        }
        i += 1;
    }
    while i < args.len() {
        files.push(args[i].clone());
        i += 1;
    }

    if files.is_empty() {
        eprintln!("stat: missing operand");
        process::exit(1);
    }

    for file in &files {
        let path = Path::new(file);
        let meta = if show_links {
            fs::symlink_metadata(path)
        } else {
            fs::metadata(path)
        };

        match meta {
            Ok(m) => {
                if fmt_flag && !fmt_str.is_empty() {
                    // Custom format
                    println!("{} {}", file, format_custom(&fmt_str, &m, path));
                } else {
                    // Default format: like `stat` on BSD
                    let mode = m.mode();
                    let ino = m.ino();
                    let dev = m.dev();
                    let nlink = m.nlink();
                    let uid = m.uid();
                    let gid = m.gid();
                    let size = m.len();
                    let blksize = m.blksize();
                    let blocks = m.blocks();
                    let atime = m.atime();
                    let mtime = m.mtime();
                    let ctime = m.ctime();

                    let file_type = if m.is_dir() { "directory" }
                        else if m.is_file() { "regular file" }
                        else if m.is_symlink() { "symbolic link" }
                        else { "special file" };

                    println!("{}: {}", file, file_type);
                    println!("  Device: {}   Inode: {}   Links: {}", dev, ino, nlink);
                    println!("  Access: ({:04o} {})  UID: {}  GID: {}",
                             mode & 0o7777, mode_to_string(mode), uid, gid);
                    println!("  Size: {}    Blocks: {}    IO Block: {}    {}", size, blocks, blksize,
                             if m.is_dir() { "directory" } else { "regular" });
                    println!("  Access: {}", format_time(atime));
                    println!("  Modify: {}", format_time(mtime));
                    println!("  Change: {}", format_time(ctime));
                }
            }
            Err(e) => {
                eprintln!("stat: {}: {}", file, e);
                process::exit(1);
            }
        }
    }
}

fn format_custom(_fmt: &str, _meta: &fs::Metadata, _path: &Path) -> String {
    // Simplified: doesn't implement full format string parsing
    format!("{:#?}", _meta)
}
