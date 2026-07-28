//! # Wayland Wire Format — Message encoding and decoding
//!
//! Implements the Wayland wire protocol as defined in the Wayland
//! specification. The wire format is:
//!
//! ```text
//! [object_id: u32 LE] [opcode: u16 LE] [size: u16 LE] [arguments...]
//! ```
//!
//! - `object_id`: 32-bit object identifier (4 bytes, little-endian)
//! - `opcode`: 16-bit request/event opcode (2 bytes, little-endian)
//! - `size`: total message length in uint32_t words (2 bytes, little-endian)
//!   Minimum 2 (header alone = 8 bytes).
//!
//! All multi-byte values are little-endian. Strings and arrays are
//! length-prefixed and padded to 4-byte alignment.
//!
//! ## Argument types
//!
//! | Type     | Wire format                                   |
//! |----------|-----------------------------------------------|
//! | `int`    | i32 (4 bytes, little-endian)                  |
//! | `uint`   | u32 (4 bytes, little-endian)                  |
//! | `fixed`  | i32 (wl_fixed_t, i24.8 fixed-point)           |
//! | `string` | u32 length + UTF-8 data + nul + padding       |
//! | `object` | u32 (object ID)                               |
//! | `new_id` | u32 (new object ID) / u32 interface + version |
//! | `array`  | u32 length + data + padding                   |
//! | `fd`     | raw fd (Unix) / endpoint_t (MINIX)            |

#![allow(dead_code)]

use alloc::vec::Vec;
use core::fmt;

/// A single fixed-point number in the Wayland wire format.
///
/// wl_fixed_t is a signed 24.8 fixed-point number stored as an i32.
/// The integer part is the upper 24 bits, the fractional part is the
/// lower 8 bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fixed(pub i32);

impl Fixed {
    /// Create a Fixed from a floating-point value.
    pub fn from_f64(v: f64) -> Self {
        Fixed((v * 256.0) as i32)
    }

    /// Convert to f64.
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / 256.0
    }

    /// Create a Fixed from an integer (fraction = 0).
    pub fn from_int(v: i32) -> Self {
        Fixed(v << 8)
    }
}

/// Describes the type of a single Wayland wire-format argument.
///
/// Used as a signature for decoding raw argument bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    Int,
    Uint,
    Fixed,
    String,
    Object,
    NewId,
    Array,
    Fd,
}

/// An argument in a Wayland message.
#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    Int(i32),
    Uint(u32),
    Fixed(Fixed),
    String(Option<alloc::string::String>),  // None = NULL string
    Object(u32),
    NewId(u32),
    Array(Vec<u8>),
    Fd(i32),  // file descriptor or MINIX endpoint
}

/// A single Wayland message (request or event).
///
/// Every message has a 4-byte header followed by arguments.
/// The `object_id` identifies which protocol object this
/// message targets.
#[derive(Debug, Clone, PartialEq)]
pub struct WaylandMessage {
    /// The protocol object ID.
    pub object_id: u32,
    /// The opcode — identifies which request/event this is.
    pub opcode: u16,
    /// Decoded arguments (empty until `decode_args_with` is called).
    pub args: Vec<Arg>,
    /// Raw argument bytes for lazy decoding with a signature.
    pub raw_args: Vec<u8>,
}

impl WaylandMessage {
    /// Create a new Wayland message.
    pub fn new(object_id: u32, opcode: u16) -> Self {
        Self {
            object_id,
            opcode,
            args: Vec::new(),
            raw_args: Vec::new(),
        }
    }

    /// Add an argument.
    pub fn arg(mut self, arg: Arg) -> Self {
        self.args.push(arg);
        self
    }

    /// Convenience: add an int argument.
    pub fn arg_int(self, v: i32) -> Self {
        self.arg(Arg::Int(v))
    }

    /// Convenience: add a uint argument.
    pub fn arg_uint(self, v: u32) -> Self {
        self.arg(Arg::Uint(v))
    }

    /// Convenience: add an object ID argument.
    pub fn arg_object(self, v: u32) -> Self {
        self.arg(Arg::Object(v))
    }

    /// Convenience: add a new_id argument.
    pub fn arg_new_id(self, v: u32) -> Self {
        self.arg(Arg::NewId(v))
    }

    /// Convenience: add a string argument.
    pub fn arg_string(self, s: &str) -> Self {
        self.arg(Arg::String(Some(alloc::string::String::from(s))))
    }

