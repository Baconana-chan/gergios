//! # TCP — Transmission Control Protocol header parsing

use crate::util::{read_u16, read_u32};
use crate::{ParseError, ParseResult};

/// Size of a TCP header without options, in bytes.
pub const TCP_HEADER_LEN: usize = 20;

/// TCP header flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpFlags(u8);

impl TcpFlags {
    pub const FIN: u8 = 0x01;
    pub const SYN: u8 = 0x02;
    pub const RST: u8 = 0x04;
    pub const PSH: u8 = 0x08;
    pub const ACK: u8 = 0x10;
    pub const URG: u8 = 0x20;
    pub const ECE: u8 = 0x40;
    pub const CWR: u8 = 0x80;

    pub fn new(bits: u8) -> Self {
        Self(bits)
    }

    pub fn bits(&self) -> u8 {
        self.0
    }

    pub fn is_set(&self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    pub fn is_syn(&self) -> bool {
        self.is_set(Self::SYN)
    }
    pub fn is_ack(&self) -> bool {
        self.is_set(Self::ACK)
    }
    pub fn is_fin(&self) -> bool {
        self.is_set(Self::FIN)
    }
    pub fn is_rst(&self) -> bool {
        self.is_set(Self::RST)
    }
    pub fn is_psh(&self) -> bool {
        self.is_set(Self::PSH)
    }
}

/// Parsed TCP header (no options — use `parse_full` for options).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub seq_num: u32,
    pub ack_num: u32,
    pub data_offset: u8,
    pub flags: TcpFlags,
    pub window_size: u16,
    pub checksum: u16,
    pub urgent_ptr: u16,
}

impl TcpHeader {
    /// Parse a TCP header from raw bytes.
    ///
    /// Returns `Err(Truncated)` if the buffer is shorter than 20 bytes.
    pub fn parse(buf: &[u8]) -> ParseResult<Self> {
        if buf.len() < TCP_HEADER_LEN {
            return Err(ParseError::Truncated);
        }

        let src_port = read_u16(buf, 0)?;
        let dst_port = read_u16(buf, 2)?;
        let seq_num = read_u32(buf, 4)?;
        let ack_num = read_u32(buf, 8)?;

        let data_offset_and_flags = read_u16(buf, 12)?;
        let data_offset = ((data_offset_and_flags >> 12) & 0x0F) as u8;
        let flags_raw = (data_offset_and_flags & 0x3F) as u8;

        let window_size = read_u16(buf, 14)?;
        let checksum = read_u16(buf, 16)?;
        let urgent_ptr = read_u16(buf, 18)?;

        if data_offset < 5 {
            return Err(ParseError::InvalidData);
        }

        Ok(Self {
            src_port,
            dst_port,
            seq_num,
            ack_num,
            data_offset,
            flags: TcpFlags::new(flags_raw),
            window_size,
            checksum,
            urgent_ptr,
        })
    }

    /// Calculate the header length in bytes (from the data offset field).
    pub fn header_len(&self) -> usize {
        (self.data_offset as usize) * 4
    }

    /// Calculate the payload length given the total segment length.
    pub fn payload_len(&self, total_len: usize) -> Option<usize> {
        let hlen = self.header_len();
        if total_len >= hlen {
            Some(total_len - hlen)
        } else {
            None
        }
    }

    /// Verify the TCP checksum over the pseudo-header + TCP segment.
    ///
    /// This is a stub — full verification requires heap allocation to build
    /// the pseudo-header + segment buffer (with zeroed checksum field).
    /// Use an external crate when full checksum verification is needed.
    pub fn verify_checksum(
        &self,
        _src_ip: [u8; 4],
        _dst_ip: [u8; 4],
        _segment: &[u8],
    ) -> bool {
        // TODO: implement proper TCP checksum verification
        // This requires concatenating pseudo-header + TCP segment which
        // needs heap allocation. Deferred until alloc/vec is available.
        unimplemented!("TCP checksum verification not yet implemented in no_std")
    }
}

impl core::fmt::Display for TcpHeader {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "TCP {} -> {} [{}] seq={} ack={} win={}",
            self.src_port,
            self.dst_port,
            if self.flags.is_syn() { "SYN" } else if self.flags.is_ack() { "ACK" } else { "" },
            self.seq_num,
            self.ack_num,
            self.window_size,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// A minimal TCP SYN packet (20 bytes, no options).
    fn syn_packet() -> [u8; 20] {
        let mut buf = [0u8; 20];
        // src_port=1234, dst_port=80
        buf[0..2].copy_from_slice(&(1234u16).to_be_bytes());
        buf[2..4].copy_from_slice(&(80u16).to_be_bytes());
        // seq_num = 1000
        buf[4..8].copy_from_slice(&(1000u32).to_be_bytes());
        // ack_num = 0
        // data_offset = 5 (20 bytes), flags = SYN (0x02)
        buf[12] = 0x50; // data_offset=5, reserved=0
        buf[13] = 0x02; // SYN flag
        // window_size = 65535
        buf[14..16].copy_from_slice(&(65535u16).to_be_bytes());
        buf
    }

    #[test]
    fn parse_tcp_syn() {
        let pkt = syn_packet();
        let tcp = TcpHeader::parse(&pkt).unwrap();
        assert_eq!(tcp.src_port, 1234);
        assert_eq!(tcp.dst_port, 80);
        assert_eq!(tcp.seq_num, 1000);
        assert!(tcp.flags.is_syn());
        assert!(!tcp.flags.is_ack());
        assert_eq!(tcp.window_size, 65535);
    }

    #[test]
    fn parse_tcp_truncated() {
        let buf = [0u8; 10];
        assert_eq!(TcpHeader::parse(&buf), Err(ParseError::Truncated));
    }

    #[test]
    fn parse_tcp_invalid_offset() {
        let mut buf = [0u8; 20];
        // data_offset = 0 (< 5), invalid
        let result = TcpHeader::parse(&buf);
        assert_eq!(result, Err(ParseError::InvalidData));
    }

    #[test]
    fn tcp_flags() {
        let flags = TcpFlags::new(TcpFlags::SYN | TcpFlags::ACK);
        assert!(flags.is_syn());
        assert!(flags.is_ack());
        assert!(!flags.is_fin());
        assert!(!flags.is_rst());
    }

    #[test]
    fn tcp_header_len() {
        let pkt = syn_packet();
        let tcp = TcpHeader::parse(&pkt).unwrap();
        assert_eq!(tcp.header_len(), 20);
    }

    #[test]
    fn tcp_display() {
        use core::fmt::Write;
        let pkt = syn_packet();
        let tcp = TcpHeader::parse(&pkt).unwrap();
        let mut buf = TestBuf::<64>::new();
        write!(buf, "{}", tcp).unwrap();
        assert!(buf.as_str().contains("1234"));
        assert!(buf.as_str().contains("80"));
    }
}
