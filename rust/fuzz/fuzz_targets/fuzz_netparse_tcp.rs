#![no_main]

use libfuzzer_sys::fuzz_target;
use net_parse::tcp::TcpHeader;

fuzz_target!(|data: &[u8]| {
    if let Ok(tcp) = TcpHeader::parse(data) {
        let _ = tcp.src_port;
        let _ = tcp.dst_port;
        let _ = tcp.flags.is_syn();
        let _ = tcp.flags.is_ack();
        let _ = tcp.seq_num;
        let _ = tcp.header_len();
    }
});
