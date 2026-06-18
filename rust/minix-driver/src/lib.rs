//! # minix-driver — Safe MMIO and port I/O wrappers for MINIX drivers
//!
//! This crate provides type-safe wrappers for MINIX hardware access patterns:
//!
//! - **MMIO** (Memory-Mapped I/O): `read32`, `write32`, `set32`, `read16`,
//!   `write16`, `set16` with `VolatileCell` for compile-time volatile access.
//! - **Port I/O** (x86): Safe wrappers around `sys_inb`/`sys_outb` via FFI.
//!
//! ## Safety
//!
//! MMIO operations are inherently unsafe due to side effects on hardware
//! registers. This crate provides safe abstractions on top of `unsafe` primitives,
//! enforcing correct types, alignment, and bounds checking where possible.
//!
//! ## Usage
//!
//! ```ignore
//! use minix_driver::mmio::*;
//!
//! let reg = MmioRegion::new(0xF000_0000 as *mut u32, 1024);
//! let val = reg.read32(0x100); // read register at offset 0x100
//! reg.write32(0x100, val | 0x1); // set bit 0
//! ```

#![no_std]
// MMIO and port I/O require unsafe for hardware register access.
// This crate wraps unsafe operations in safe, bounds-checked APIs.
#![deny(unsafe_op_in_unsafe_fn)]

pub mod mmio;
pub mod port;

/// Common error type for driver operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverError {
    /// The MMIO region pointer is null.
    NullPointer,
    /// Access is outside the mapped region bounds.
    OutOfBounds,
    /// I/O port operation failed (MINIX kernel call returned error).
    IoError,
    /// Invalid alignment for the requested access width.
    Misaligned,
}
