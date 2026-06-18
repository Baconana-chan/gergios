//! # UDP — User Datagram Protocol header parsing

use crate::util::read_u16;
use crate::{ParseError, ParseResult};

/// Size of a UDP header in bytes.
pub const UDP_HEADER_LEN: usize = 8;

/// Parsed UDP header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UdpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub length: u16,
    pub checksum: u16,
}

impl UdpHeader {
    /// Parse a UDP header from raw bytes.
    ///
    /// Returns `Err(Truncated)` if the buffer is shorter than 8 bytes.
    pub fn parse(buf: &[u8]) -> ParseResult<Self> {
        if buf.len() < UDP_HEADER_LEN {
            return Err(ParseError::Truncated);
        }

        Ok(Self {
            src_port: read_u16(buf, 0)?,
            dst_port: read_u16(buf, 2)?,
            length: read_u16(buf, 4)?,
            checksum: read_u16(buf, 6)?,
        })
    }

    /// Get the payload length (total length - header).
    pub fn payload_len(&self) -> Option<usize> {
        let len = self.length as usize;
        if len >= UDP_HEADER_LEN {
            Some(len - UDP_HEADER_LEN)
        } else {
            None
        }
    }
}

impl core::fmt::Display for UdpHeader {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "UDP {} -> {} len={}",
            self.src_port, self.dst_port, self.length,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn udp_packet() -> [u8; 8] {
        let mut buf = [0u8; 8];
        // src_port=53 (DNS), dst_port=12345
        buf[0..2].copy_from_slice(&(53u16).to_be_bytes());
        buf[2..4].copy_from_slice(&(12345u16).to_be_bytes());
        // length = 8 (header only, no payload)
        buf[4..6].copy_from_slice(&(8u16).to_be_bytes());
        buf
    }

    #[test]
    fn parse_udp() {
        let pkt = udp_packet();
        let udp = UdpHeader::parse(&pkt).unwrap();
        assert_eq!(udp.src_port, 53);
        assert_eq!(udp.dst_port, 12345);
        assert_eq!(udp.length, 8);
        assert_eq!(udp.payload_len(), Some(0));
    }

    #[test]
    fn parse_udp_truncated() {
        assert_eq!(UdpHeader::parse(&[0u8; 4]), Err(ParseError::Truncated));
    }

    // Helper to test Display without std::format!
    struct TestBuf<const N: usize>([u8; N], usize);
    impl<const N: usize> TestBuf<N> {
        fn new() -> Self { Self([0u8; N], 0) }
        fn as_str(&self) -> &str { core::str::from_utf8(&self.0[..self.1]).unwrap() }
    }
    impl<const N: usize> core::fmt::Write for TestBuf<N> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let b = s.as_bytes();
            if self.1 + b.len() > N { return Err(core::fmt::Error); }
            self.0[self.1..self.1 + b.len()].copy_from_slice(b);
            self.1 += b.len();
            Ok(())
        }
    }

    #[test]
    fn udp_display() {
        use core::fmt::Write;
        let pkt = udp_packet();
        let udp = UdpHeader::parse(&pkt).unwrap();
        let mut buf = TestBuf::<64>::new();
        write!(buf, "{}", udp).unwrap();
        assert!(buf.as_str().contains("53"));
        assert!(buf.as_str().contains("12345"));
    }
}
