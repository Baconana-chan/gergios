//! Rust port of the MINIX/NetBSD `pagesize` utility.
//!
//! Usage:
//!   pagesize
//!
//! Prints the system page size in bytes.

fn main() {
    #[cfg(unix)]
    {
        let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
        println!("{}", page_size);
    }

    #[cfg(not(unix))]
    {
        println!("4096");
    }
}
