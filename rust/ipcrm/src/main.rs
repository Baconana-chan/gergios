//! Rust port of the MINIX/NetBSD `ipcrm` utility.
//!
//! Usage:
//!   ipcrm [-W] [-q msqid] [-m shmid] [-s semid] ...
//!   ipcrm [-M shmkey] [-Q msgkey] [-S semkey] ...
//!
//! Removes IPC resources (message queues, semaphores, shared memory).

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut argv = &args[1..];

    let mut shm_ids: Vec<i32> = Vec::new();
    let mut sem_ids: Vec<i32> = Vec::new();
    let mut msg_ids: Vec<i32> = Vec::new();

    while !argv.is_empty() && argv[0].starts_with('-') && argv[0] != "--" {
        let opt = argv[0].clone();
        if opt == "--" { argv = &argv[1..]; break; }
        match opt.as_str() {
            "-m" | "-w" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: ipcrm [-m shmid] [-s semid] [-q msqid]"); std::process::exit(1); }
                if let Ok(id) = argv[0].parse() { shm_ids.push(id); }
            }
            "-s" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: ipcrm [-m shmid] [-s semid] [-q msqid]"); std::process::exit(1); }
                if let Ok(id) = argv[0].parse() { sem_ids.push(id); }
            }
            "-q" => {
                argv = &argv[1..];
                if argv.is_empty() { eprintln!("usage: ipcrm [-m shmid] [-s semid] [-q msqid]"); std::process::exit(1); }
                if let Ok(id) = argv[0].parse() { msg_ids.push(id); }
            }
            "-M" | "-Q" | "-S" | "-W" => {
                argv = &argv[1..]; // skip key args (simplified)
                if argv.is_empty() { eprintln!("usage: ipcrm ..."); std::process::exit(1); }
            }
            _ => { eprintln!("usage: ipcrm [-m shmid] [-s semid] [-q msqid]"); std::process::exit(1); }
        }
        argv = &argv[1..];
    }

    #[cfg(unix)]
    {
        for &id in &shm_ids {
            if unsafe { libc::shmctl(id, libc::IPC_RMID, std::ptr::null_mut()) } != 0 {
                eprintln!("ipcrm: shm {}: {}", id, std::io::Error::last_os_error());
            }
        }
        for &id in &sem_ids {
            if unsafe { libc::semctl(id, 0, libc::IPC_RMID) } != 0 {
                eprintln!("ipcrm: sem {}: {}", id, std::io::Error::last_os_error());
            }
        }
        for &id in &msg_ids {
            if unsafe { libc::msgctl(id, libc::IPC_RMID, std::ptr::null_mut()) } != 0 {
                eprintln!("ipcrm: msg {}: {}", id, std::io::Error::last_os_error());
            }
        }
    }

    #[cfg(not(unix))]
    {
        let _ = (shm_ids, sem_ids, msg_ids);
        eprintln!("ipcrm: not supported on this platform");
        std::process::exit(1);
    }
}
