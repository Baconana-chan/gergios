//! # DNS — Domain Name System message parsing

use crate::util::{read_u16, read_array};
use crate::{ParseError, ParseResult};

/// Size of a DNS header in bytes.
pub const DNS_HEADER_LEN: usize = 12;

/// DNS header flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DnsFlags(u16);

impl DnsFlags {
    pub fn new(bits: u16) -> Self {
        Self(bits)
    }

    pub fn bits(&self) -> u16 {
        self.0
    }

    /// Is this a query (0) or a response (1)?
    pub fn is_query(&self) -> bool {
        (self.0 >> 15) == 0
    }

    pub fn is_response(&self) -> bool {
        (self.0 >> 15) == 1
    }

    /// Operation code (standard query = 0).
    pub fn opcode(&self) -> u8 {
        ((self.0 >> 11) & 0x0F) as u8
    }

    /// Is this a standard query?
    pub fn is_standard_query(&self) -> bool {
        self.opcode() == 0
    }

    /// Response code (0 = no error).
    pub fn rcode(&self) -> u8 {
        (self.0 & 0x0F) as u8
    }

    pub fn is_noerror(&self) -> bool {
        self.rcode() == 0
    }

    pub fn is_nxdomain(&self) -> bool {
        self.rcode() == 3
    }
}

/// Parsed DNS header (12 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DnsHeader {
    pub id: u16,
    pub flags: DnsFlags,
    pub qdcount: u16, // number of questions
    pub ancount: u16, // number of answers
    pub nscount: u16, // number of authority records
    pub arcount: u16, // number of additional records
}

impl DnsHeader {
    /// Parse a DNS header from raw bytes.
    pub fn parse(buf: &[u8]) -> ParseResult<Self> {
        if buf.len() < DNS_HEADER_LEN {
            return Err(ParseError::Truncated);
        }

        Ok(Self {
            id: read_u16(buf, 0)?,
            flags: DnsFlags::new(read_u16(buf, 2)?),
            qdcount: read_u16(buf, 4)?,
            ancount: read_u16(buf, 6)?,
            nscount: read_u16(buf, 8)?,
            arcount: read_u16(buf, 10)?,
        })
    }

    /// Returns the total number of entries (questions + answers + authority + additional).
    pub fn total_entries(&self) -> usize {
        self.qdcount as usize + self.ancount as usize + self.nscount as usize + self.arcount as usize
    }
}

/// DNS query type (QTYPE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum QueryType {
    A     = 1,     // IPv4 address
    NS    = 2,     // nameserver
    CNAME = 5,     // canonical name
    SOA   = 6,     // start of authority
    PTR   = 12,    // pointer (reverse DNS)
    MX    = 15,    // mail exchange
    TXT   = 16,    // text record
    AAAA  = 28,    // IPv6 address
    SRV   = 33,    // service locator
    ANY   = 255,   // wildcard
}

impl QueryType {
    pub fn from_u16(v: u16) -> Option<Self> {
        match v {
            1 => Some(Self::A),
            2 => Some(Self::NS),
            5 => Some(Self::CNAME),
            6 => Some(Self::SOA),
            12 => Some(Self::PTR),
            15 => Some(Self::MX),
            16 => Some(Self::TXT),
            28 => Some(Self::AAAA),
            33 => Some(Self::SRV),
            255 => Some(Self::ANY),
            _ => None,
        }
    }
}

/// DNS query class (QCLASS). Only IN is commonly used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum QueryClass {
    IN = 1,   // internet
    CH = 3,   // CHAOS
    HS = 4,   // HESIOD
}

/// A single DNS question.
#[derive(Debug, Clone)]
pub struct DnsQuestion<'a> {
    /// The uncompressed domain name.
    pub name: &'a [u8],
    pub qtype: u16,
    pub qclass: u16,
}

/// A single DNS resource record.
#[derive(Debug, Clone)]
pub struct DnsRecord<'a> {
    pub name: &'a [u8],
    pub rtype: u16,
    pub rclass: u16,
    pub ttl: u32,
    pub rdlength: u16,
    pub rdata: &'a [u8],
}

impl<'a> DnsRecord<'a> {
    /// Try to parse the rdata as an IPv4 address (for A records).
    pub fn ipv4_addr(&self) -> Option<[u8; 4]> {
        if self.rtype == QueryType::A as u16 && self.rdlength == 4 {
            Some(read_array::<4>(self.rdata, 0).ok()?)
        } else {
            None
        }
    }
}

