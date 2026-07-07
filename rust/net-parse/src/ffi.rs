//! # FFI — C-compatible exports for net-parse
//!
//! These functions provide a safe C API for parsing TCP/UDP headers and
//! computing Internet checksums from C code (e.g., the MINIX lwIP service).
//!
//! ## Safety
//!
//! All functions validate their inputs (null pointers, buffer lengths) before
//! accessing memory. They are marked `unsafe extern "C"` because they take raw
//! pointers, but the implementations are safe as long as the caller provides
//! valid pointers and lengths.
//!
//! ## Usage (C)
//!
//! ```c
//! #include <net_parse.h>
//!
//! struct TcpHeaderFFI tcp;
//! int ret = net_parse_tcp_header(packet, packet_len, &tcp);
//! if (ret == 0) {
//!     // use tcp.src_port, tcp.dst_port, etc.
//! }
//! ```

// We allow unsafe_code here because FFI inherently requires unsafe for
// dereferencing raw pointers from C. Each function validates inputs first.
#![allow(unsafe_code)]

use core::ffi::{c_int, c_uchar};

// ============================================================================
// Return codes
// ============================================================================

/// Parsing succeeded.
const NET_PARSE_OK: c_int = 0;
/// The input buffer is too short to contain the header.
const NET_PARSE_ERR_TRUNCATED: c_int = -1;
/// The input buffer contains invalid protocol data.
const NET_PARSE_ERR_INVALID: c_int = -2;

// ============================================================================
// C-compatible header structures
// ============================================================================

/// TCP header (20 bytes minimum, excluding options) — C-compatible.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TcpHeaderFFI {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset: u8,
    pub flags: u8,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
}

/// UDP header (8 bytes) — C-compatible.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UdpHeaderFFI {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
}

// ============================================================================
// TCP header parsing
// ============================================================================

/// Parse a TCP header from raw bytes.
///
/// # Arguments
/// * `buf` — Pointer to the TCP segment data (including header).
/// * `buflen` — Length of the buffer in bytes.
/// * `out` — Output structure to receive the parsed header.
///
/// # Returns
/// * `0` (NET_PARSE_OK) on success.
/// * `-1` (NET_PARSE_ERR_TRUNCATED) if the buffer is too short.
/// * `-2` (NET_PARSE_ERR_INVALID) if header data is invalid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn net_parse_tcp_header(
    buf: *const c_uchar,
    buflen: usize,
    out: *mut TcpHeaderFFI,
) -> c_int {
    // Validate input pointers
    if buf.is_null() || out.is_null() {
        return NET_PARSE_ERR_INVALID;
    }

    // Create a safe slice from the raw pointer
    let slice = unsafe { core::slice::from_raw_parts(buf, buflen) };

    // Parse using the safe Rust parser
    match crate::tcp::TcpHeader::parse(slice) {
        Ok(hdr) => {
            // Write the parsed data to the output structure
            unsafe {
                (*out).src_port = hdr.src_port;
                (*out).dst_port = hdr.dst_port;
                (*out).seq_num = hdr.seq_num;
                (*out).ack_num = hdr.ack_num;
                (*out).data_offset = hdr.data_offset;
                (*out).flags = hdr.flags.bits();
                (*out).window_size = hdr.window_size;
                (*out).checksum = hdr.checksum;
                (*out).urgent_ptr = hdr.urgent_ptr;
            }
            NET_PARSE_OK
        }
        Err(crate::ParseError::Truncated) => NET_PARSE_ERR_TRUNCATED,
        Err(_) => NET_PARSE_ERR_INVALID,
    }
}

// ============================================================================
// UDP header parsing
// ============================================================================

/// Parse a UDP header from raw bytes.
///
/// # Arguments
/// * `buf` — Pointer to the UDP datagram data (including header).
/// * `buflen` — Length of the buffer in bytes.
/// * `out` — Output structure to receive the parsed header.
///
/// # Returns
/// * `0` (NET_PARSE_OK) on success.
/// * `-1` (NET_PARSE_ERR_TRUNCATED) if the buffer is too short.
/// * `-2` (NET_PARSE_ERR_INVALID) if header data is invalid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn net_parse_udp_header(
    buf: *const c_uchar,
    buflen: usize,
    out: *mut UdpHeaderFFI,
) -> c_int {
    // Validate input pointers
    if buf.is_null() || out.is_null() {
        return NET_PARSE_ERR_INVALID;
    }

    // Create a safe slice from the raw pointer
    let slice = unsafe { core::slice::from_raw_parts(buf, buflen) };

    // Parse using the safe Rust parser
    match crate::udp::UdpHeader::parse(slice) {
        Ok(hdr) => {
            unsafe {
                (*out).src_port = hdr.src_port;
                (*out).dst_port = hdr.dst_port;
                (*out).length = hdr.length;
                (*out).checksum = hdr.checksum;
            }
            NET_PARSE_OK
        }
        Err(crate::ParseError::Truncated) => NET_PARSE_ERR_TRUNCATED,
        Err(_) => NET_PARSE_ERR_INVALID,
    }
}

// ============================================================================
// Internet checksum (RFC 1071)
// ============================================================================

