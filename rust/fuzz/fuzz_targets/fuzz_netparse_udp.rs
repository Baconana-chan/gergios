#![no_main]

use libfuzzer_sys::fuzz_target;
use net_parse::udp::UdpHeader;

fuzz_target!(|data: &[u8]| {
    if let Ok(udp) = UdpHeader::parse(data) {
        let _ = udp.src_port;
        let _ = udp.dst_port;
        let _ = udp.length;
        let _ = udp.payload_len();
    }
});
