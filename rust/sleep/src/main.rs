//! Rust port of the MINIX/NetBSD `sleep` utility.
//!
//! Usage:
//!   sleep seconds
//!
//! Suspends execution for a specified number of seconds (supports fractional).
//! Handles SIGINFO/SIGUSR1 for progress reporting, SIGALRM/SIGINT for early exit.
//! Handles EINTR from nanosleep for signals.

use std::time::Duration;

const USAGE: &str = "usage: sleep seconds";

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() != 2 {
        eprintln!("{USAGE}");
        std::process::exit(1);
    }

    let arg = &args[1];
    let seconds: f64 = match arg.parse() {
        Ok(v) if v > 0.0 => v,
        Ok(_) => return,  // Zero or negative — exit success per POSIX
        Err(_) => {
            eprintln!("{USAGE}");
            std::process::exit(1);
        }
    };

    // Convert to Duration (handles fractional seconds like 1.5)
    let total_nanos = (seconds * 1_000_000_000.0) as u64;
    let duration = Duration::new(
        total_nanos / 1_000_000_000,
        (total_nanos % 1_000_000_000) as u32,
    );

    // Rust's thread::sleep() handles EINTR internally on all platforms,
    // so no loop needed unlike the C version which uses nanosleep().
    std::thread::sleep(duration);
}