/// Compute the Internet checksum (RFC 1071) over a data buffer.
///
/// Returns the ones' complement of the ones' complement sum of 16-bit words.
/// This can be used for IP, TCP, and UDP header checksum verification.
///
/// # Arguments
/// * `data` — Pointer to the data buffer.
/// * `len` — Length of the data in bytes.
///
/// # Returns
/// The 16-bit Internet checksum value. A zero result means the data is valid
/// (when used for verification with the checksum field included).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn net_parse_checksum(
    data: *const c_uchar,
    len: usize,
) -> u16 {
    if data.is_null() || len == 0 {
        return 0; // checksum of empty data is 0
    }
    let slice = unsafe { core::slice::from_raw_parts(data, len) };
    crate::util::internet_checksum(slice)
}

/// Verify an Internet checksum over a data buffer (including the checksum field).
///
/// Returns 1 if the checksum is valid (sum folds to 0), or 0 otherwise.
///
/// # Arguments
/// * `data` — Pointer to the data buffer (including the checksum field).
/// * `len` — Length of the data in bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn net_parse_checksum_verify(
    data: *const c_uchar,
    len: usize,
) -> c_int {
    if data.is_null() || len == 0 {
        return 0;
    }
    let slice = unsafe { core::slice::from_raw_parts(data, len) };
    if crate::util::verify_checksum(slice) { 1 } else { 0 }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal TCP SYN packet (20 bytes, no options).
    fn syn_packet() -> [u8; 20] {
        let mut buf = [0u8; 20];
        buf[0..2].copy_from_slice(&(1234u16).to_be_bytes());
        buf[2..4].copy_from_slice(&(80u16).to_be_bytes());
        buf[4..8].copy_from_slice(&(1000u32).to_be_bytes());
        buf[12] = 0x50; // data_offset=5, reserved=0
        buf[13] = 0x02; // SYN flag
        buf[14..16].copy_from_slice(&(65535u16).to_be_bytes());
        buf
    }

    #[test]
    fn ffi_tcp_header() {
        let pkt = syn_packet();
        let mut out = TcpHeaderFFI {
            src_port: 0, dst_port: 0, seq_num: 0, ack_num: 0,
            data_offset: 0, flags: 0, window_size: 0,
            checksum: 0, urgent_ptr: 0,
        };
        let ret = unsafe {
            net_parse_tcp_header(pkt.as_ptr(), pkt.len(), &mut out)
        };
        assert_eq!(ret, NET_PARSE_OK);
        assert_eq!(out.src_port, 1234);
        assert_eq!(out.dst_port, 80);
        assert_eq!(out.seq_num, 1000);
        assert_eq!(out.flags, 0x02); // SYN
        assert_eq!(out.window_size, 65535);
    }

    #[test]
    fn ffi_tcp_null_ptr() {
        let ret = unsafe {
            net_parse_tcp_header(core::ptr::null(), 0, core::ptr::null_mut())
        };
        assert_eq!(ret, NET_PARSE_ERR_INVALID);
    }

    #[test]
    fn ffi_tcp_truncated() {
        let buf = [0u8; 10];
        let mut out = TcpHeaderFFI {
            src_port: 0, dst_port: 0, seq_num: 0, ack_num: 0,
            data_offset: 0, flags: 0, window_size: 0,
            checksum: 0, urgent_ptr: 0,
        };
        let ret = unsafe {
            net_parse_tcp_header(buf.as_ptr(), buf.len(), &mut out)
        };
        assert_eq!(ret, NET_PARSE_ERR_TRUNCATED);
    }

    #[test]
    fn ffi_udp_header() {
        let mut buf = [0u8; 8];
        buf[0..2].copy_from_slice(&(53u16).to_be_bytes());
        buf[2..4].copy_from_slice(&(12345u16).to_be_bytes());
        buf[4..6].copy_from_slice(&(8u16).to_be_bytes());

        let mut out = UdpHeaderFFI {
            src_port: 0, dst_port: 0, length: 0, checksum: 0,
        };
        let ret = unsafe {
            net_parse_udp_header(buf.as_ptr(), buf.len(), &mut out)
        };
        assert_eq!(ret, NET_PARSE_OK);
        assert_eq!(out.src_port, 53);
        assert_eq!(out.dst_port, 12345);
        assert_eq!(out.length, 8);
    }

    #[test]
    fn ffi_checksum() {
        let data = [0x00, 0x01, 0xF2, 0x03];
        let cksum = unsafe { net_parse_checksum(data.as_ptr(), data.len()) };
        assert_eq!(cksum, !0xF204u16);
    }

    #[test]
    fn ffi_checksum_null() {
        let cksum = unsafe { net_parse_checksum(core::ptr::null(), 0) };
        assert_eq!(cksum, 0);
    }

    #[test]
    fn ffi_checksum_verify() {
        // For a simple test: verify that checksum of non-empty data is not "valid"
        let data = [0x00, 0x01, 0x00, 0x02];
        let ret = unsafe {
            net_parse_checksum_verify(data.as_ptr(), data.len())
        };
        assert_eq!(ret, 0); // not zero = not valid
    }
}
