//! # net-parse — MINIX network protocol parsers
//!
//! This crate provides safe, `no_std` protocol parsers for TCP, UDP, and DNS
//! headers. It contains **zero `unsafe` code** and uses only slice-based
//! parsing with exhaustive bounds checking.
//!
//! ## Design
//!
//! All parsers follow the same pattern:
//! 1. Take a `&[u8]` slice
//! 2. Check the length is sufficient for the header
//! 3. Return a parsed struct (zero-copy where possible)
//!
//! ## Usage
//!
//! ```ignore
//! use net_parse::tcp::TcpHeader;
//!
//! let bytes = get_packet();
//! if let Ok(tcp) = TcpHeader::parse(&bytes) {
//!     println!("src_port={}, dst_port={}", tcp.src_port, tcp.dst_port);
//! }
//! ```

#![no_std]
#![deny(unsafe_code)]

pub mod tcp;
pub mod udp;
pub mod dns;
pub mod util;

/// Common error type for packet parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// Packet is too short to contain the header.
    Truncated,
    /// Packet contains invalid protocol data.
    InvalidData,
    /// Checksum verification failed.
    ChecksumMismatch,
    /// Unsupported protocol version or option.
    Unsupported,
}

/// Result type for packet parsing operations.
pub type ParseResult<T> = Result<T, ParseError>;