    /// Convenience: add a fixed argument.
    pub fn arg_fixed(self, v: Fixed) -> Self {
        self.arg(Arg::Fixed(v))
    }

    /// Convenience: add an array argument.
    pub fn arg_array(self, data: Vec<u8>) -> Self {
        self.arg(Arg::Array(data))
    }

    /// Convenience: add an fd argument.
    pub fn arg_fd(self, fd: i32) -> Self {
        self.arg(Arg::Fd(fd))
    }

    /// Decode the raw argument bytes using the given type signature.
    ///
    /// Panics if the raw data doesn't match the expected types.
    pub fn decode_args_with(&mut self, types: &[ArgType]) -> Result<(), DecodeError> {
        self.args = decode_args_from_sig(&self.raw_args, types)?;
        Ok(())
    }
}

// ── Encoder ──────────────────────────────────────────────────────────────

/// Encode a Wayland message into its wire format byte representation.
///
/// Returns the raw bytes ready to be sent over the transport.
///
/// The Wayland wire format header is:
/// ```text
/// bytes 0-3: object_id (u32 LE)
/// bytes 4-5: opcode (u16 LE)
/// bytes 6-7: size in uint32_t words (u16 LE)
/// bytes 8+: arguments …
/// ```
pub fn encode(msg: &WaylandMessage) -> Vec<u8> {
    // First pass: compute total byte size
    let mut byte_size = 8; // header = 8 bytes (2 words)
    for arg in &msg.args {
        byte_size += arg_wire_size(arg);
    }

    let word_size = (byte_size / 4) as u16;  // size in units of 4 bytes

    let mut buf = Vec::with_capacity(byte_size);

    // Bytes 0-3: object_id (u32 LE)
    buf.extend_from_slice(&msg.object_id.to_le_bytes());

    // Bytes 4-5: opcode (u16 LE)
    buf.extend_from_slice(&msg.opcode.to_le_bytes());

    // Bytes 6-7: size (u16 LE) — total message length in uint32_t words
    buf.extend_from_slice(&word_size.to_le_bytes());

    // Arguments
    for arg in &msg.args {
        encode_arg(arg, &mut buf);
    }

    debug_assert_eq!(buf.len(), byte_size, "encoded size mismatch");
    buf
}

/// Compute the wire size of an argument (including padding).
fn arg_wire_size(arg: &Arg) -> usize {
    match arg {
        Arg::Int(_) | Arg::Uint(_) | Arg::Fixed(_) | Arg::Object(_) | Arg::NewId(_) | Arg::Fd(_) => {
            4 // all 4-byte scalars
        }
        Arg::String(None) => 4, // NULL string: just length 0
        Arg::String(Some(s)) => {
            // u32 length (including nul) + string data + nul + padding
            let len = s.len() + 1; // +1 for nul terminator
            4 + align4(len)
        }
        Arg::Array(data) => {
            // u32 length + data + padding
            4 + align4(data.len())
        }
    }
}

/// Encode a single argument into the buffer.
fn encode_arg(arg: &Arg, buf: &mut Vec<u8>) {
    match arg {
        Arg::Int(v) => buf.extend_from_slice(&v.to_le_bytes()),
        Arg::Uint(v) => buf.extend_from_slice(&v.to_le_bytes()),
        Arg::Fixed(v) => buf.extend_from_slice(&v.0.to_le_bytes()),
        Arg::Object(v) | Arg::NewId(v) => buf.extend_from_slice(&v.to_le_bytes()),
        Arg::Fd(v) => buf.extend_from_slice(&v.to_le_bytes()),
        Arg::String(None) => {
            // NULL string: length = 0
            buf.extend_from_slice(&0u32.to_le_bytes());
        }
        Arg::String(Some(s)) => {
            let len = s.len() + 1; // include nul terminator
            buf.extend_from_slice(&(len as u32).to_le_bytes());
            buf.extend_from_slice(s.as_bytes());
            buf.push(0); // nul terminator
            // Pad to 4 bytes
            let padded = align4(len);
            for _ in len..padded {
                buf.push(0);
            }
        }
        Arg::Array(data) => {
            buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
            buf.extend_from_slice(data);
            // Pad to 4 bytes
            let padded = align4(data.len());
            for _ in data.len()..padded {
                buf.push(0);
            }
        }
    }
}

/// Round up to the next multiple of 4.
fn align4(n: usize) -> usize {
    (n + 3) & !3
}

// ── Decoder ──────────────────────────────────────────────────────────────

