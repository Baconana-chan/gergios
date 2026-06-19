use std::env;
use std::process;
use std::time::Instant;

extern crate libc;

/// Format a duration in seconds with microseconds precision
fn format_duration(secs: f64) -> String {
    if secs >= 3600.0 {
        format!("{:6.2}s", secs)
    } else if secs >= 60.0 {
        format!("{:6.2}s", secs)
    } else if secs >= 1.0 {
        format!("{:6.3}s", secs)
    } else if secs >= 0.001 {
        format!("{:6.1f}ms", secs * 1000.0)
    } else {
        format!("{:6.1f}us", secs * 1_000_000.0)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("time: missing command");
        process::exit(1);
    }

    let command = &args[1];
    let cmd_args: Vec<&str> = args[2..].iter().map(|s| s.as_str()).collect();

    let start = Instant::now();

    // Get resource usage before
    // In POSIX, time uses getrusage(RUSAGE_CHILDREN) to get user/sys time
    // For simplicity, we'll measure wall clock only using Instant.

    let mut child = match std::process::Command::new(command)
        .args(&cmd_args)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("time: cannot execute '{}': {}", command, e);
            process::exit(126);
        }
    };

    let status = match child.wait() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("time: failed to wait for command: {}", e);
            process::exit(127);
        }
    };

    let elapsed = start.elapsed();
    let real_secs = elapsed.as_secs_f64();

    // Try to get resource usage for user/sys time
    // On POSIX, we'd use wait3 or wait4 with rusage, or getrusage(RUSAGE_CHILDREN)
    let (user_secs, sys_secs) = get_child_rusage();

    // Output format: like bash's time builtin
    eprintln!(
        "real\t{}\nuser\t{}\nsys\t{}",
        format_duration(real_secs),
        format_duration(user_secs),
        format_duration(sys_secs),
    );

    if let Some(code) = status.code() {
        process::exit(code);
    }
}

fn get_child_rusage() -> (f64, f64) {
    #[cfg(unix)]
    {
        let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
        let ret = unsafe { libc::getrusage(libc::RUSAGE_CHILDREN, &mut usage) };
        if ret == 0 {
            let user = usage.ru_utime.tv_sec as f64 + usage.ru_utime.tv_usec as f64 / 1_000_000.0;
            let sys = usage.ru_stime.tv_sec as f64 + usage.ru_stime.tv_usec as f64 / 1_000_000.0;
            return (user, sys);
        }
    }
    (0.0, 0.0)
}
