//! # Message Framing
//!
//! Simple length-prefixed binary message format for game networking.
//!
//! ## Wire Format
//!
//! ```text
//! [ length: 4 bytes LE ] [ tag: 1 byte ] [ payload: N bytes ]
//! ```
//!
//! - `length`: total message size including tag (tag + payload)
//! - `tag`: message type identifier (0..255)
//! - `payload`: raw message data
//!
//! Maximum payload size: 65,535 bytes (16-bit limit for simplicity).

use std::io::{self, Read, Write};

/// Maximum message payload size (64 KB).
pub const MAX_PAYLOAD: usize = 65535;

/// A framed network message with a tag and payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// Message type tag (0..255).
    tag: u8,
    /// Message payload bytes.
    payload: Vec<u8>,
}

impl Message {
    /// Create a new message with the given tag and payload.
    pub fn new(tag: u8, payload: &[u8]) -> Self {
        Self {
            tag,
            payload: payload.to_vec(),
        }
    }

    /// Create a message with an empty payload.
    pub fn empty(tag: u8) -> Self {
        Self {
            tag,
            payload: Vec::new(),
        }
    }

    /// Get the message tag.
    pub fn tag(&self) -> u8 {
        self.tag
    }

    /// Get the message payload as a byte slice.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Consume the message and return the payload.
    pub fn into_payload(self) -> Vec<u8> {
        self.payload
    }

    /// Get the total wire size of this message.
    pub fn wire_size(&self) -> usize {
        4 + 1 + self.payload.len() // length prefix + tag + payload
    }

    /// Encode this message into a framed byte buffer.
    ///
    /// Format: [length:4 LE][tag:1][payload:N]
    pub fn encode(&self) -> Vec<u8> {
        let total = 1 + self.payload.len(); // tag + payload
        let mut buf = Vec::with_capacity(4 + total);
        buf.extend_from_slice(&(total as u32).to_le_bytes()); // length prefix
        buf.push(self.tag);
        buf.extend_from_slice(&self.payload);
        buf
    }

    /// Decode a single message from the beginning of a byte buffer.
    ///
    /// Returns the decoded message and the number of bytes consumed.
    /// Returns `None` if there aren't enough bytes for a complete message.
    pub fn decode(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < 4 {
            return None; // not enough for length prefix
        }

        let total = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;

        if total == 0 || total > 1 + MAX_PAYLOAD {
            return None; // invalid length
        }

        // Total wire size = 4 (length) + total (tag + payload)
        let wire_size = 4 + total;
        if data.len() < wire_size {
            return None; // not enough data
        }

        let tag = data[4];
        let payload = data[5..wire_size].to_vec();

        Some((Self { tag, payload }, wire_size))
    }

    /// Write this message to a stream (TCP socket, file, etc.).
    pub fn write_to<W: Write>(&self, writer: &mut W) -> io::Result<()> {
        let encoded = self.encode();
        writer.write_all(&encoded)
    }

    /// Read one message from a stream.
    ///
    /// Returns `None` if the connection was closed gracefully.
    pub fn read_from<R: Read>(reader: &mut R) -> io::Result<Option<Self>> {
        // Read 4-byte length prefix
        let mut len_buf = [0u8; 4];
        match reader.read_exact(&mut len_buf) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }

        let total = u32::from_le_bytes(len_buf) as usize;
        if total == 0 || total > 1 + MAX_PAYLOAD {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid message length"));
        }

        // Read tag + payload
        let mut body = vec![0u8; total];
        reader.read_exact(&mut body)?;

        let tag = body[0];
        let payload = body[1..].to_vec();