/// Decode a DNS name into a fixed-size buffer (no alloc).
///
/// Returns the decoded name bytes and the offset after parsing.
/// Buffer must be at least 256 bytes (DNS name max).
pub fn decode_name_buf(msg: &[u8], offset: usize, out: &mut [u8]) -> ParseResult<usize> {
    let mut out_pos = 0;
    let mut cur = offset;
    let mut pointer_count = 0;

    loop {
        if cur >= msg.len() {
            return Err(ParseError::Truncated);
        }
        let len = msg[cur] as usize;

        if len == 0 {
            // End of name
            cur += 1;
            break;
        }

        if len & 0xC0 == 0xC0 {
            // Compressed label (pointer)
            if cur + 1 >= msg.len() {
                return Err(ParseError::Truncated);
            }
            let ptr = ((len & 0x3F) << 8) | (msg[cur + 1] as usize);
            cur += 2;
            pointer_count += 1;
            if pointer_count > 10 {
                return Err(ParseError::InvalidData);
            }
            // Restart at the pointer target
            cur = ptr;
            continue;
        }

        // Normal label
        if cur + 1 + len > msg.len() {
            return Err(ParseError::Truncated);
        }
        if out_pos > 0 {
            if out_pos >= out.len() {
                return Err(ParseError::InvalidData);
            }
            out[out_pos] = b'.';
            out_pos += 1;
        }
        for &b in &msg[cur + 1..cur + 1 + len] {
            if out_pos >= out.len() {
                return Err(ParseError::InvalidData);
            }
            out[out_pos] = b;
            out_pos += 1;
        }
        cur += 1 + len;
    }

    Ok(out_pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dns_query_header() -> [u8; 12] {
        let mut buf = [0u8; 12];
        buf[0..2].copy_from_slice(&(0x1234u16).to_be_bytes()); // id
        buf[2..4].copy_from_slice(&(0x0100u16).to_be_bytes()); // flags: standard query, RD=1
        buf[4..6].copy_from_slice(&(1u16).to_be_bytes()); // qdcount = 1
        buf
    }

    #[test]
    fn parse_dns_header() {
        let hdr = DnsHeader::parse(&dns_query_header()).unwrap();
        assert_eq!(hdr.id, 0x1234);
        assert!(hdr.flags.is_query());
        assert!(!hdr.flags.is_response());
        assert_eq!(hdr.qdcount, 1);
        assert_eq!(hdr.ancount, 0);
    }

    #[test]
    fn parse_dns_truncated() {
        assert_eq!(DnsHeader::parse(&[0u8; 4]), Err(ParseError::Truncated));
    }

    #[test]
    fn dns_flags_query() {
        let flags = DnsFlags::new(0x0100); // QR=0, OPCODE=0000, RD=1
        assert!(flags.is_query());
        assert!(!flags.is_response());
        assert!(flags.is_standard_query());
        assert!(flags.is_noerror());
    }

    #[test]
    fn dns_flags_response() {
        let flags = DnsFlags::new(0x8180); // QR=1, RCODE=0000 (no error)
        assert!(!flags.is_query());
        assert!(flags.is_response());
        assert!(flags.is_noerror());
    }

    #[test]
    fn dns_flags_nxdomain() {
        let flags = DnsFlags::new(0x8183); // QR=1, RCODE=3 (NXDOMAIN)
        assert!(flags.is_response());
        assert!(!flags.is_noerror());
        assert!(flags.is_nxdomain());
    }

    #[test]
    fn query_type_conversion() {
        assert_eq!(QueryType::from_u16(1), Some(QueryType::A));
        assert_eq!(QueryType::from_u16(28), Some(QueryType::AAAA));
        assert_eq!(QueryType::from_u16(255), Some(QueryType::ANY));
        assert_eq!(QueryType::from_u16(999), None);
    }

    #[test]
    fn decode_name_simple() {
        // Encode "www.example.com" manually
        // 3www7example3com0
        let msg = [
            3, b'w', b'w', b'w',
            7, b'e', b'x', b'a', b'm', b'p', b'l', b'e',
            3, b'c', b'o', b'm',
            0,
        ];
        let mut out = [0u8; 256];
        let len = decode_name_buf(&msg, 0, &mut out).unwrap();
        assert_eq!(&out[..len], b"www.example.com");
    }
}
