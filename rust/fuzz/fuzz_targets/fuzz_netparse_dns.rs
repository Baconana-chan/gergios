#![no_main]

use libfuzzer_sys::fuzz_target;
use net_parse::dns::{DnsHeader, decode_name_buf};

fuzz_target!(|data: &[u8]| {
    if let Ok(hdr) = DnsHeader::parse(data) {
        let _ = hdr.flags.is_query();
        let _ = hdr.flags.is_response();
        let _ = hdr.flags.rcode();
        let _ = hdr.total_entries();
    }
    // Fuzz name decompression
    if data.len() > 12 {
        let mut out = [0u8; 256];
        let _ = decode_name_buf(data, 12, &mut out);
    }
});
