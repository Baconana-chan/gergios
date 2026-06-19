//! Rust port of the MINIX/NetBSD `sync` utility.
//!
//! Usage:
//!   sync
//!
//! Calls sync(2) to flush filesystem buffers. Ignores all arguments.

fn main() {
    unsafe { libc::sync(); }
}
