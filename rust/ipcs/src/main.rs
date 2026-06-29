//! Rust port of the MINIX/NetBSD `ipcs` utility.
//!
//! Usage:
//!   ipcs [-a] [-b] [-c] [-m] [-o] [-p] [-s] [-q] [-t] [-T]
//!
//! Shows IPC resources: shared memory, semaphores, message queues.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];
    let mut show_shm = false;
    let mut show_sem = false;
    let mut show_msg = false;
    let mut show_all = false;

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        for ch in opt.chars().skip(1) {
            match ch {
                'a' => show_all = true,
                'm' => show_shm = true,
                's' => show_sem = true,
                'q' => show_msg = true,
                'b' | 'c' | 'o' | 'p' | 't' | 'T' => {},
                _ => { eprintln!("usage: ipcs [-a] [-m] [-s] [-q]"); std::process::exit(1); }
            }
        }
        argv = &argv[1..];
    }

    if !show_shm && !show_sem && !show_msg && !show_all {
        show_all = true;
    }

    #[cfg(unix)]
    {
        // Read from /proc/sysvipc/ or use sysctl
        // Simplified: parse /proc/sysvipc/shm, /proc/sysvipc/sem, /proc/sysvipc/msg
        if show_shm || show_all {
            show_ipc("Shared Memory", "/proc/sysvipc/shm", "shmid", "owner", "perms", "bytes");
        }
        if show_sem || show_all {
            show_ipc("Semaphores", "/proc/sysvipc/sem", "semid", "owner", "perms", "nsems");
        }
        if show_msg || show_all {
            show_ipc("Message Queues", "/proc/sysvipc/msg", "msqid", "owner", "perms", "bytes");
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (show_shm, show_sem, show_msg, show_all);
        eprintln!("ipcs: not supported on this platform");
        std::process::exit(1);
    }
}

fn show_ipc(title: &str, path: &str, id_label: &str, owner_label: &str, perms_label: &str, size_label: &str) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return,
    };

    println!("\n{}:", title);
    println!("{:<10} {:<10} {:<10} {:<10}", id_label, owner_label, perms_label, size_label);
    println!("{}", "-".repeat(40));

    for line in content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 5 {
            println!("{:<10} {:<10} {:<10} {:<10}",
                parts[0], parts[1], parts[3], parts[4]);
        }
    }
}
