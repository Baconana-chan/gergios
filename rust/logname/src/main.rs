//! Rust port of the MINIX/NetBSD `logname` utility.
//!
//! Usage:
//!   logname
//!
//! Prints the user's login name.

fn main() {
    let name = std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| {
            eprintln!("logname: no login name");
            std::process::exit(1);
        });
    println!("{}", name);
}
