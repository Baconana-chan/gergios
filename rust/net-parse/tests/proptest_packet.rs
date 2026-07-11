//! Property-based tests for net-parse protocol parsers.
//!
//! Uses the proptest API directly (TestRunner::run()) instead of the
//! proptest! macro, because the macro in v1.11.0 doesn't generate
//! #[test] functions in integration tests.

use proptest::prelude::*;
use proptest::test_runner::{TestRunner, Config};
use net_parse::tcp::TcpHeader;
use net_parse::udp::UdpHeader;
use net_parse::ParseError;

#[test]
fn valid_tcp_header_parses() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(any::<u16>(), any::<u16>(), any::<u32>(), any::<u32>(),
          5u16..=15u16, any::<u8>(), any::<u16>(), any::<u16>(), any::<u16>()),
        |(src_port, dst_port, seq_num, ack_num, data_offset, flags, window_size, checksum, urgent_ptr)| {
            let mut buf = [0u8; 20];
            buf[0..2].copy_from_slice(&src_port.to_be_bytes());
            buf[2..4].copy_from_slice(&dst_port.to_be_bytes());
            buf[4..8].copy_from_slice(&seq_num.to_be_bytes());
            buf[8..12].copy_from_slice(&ack_num.to_be_bytes());
            buf[12] = (data_offset as u8) << 4;
            buf[13] = flags & 0x3F;
            buf[14..16].copy_from_slice(&window_size.to_be_bytes());
            buf[16..18].copy_from_slice(&checksum.to_be_bytes());
            buf[18..20].copy_from_slice(&urgent_ptr.to_be_bytes());
            let tcp = TcpHeader::parse(&buf).unwrap();
            prop_assert_eq!(tcp.src_port, src_port);
            prop_assert_eq!(tcp.dst_port, dst_port);
            prop_assert_eq!(tcp.seq_num, seq_num);
            prop_assert_eq!(tcp.window_size, window_size);
            prop_assert_eq!(tcp.header_len(), (data_offset as usize) * 4);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn valid_udp_header_parses() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(any::<u16>(), any::<u16>(), 8u16..=65535u16, any::<u16>()),
        |(src_port, dst_port, length, checksum)| {
            let mut buf = [0u8; 8];
            buf[0..2].copy_from_slice(&src_port.to_be_bytes());
            buf[2..4].copy_from_slice(&dst_port.to_be_bytes());
            buf[4..6].copy_from_slice(&length.to_be_bytes());
            buf[6..8].copy_from_slice(&checksum.to_be_bytes());
            let udp = UdpHeader::parse(&buf).unwrap();
            prop_assert_eq!(udp.src_port, src_port);
            prop_assert_eq!(udp.dst_port, dst_port);
            prop_assert_eq!(udp.length, length);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn truncated_tcp_is_rejected() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(proptest::collection::vec(any::<u8>(), 0..=19)),
        |bytes| {
            prop_assert_eq!(TcpHeader::parse(&bytes), Err(ParseError::Truncated));
            Ok(())
        },
    ).unwrap();
}

#[test]
fn truncated_udp_is_rejected() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(proptest::collection::vec(any::<u8>(), 0..=7)),
        |bytes| {
            prop_assert_eq!(UdpHeader::parse(&bytes), Err(ParseError::Truncated));
            Ok(())
        },
    ).unwrap();
}

#[test]
fn invalid_tcp_data_offset_is_rejected() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(any::<u16>(), any::<u16>(), any::<u8>()),
        |(src_port, dst_port, flags)| {
            let mut buf = [0u8; 20];
            buf[0..2].copy_from_slice(&src_port.to_be_bytes());
            buf[2..4].copy_from_slice(&dst_port.to_be_bytes());
            for doff in 0u8..=4u8 {
                buf[12] = doff << 4;
                buf[13] = flags & 0x3F;
                prop_assert_eq!(TcpHeader::parse(&buf), Err(ParseError::InvalidData));
            }
            Ok(())
        },
    ).unwrap();
}

#[test]
fn tcp_payload_len_consistency() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(5u16..=15u16, 0u16..=100u16),
        |(data_offset, extra_bytes)| {
            let header_len = (data_offset as usize) * 4;
            let total_len = header_len + extra_bytes as usize;
            let mut buf = vec![0u8; header_len.max(20)];
            buf[12] = (data_offset as u8) << 4;
            let tcp = TcpHeader::parse(&buf).unwrap();
            prop_assert_eq!(tcp.payload_len(total_len), Some(extra_bytes as usize));
            Ok(())
        },
    ).unwrap();
}

#[test]
fn udp_payload_len_consistency() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(8u16..=65535u16),
        |length| {
            let mut buf = [0u8; 8];
            buf[4..6].copy_from_slice(&length.to_be_bytes());
            let udp = UdpHeader::parse(&buf).unwrap();
            prop_assert_eq!(udp.payload_len(), Some((length - 8) as usize));
            Ok(())
        },
    ).unwrap();
}

#[test]
fn tcp_flags_roundtrip() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(0u8..=0x3Fu8),
        |flags_bits| {
            use net_parse::tcp::TcpFlags;
            let flags = TcpFlags::new(flags_bits);
            prop_assert_eq!(flags.bits(), flags_bits);
            prop_assert_eq!(flags.is_syn(), flags_bits & TcpFlags::SYN != 0);
            prop_assert_eq!(flags.is_ack(), flags_bits & TcpFlags::ACK != 0);
            prop_assert_eq!(flags.is_fin(), flags_bits & TcpFlags::FIN != 0);
            prop_assert_eq!(flags.is_rst(), flags_bits & TcpFlags::RST != 0);
            Ok(())
        },
    ).unwrap();
}

#[test]
fn udp_short_length() {
    let mut runner = TestRunner::new(Config::default());
    runner.run(
        &(any::<u16>(), any::<u16>(), 0u16..=7u16),
        |(src_port, dst_port, short_len)| {
            let mut buf = [0u8; 8];
            buf[0..2].copy_from_slice(&src_port.to_be_bytes());
            buf[2..4].copy_from_slice(&dst_port.to_be_bytes());
            buf[4..6].copy_from_slice(&short_len.to_be_bytes());
            let udp = UdpHeader::parse(&buf).unwrap();
            prop_assert_eq!(udp.payload_len(), None);
            Ok(())
        },
    ).unwrap();
}
