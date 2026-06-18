//! Rust port of the MINIX/NetBSD `false` utility.
//!
//! Usage:
//!   false
//!
//! Exits with status 1 (failure). Ignores all arguments.

fn main() {
    std::process::exit(1);
}