/// Error during Wayland message decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// Buffer too small for header.
    TruncatedHeader,
    /// Message size in header is less than minimum (2 words = 8 bytes).
    InvalidSize(u16),
    /// Buffer is shorter than declared message size.
    TruncatedBody { declared: u16, actual: usize },
    /// Invalid string: unterminated or length mismatch.
    InvalidString,
    /// Ran out of data while decoding arguments.
    ArgUnderflow,
    /// Unknown argument type discriminant (shouldn't happen with our encoder).
    UnknownType,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedHeader => write!(f, "truncated header"),
            Self::InvalidSize(s) => write!(f, "invalid message size: {}", s),
            Self::TruncatedBody { declared, actual } => {
                write!(f, "truncated body: declared {} words, have {} bytes", declared, actual)
            }
            Self::InvalidString => write!(f, "invalid string"),
            Self::ArgUnderflow => write!(f, "argument underflow"),
            Self::UnknownType => write!(f, "unknown argument type"),
        }
    }
}

/// Decode a Wayland message from raw bytes.
///
/// Returns the decoded message and the number of bytes consumed.
///
/// Header layout:
/// ```text
/// bytes 0-3: object_id (u32 LE)
/// bytes 4-5: opcode (u16 LE)
/// bytes 6-7: size in uint32_t words (u16 LE)
/// ```
pub fn decode(buf: &[u8]) -> Result<(WaylandMessage, usize), DecodeError> {
    if buf.len() < 8 {
        return Err(DecodeError::TruncatedHeader);
    }

    // Bytes 0-3: object_id (u32 LE)
    let object_id = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);

    // Bytes 4-5: opcode (u16 LE)
    let opcode = u16::from_le_bytes([buf[4], buf[5]]);

    // Bytes 6-7: size (u16 LE) — in uint32_t words
    let size_words = u16::from_le_bytes([buf[6], buf[7]]);
    let total_bytes = (size_words as usize) * 4;

    if size_words < 2 {
        return Err(DecodeError::InvalidSize(size_words));
    }
    if buf.len() < total_bytes {
        return Err(DecodeError::TruncatedBody {
            declared: size_words,
            actual: buf.len(),
        });
    }

    // Store raw argument bytes for lazy decoding
    let arg_bytes = total_bytes - 8;
    let raw_args = buf[8..8 + arg_bytes].to_vec();

    Ok((WaylandMessage {
        object_id,
        opcode,
        args: Vec::new(),
        raw_args,
    }, total_bytes))
}

/// Decode arguments from raw bytes using a type signature.
fn decode_args_from_sig(buf: &[u8], types: &[ArgType]) -> Result<Vec<Arg>, DecodeError> {
    let mut args = Vec::with_capacity(types.len());
    let mut offset = 0;

    for &arg_type in types {
        match arg_type {
            ArgType::Int | ArgType::Uint | ArgType::Fixed | ArgType::Object | ArgType::NewId | ArgType::Fd => {
                if offset + 4 > buf.len() {
                    return Err(DecodeError::ArgUnderflow);
                }
                let val = u32::from_le_bytes([
                    buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3],
                ]);
                offset += 4;
                args.push(match arg_type {
                    ArgType::Int => Arg::Int(val as i32),
                    ArgType::Uint => Arg::Uint(val),
                    ArgType::Fixed => Arg::Fixed(Fixed(val as i32)),
                    ArgType::Object => Arg::Object(val),
                    ArgType::NewId => Arg::NewId(val),
                    ArgType::Fd => Arg::Fd(val as i32),
                    _ => unreachable!(),
                });
            }
            ArgType::String => {
                if offset + 4 > buf.len() {
                    return Err(DecodeError::ArgUnderflow);
                }
                let len = u32::from_le_bytes([
                    buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3],
                ]);
                offset += 4;

                if len == 0 {
                    // NULL string
                    args.push(Arg::String(None));
                } else {
                    let string_len = len as usize;
                    if offset + string_len > buf.len() {
                        return Err(DecodeError::InvalidString);
                    }
                    // String length includes the nul terminator
                    let actual_str_len = string_len.saturating_sub(1);
                    let s = core::str::from_utf8(&buf[offset..offset + actual_str_len])
                        .map_err(|_| DecodeError::InvalidString)?;
                    offset += string_len;
                    // Skip padding
                    let padded = align4(string_len);
                    offset += padded - string_len;
                    args.push(Arg::String(Some(alloc::string::String::from(s))));
                }
            }
            ArgType::Array => {
                if offset + 4 > buf.len() {
                    return Err(DecodeError::ArgUnderflow);
                }
                let len = u32::from_le_bytes([
                    buf[offset], buf[offset + 1], buf[offset + 2], buf[offset + 3],
                ]);
                offset += 4;

                let arr_len = len as usize;
                if offset + arr_len > buf.len() {
                    return Err(DecodeError::ArgUnderflow);
                }
                let data = buf[offset..offset + arr_len].to_vec();
                offset += arr_len;
                // Skip padding
                let padded = align4(arr_len);
                offset += padded - arr_len;
                args.push(Arg::Array(data));
            }
        }
    }

    Ok(args)
}

