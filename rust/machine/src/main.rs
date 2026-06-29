//! Rust port of the MINIX/NetBSD `machine` utility.
//!
//! Usage:
//!   machine
//!
//! Prints the machine architecture.

fn main() {
    #[cfg(target_arch = "x86_64")]
    println!("amd64");
    #[cfg(target_arch = "x86")]
    println!("i386");
    #[cfg(target_arch = "aarch64")]
    println!("aarch64");
    #[cfg(target_arch = "arm")]
    println!("arm");
    #[cfg(target_arch = "riscv64")]
    println!("riscv64");
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64", target_arch = "arm", target_arch = "riscv64")))]
    println!("unknown");
}