        Ok(Some(Self { tag, payload }))
    }

    /// Create a string message (payload is UTF-8 encoded).
    pub fn string(tag: u8, s: &str) -> Self {
        Self::new(tag, s.as_bytes())
    }

    /// Try to interpret the payload as a UTF-8 string.
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(&self.payload).ok()
    }

    /// Create a message from a serialized type (using a closure).
    pub fn from_serialize<F>(tag: u8, f: F) -> Self
    where
        F: FnOnce(&mut Vec<u8>),
    {
        let mut payload = Vec::new();
        f(&mut payload);
        Self { tag, payload }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_new() {
        let msg = Message::new(42, b"hello");
        assert_eq!(msg.tag(), 42);
        assert_eq!(msg.payload(), b"hello");
    }

    #[test]
    fn test_message_empty() {
        let msg = Message::empty(0);
        assert_eq!(msg.tag(), 0);
        assert!(msg.payload().is_empty());
    }

    #[test]
    fn test_message_encode_decode() {
        let original = Message::new(7, b"Hello, World!");
        let encoded = original.encode();
        let (decoded, consumed) = Message::decode(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded.tag(), original.tag());
        assert_eq!(decoded.payload(), original.payload());
    }

    #[test]
    fn test_message_decode_short_buffer() {
        assert!(Message::decode(&[0u8; 3]).is_none()); // not enough for length
        assert!(Message::decode(&[5, 0, 0, 0]).is_none()); // not enough for body
    }

    #[test]
    fn test_message_decode_invalid_length() {
        // Length says 0
        assert!(Message::decode(&[0, 0, 0, 0]).is_none());
    }

    #[test]
    fn test_message_string() {
        let msg = Message::string(1, "test");
        assert_eq!(msg.as_str(), Some("test"));
    }

    #[test]
    fn test_message_wire_size() {
        let msg = Message::new(1, b"1234");
        // 4 bytes length + 1 byte tag + 4 bytes payload = 9
        assert_eq!(msg.wire_size(), 9);
    }

    #[test]
    fn test_message_read_write_stream() {
        let msg1 = Message::new(1, b"msg1");
        let msg2 = Message::new(2, b"msg2_longer");

        let mut buf = Vec::new();
        msg1.write_to(&mut buf).unwrap();
        msg2.write_to(&mut buf).unwrap();

        // Read back from the buffer
        let mut cursor = std::io::Cursor::new(&buf);
        let decoded1 = Message::read_from(&mut cursor).unwrap().unwrap();
        let decoded2 = Message::read_from(&mut cursor).unwrap().unwrap();

        assert_eq!(decoded1, msg1);
        assert_eq!(decoded2, msg2);
    }

    #[test]
    fn test_message_read_from_empty() {
        let mut empty = std::io::Cursor::new(&[]);
        assert!(Message::read_from(&mut empty).unwrap().is_none());
    }

    #[test]
    fn test_message_from_serialize() {
        let msg = Message::from_serialize(5, |buf| {
            buf.extend_from_slice(&42u32.to_le_bytes());
            buf.push(b'!');
        });
        assert_eq!(msg.tag(), 5);
        assert_eq!(msg.payload(), &[42, 0, 0, 0, b'!']);
    }

    #[test]
    fn test_message_clone_eq() {
        let a = Message::new(10, b"data");
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_message_into_payload() {
        let msg = Message::new(3, b"payload");
        assert_eq!(msg.into_payload(), b"payload");
    }

    #[test]
    fn test_message_max_payload() {
        let data = vec![0u8; MAX_PAYLOAD];
        let msg = Message::new(255, &data);
        let encoded = msg.encode();
        let (decoded, _) = Message::decode(&encoded).unwrap();
        assert_eq!(decoded.payload().len(), MAX_PAYLOAD);
    }

    #[test]
    fn test_multiple_messages_in_buffer() {
        let msg1 = Message::new(1, b"first");
        let msg2 = Message::new(2, b"second");
        let msg3 = Message::new(3, b"third");

        let mut buf = Vec::new();
        buf.extend_from_slice(&msg1.encode());
        buf.extend_from_slice(&msg2.encode());
        buf.extend_from_slice(&msg3.encode());

        let (d1, c1) = Message::decode(&buf).unwrap();
        assert_eq!(d1, msg1);
        let (d2, c2) = Message::decode(&buf[c1..]).unwrap();
        assert_eq!(d2, msg2);
        let (d3, c3) = Message::decode(&buf[c1 + c2..]).unwrap();
        assert_eq!(d3, msg3);
    }
}