// ── Fixed-point helpers ─────────────────────────────────────────────────

/// Convert a wl_fixed_t to a floating-point value.
pub fn wl_fixed_to_double(v: i32) -> f64 {
    Fixed(v).to_f64()
}

/// Convert a floating-point value to wl_fixed_t.
pub fn wl_fixed_from_double(v: f64) -> i32 {
    Fixed::from_f64(v).0
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_empty_message() {
        let msg = WaylandMessage::new(1, 0);
        let bytes = encode(&msg);
        // header: object_id=1(4B), opcode=0(2B), size=2(2B) = 8 bytes
        assert_eq!(bytes.len(), 8);
        // bytes 0-3: object_id = 1 LE
        assert_eq!(&bytes[0..4], [1, 0, 0, 0]);
        // bytes 4-5: opcode = 0
        assert_eq!(&bytes[4..6], [0, 0]);
        // bytes 6-7: size = 2 words
        assert_eq!(&bytes[6..8], [2, 0]);
    }

    #[test]
    fn encode_int_arg() {
        let msg = WaylandMessage::new(42, 3)
            .arg_int(-1)
            .arg_uint(0xFF);
        let bytes = encode(&msg);
        // header(8) + int(4) + uint(4) = 16 bytes = 4 words
        assert_eq!(bytes.len(), 16);
        // bytes 0-3: object_id = 42 LE
        assert_eq!(&bytes[0..4], [42, 0, 0, 0]);
        // bytes 4-5: opcode = 3
        assert_eq!(&bytes[4..6], [3, 0]);
        // bytes 6-7: size = 4 words
        assert_eq!(&bytes[6..8], [4, 0]);
        // bytes 8-11: int(-1) = 0xFFFFFFFF
        assert_eq!(&bytes[8..12], [0xFF, 0xFF, 0xFF, 0xFF]);
        // bytes 12-15: uint(0xFF) = 0x000000FF
        assert_eq!(&bytes[12..16], [0xFF, 0x00, 0x00, 0x00]);
    }

    #[test]
    fn encode_string_arg() {
        let msg = WaylandMessage::new(1, 1)
            .arg_string("Hi");
        let bytes = encode(&msg);
        // header(8) + length(4) + "Hi\0"(3) + padding(1) = 16 bytes = 4 words
        assert_eq!(bytes.len(), 16);
        // length = 3 (includes nul)
        assert_eq!(&bytes[8..12], [3, 0, 0, 0]);  // length = 3
        assert_eq!(&bytes[12..15], [b'H', b'i', 0]);  // "Hi\0"
        assert_eq!(bytes[15], 0);  // padding
    }

    #[test]
    fn encode_null_string() {
        let msg = WaylandMessage::new(1, 1)
            .arg(Arg::String(None));
        let bytes = encode(&msg);
        // header(8) + length(4) = 12 bytes = 3 words
        assert_eq!(bytes.len(), 12);
        assert_eq!(&bytes[8..12], [0, 0, 0, 0]);  // length = 0
    }

    #[test]
    fn encode_object_and_new_id() {
        let msg = WaylandMessage::new(2, 0)
            .arg_object(5)
            .arg_new_id(6);
        let bytes = encode(&msg);
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[8..12], [5, 0, 0, 0]); // object 5
        assert_eq!(&bytes[12..16], [6, 0, 0, 0]); // new_id 6
    }

    #[test]
    fn encode_array() {
        let msg = WaylandMessage::new(1, 0)
            .arg_array(alloc::vec![1u8, 2, 3]);
        let bytes = encode(&msg);
        // header(8) + length(4) + data(3) + padding(1) = 16 bytes
        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[8..12], [3, 0, 0, 0]);  // length = 3
        assert_eq!(&bytes[12..15], [1, 2, 3]);      // data
        assert_eq!(bytes[15], 0);                    // padding
    }

    #[test]
    fn encode_fixed() {
        let msg = WaylandMessage::new(1, 0)
            .arg_fixed(Fixed::from_f64(1.5));
        let bytes = encode(&msg);
        // 1.5 * 256 = 384 = 0x180
        assert_eq!(&bytes[8..12], [0x80, 0x01, 0x00, 0x00]);
        // 384 LE = 0x80, 0x01, 0x00, 0x00
        assert_eq!(Fixed::from_f64(1.5).0, 384);
        assert!((Fixed(384).to_f64() - 1.5).abs() < 0.01);
    }

    #[test]
    fn decode_header_only() {
        let msg = WaylandMessage::new(7, 2);
        let bytes = encode(&msg);
        let (decoded, consumed) = decode(&bytes).unwrap();
        assert_eq!(decoded.object_id, 7);
        assert_eq!(decoded.opcode, 2);
        assert_eq!(decoded.args.len(), 0);
        assert_eq!(consumed, 8);
    }

    #[test]
    fn decode_invalid_size() {
        // Size less than 2 words
        let buf = [1, 0, 0, 0, 0, 0, 1, 0]; // object_id=1, opcode=0, size=1
        assert_eq!(decode(&buf), Err(DecodeError::InvalidSize(1)));
    }

    #[test]
    fn decode_truncated() {
        // Only 7 bytes — less than 8-byte header minimum
        let buf = [1, 0, 0, 0, 0, 0, 2];
        assert_eq!(decode(&buf), Err(DecodeError::TruncatedHeader));
    }

    #[test]
    fn fixed_roundtrip() {
        let vals = [0.0, 1.0, -1.0, 3.14159, 100.5, -0.5];
        for &v in &vals {
            let f = Fixed::from_f64(v);
            let back = f.to_f64();
            assert!((back - v).abs() < 0.01, "fixed roundtrip failed for {}", v);
        }
    }

    #[test]
    fn fixed_from_int() {
        assert_eq!(Fixed::from_int(42).0, 42 << 8);
        assert_eq!(Fixed::from_int(42).to_f64(), 42.0);
    }

    #[test]
    fn decode_args_with_signature() {
        // Encode a message with args: uint(42), int(-1), string("hello")
        let msg = WaylandMessage::new(1, 0)
            .arg_uint(42)
            .arg_int(-1)
            .arg_string("hello");
        let bytes = encode(&msg);

        // Decode raw
        let (mut decoded, _) = decode(&bytes).unwrap();
        assert!(decoded.args.is_empty());
        assert!(!decoded.raw_args.is_empty());

        // Now decode with signature
        decoded.decode_args_with(&[ArgType::Uint, ArgType::Int, ArgType::String]).unwrap();
        assert_eq!(decoded.args.len(), 3);
        assert_eq!(decoded.args[0], Arg::Uint(42));
        assert_eq!(decoded.args[1], Arg::Int(-1));
        assert_eq!(decoded.args[2], Arg::String(Some("hello".into())));
    }

    #[test]
    fn decode_args_new_id() {
        let msg = WaylandMessage::new(1, 0).arg_new_id(100);
        let bytes = encode(&msg);
        let (mut decoded, _) = decode(&bytes).unwrap();

        decoded.decode_args_with(&[ArgType::NewId]).unwrap();
        assert_eq!(decoded.args[0], Arg::NewId(100));
    }

    #[test]
    fn decode_args_object() {
        let msg = WaylandMessage::new(1, 0).arg_object(42).arg_object(0);
        let bytes = encode(&msg);
        let (mut decoded, _) = decode(&bytes).unwrap();

        decoded.decode_args_with(&[ArgType::Object, ArgType::Object]).unwrap();
        assert_eq!(decoded.args[0], Arg::Object(42));
        assert_eq!(decoded.args[1], Arg::Object(0));
    }

    #[test]
    fn decode_args_null_string() {
        let msg = WaylandMessage::new(1, 0).arg(Arg::String(None));
        let bytes = encode(&msg);
        let (mut decoded, _) = decode(&bytes).unwrap();

        decoded.decode_args_with(&[ArgType::String]).unwrap();
        assert_eq!(decoded.args[0], Arg::String(None));
    }

    #[test]
    fn align4_works() {
        assert_eq!(align4(0), 0);
        assert_eq!(align4(1), 4);
        assert_eq!(align4(3), 4);
        assert_eq!(align4(4), 4);
        assert_eq!(align4(5), 8);
    }
}
