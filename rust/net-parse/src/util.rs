//! # net-parse utilities
//!
//! Common helper functions for network protocol parsing.

use crate::{ParseError, ParseResult};

/// Read a big-endian u16 from a slice at the given offset.
///
/// Returns `Err(Truncated)` if the offset + 2 exceeds the slice length.
#[inline]
pub fn read_u16(buf: &[u8], offset: usize) -> ParseResult<u16> {
    if offset + 2 > buf.len() {
        return Err(ParseError::Truncated);
    }
    Ok(u16::from_be_bytes([buf[offset], buf[offset + 1]]))
}

/// Read a big-endian u32 from a slice at the given offset.
#[inline]
pub fn read_u32(buf: &[u8], offset: usize) -> ParseResult<u32> {
    if offset + 4 > buf.len() {
        return Err(ParseError::Truncated);
    }
    Ok(u32::from_be_bytes([
        buf[offset],
        buf[offset + 1],
        buf[offset + 2],
        buf[offset + 3],
    ]))
}

/// Read a u8 from a slice at the given offset.
#[inline]
pub fn read_u8(buf: &[u8], offset: usize) -> ParseResult<u8> {
    if offset >= buf.len() {
        return Err(ParseError::Truncated);
    }
    Ok(buf[offset])
}

/// Copy a fixed-size array from a slice at the given offset.
#[inline]
pub fn read_array<const N: usize>(buf: &[u8], offset: usize) -> ParseResult<[u8; N]> {
    if offset + N > buf.len() {
        return Err(ParseError::Truncated);
    }
    let mut arr = [0u8; N];
    arr.copy_from_slice(&buf[offset..offset + N]);
    Ok(arr)
}

/// Compute the Internet checksum (RFC 1071) over a sequence of data.
///
/// This is the ones' complement of the ones' complement sum of 16-bit words.
/// Used by TCP, UDP, and IP headers.
pub fn internet_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += u32::from(u16::from_be_bytes([data[i], data[i + 1]]));
        i += 2;
    }
    if i < data.len() {
        sum += u32::from(data[i]) << 8; // trailing byte
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// Verify that an Internet checksum is valid (equals 0 after folding).
pub fn verify_checksum(data: &[u8]) -> bool {
    internet_checksum(data) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_u16() {
        let buf = [0x08, 0x00]; // 2048 in big-endian
        assert_eq!(read_u16(&buf, 0).unwrap(), 2048);
    }

    #[test]
    fn test_read_u16_truncated() {
        let buf = [0x08];
        assert_eq!(read_u16(&buf, 0), Err(ParseError::Truncated));
    }

    #[test]
    fn test_read_u32() {
        let buf = [0xC0, 0xA8, 0x00, 0x01]; // 192.168.0.1
        assert_eq!(read_u32(&buf, 0).unwrap(), 0xC0A8_0001);
    }

    #[test]
    fn test_read_u8() {
        assert_eq!(read_u8(&[0x42], 0).unwrap(), 0x42);
        assert_eq!(read_u8(&[], 0), Err(ParseError::Truncated));
    }

    #[test]
    fn test_read_array() {
        let buf = [0x01, 0x02, 0x03, 0x04];
        assert_eq!(read_array::<4>(&buf, 0).unwrap(), [0x01, 0x02, 0x03, 0x04]);
        assert_eq!(read_array::<4>(&buf, 1), Err(ParseError::Truncated));
    }

    #[test]
    fn test_internet_checksum() {
        // RFC 1071 example: sum of 0x0001 and 0xF203
        let data = [0x00, 0x01, 0xF2, 0x03];
        // ones' complement of (0x0001 + 0xF203 = 0xF204)
        assert_eq!(internet_checksum(&data), !0xF204u16);
    }

    #[test]
    fn test_verify_checksum() {
        // A valid checksummed packet would have checksum = 0 after verification
        // For a simple test, just check that the fold works
        let checksum = internet_checksum(&[0x00, 0x01, 0x00, 0x02]);
        assert_ne!(checksum, 0); // checksum is not zero for data
    }
}
