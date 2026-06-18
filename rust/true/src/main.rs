//! Rust port of the MINIX/NetBSD `true` utility.
//!
//! Usage:
//!   true
//!
//! Exits with status 0 (success). Ignores all arguments.

fn main() {
    std::process::exit(0);
}
