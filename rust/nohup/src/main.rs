use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::{self, Command, Stdio};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("nohup: missing operand");
        process::exit(1);
    }

    // Find where options end and command begins
    let mut cmd_start = 1;
    while cmd_start < args.len() && args[cmd_start].starts_with('-') {
        cmd_start += 1;
    }
    if cmd_start >= args.len() {
        eprintln!("nohup: missing command");
        process::exit(1);
    }

    let command = &args[cmd_start];
    let cmd_args: Vec<&str> = args[cmd_start + 1..].iter().map(|s| s.as_str()).collect();

    // POSIX: ignore SIGHUP
    #[cfg(unix)]
    {
        unsafe {
            libc::signal(libc::SIGHUP, libc::SIG_IGN);
        }
    }

    // Find an appropriate nohup.out file and open it
    let nohup_path = find_nohup_out();
    let stdout_file: Option<File> = if let Some(ref path) = nohup_path {
        match OpenOptions::new().create(true).append(true).open(Path::new(path)) {
            Ok(f) => {
                // Announce to stderr if we weren't able to announce earlier
                eprintln!("nohup: appending output to '{}'", path);
                Some(f)
            }
            Err(_) => None,
        }
    } else {
        // Last resort: try $HOME/nohup.out
        let home_path = env::var("HOME").map(|h| Path::new(&h).join("nohup.out")).ok();
        if let Some(ref hp) = home_path {
            match OpenOptions::new().create(true).append(true).open(hp) {
                Ok(f) => {
                    eprintln!("nohup: appending output to '{}'", hp.display());
                    Some(f)
                }
                Err(_) => None,
            }
        } else {
            None
        }
    };

    let mut child = Command::new(command)
        .args(&cmd_args);

    // Redirect stdout
    if let Some(ref file) = stdout_file {
        // Need to duplicate the file handle since we can't move it
        // Use file.try_clone() for stderr if needed
        #[cfg(unix)]
        {
            use std::os::unix::io::FromRawFd;
            // Get the raw fd and duplicate it for stderr
            let fd = unsafe {
                use std::os::unix::io::AsRawFd;
                libc::dup(file.as_raw_fd())
            };
            if fd >= 0 {
                let stderr_file = unsafe { File::from_raw_fd(fd) };
                child.stdout(Stdio::from(
                    file.try_clone().unwrap_or_else(|_| File::create("/dev/null").unwrap())
                ));
                child.stderr(Stdio::from(stderr_file));
            } else {
                child.stdout(Stdio::from(
                    file.try_clone().unwrap_or_else(|_| File::create("/dev/null").unwrap())
                ));
            }
        }
        #[cfg(not(unix))]
        {
            child.stdout(Stdio::from(
                file.try_clone().unwrap_or_else(|_| File::create("nohup.out").unwrap())
            ));
        }
    } else {
        child.stdout(Stdio::inherit());
        child.stderr(Stdio::inherit());
    }

    let mut child = match child.spawn() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nohup: failed to run command '{}': {}", command, e);
            process::exit(126);
        }
    };

    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("nohup: failed to wait for command: {}", e);
            process::exit(127);
        }
    };

    if let Some(code) = status.code() {
        process::exit(code);
    }
}

/// Find an appropriate nohup.out file path (without opening it).
/// Try current directory first, then $HOME.
fn find_nohup_out() -> Option<String> {
    // Try current directory
    if let Ok(cwd) = env::current_dir() {
        let path = cwd.join("nohup.out");
        let path_str = path.to_string_lossy().to_string();
        // Check if we can create or append to it
        if path.exists() {
            if let Ok(meta) = fs::metadata(&path) {
                if meta.is_file() {
                    return Some(path_str);
                }
            }
        } else {
            // Test writeability by opening
            match OpenOptions::new()
                .create(true)
                .write(true)
                .open(&path)
            {
                Ok(f) => {
                    drop(f);
                    return Some(path_str);
                }
                Err(_) => {}
            }
        }
    }

    // Try $HOME/nohup.out
    if let Ok(home) = env::var("HOME") {
        let path = Path::new(&home).join("nohup.out");
        let path_str = path.to_string_lossy().to_string();
        match OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(&path)
        {
            Ok(f) => {
                drop(f);
                return Some(path_str);
            }
            Err(_) => {}
        }
    }

    None
}
