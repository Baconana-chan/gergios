//! # Transport Abstraction
//!
//! Abstracts the underlying communication channel for Wayland messages.
//! On host platforms, we use Unix domain sockets. On MINIX, we use
//! the MINIX IPC mechanism.
//!
//! For host development on Windows (which lacks Unix sockets), we
//! provide an in-process ring-buffer transport for testing.

use alloc::rc::Rc;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::wire::{self, WaylandMessage};

/// Error during transport operations.
#[derive(Debug)]
pub enum TransportError {
    /// The connection was closed.
    ConnectionClosed,
    /// A message was too large to send/receive.
    MessageTooLarge,
    /// Wire format decoding failed.
    DecodeError(wire::DecodeError),
    /// I/O error (platform-specific).
    IoError,
}

/// Trait for Wayland message transport.
///
/// Implementations:
/// - `RingBufTransport` (in-process, for testing)
/// - `UnixTransport` (host Linux/Mac)
/// - `MinixIpcTransport` (MINIX target)
pub trait WaylandTransport {
    /// Send an encoded Wayland message.
    fn send(&mut self, msg: &WaylandMessage) -> Result<(), TransportError>;

    /// Receive a Wayland message (blocking).
    fn receive(&mut self) -> Result<WaylandMessage, TransportError>;

    /// Check if there are pending messages.
    fn has_pending(&self) -> bool;

    /// Flush any buffered data to the transport.
    fn flush(&mut self) -> Result<(), TransportError>;
}

// ── Connected Transport Pair (for testing) ───────────────────────────────

/// A connected pair of transports that link to each other.
///
/// Uses `Rc<RefCell<>>` for shared ownership so that both ends can be
/// moved independently while sharing the underlying buffers.
pub fn create_transport_pair() -> (RingBufTransport, RingBufTransport) {
    let buf_ab = Rc::new(RefCell::new(VecDeque::new()));
    let buf_ba = Rc::new(RefCell::new(VecDeque::new()));

    let server = RingBufTransport {
        send: buf_ba.clone(),
        recv: buf_ab.clone(),
    };
    let client = RingBufTransport {
        send: buf_ab,
        recv: buf_ba,
    };

    (server, client)
}

// ── Ring Buffer Transport (for testing) ──────────────────────────────────

/// In-process ring-buffer transport for testing.
///
/// Uses `Rc<RefCell<VecDeque>>` for the underlying buffers so that
/// a pair of transports can be connected (server's send → client's recv).
pub struct RingBufTransport {
    /// Buffer for outgoing messages (written by send, read by peer).
    send: Rc<RefCell<VecDeque<Vec<u8>>>>,
    /// Buffer for incoming messages (written by peer, read by receive).
    recv: Rc<RefCell<VecDeque<Vec<u8>>>>,
}

impl RingBufTransport {
    /// Create an unconnected transport with empty buffers.
    /// Use `create_transport_pair()` to get a connected pair.
    pub fn new() -> Self {
        Self {
            send: Rc::new(RefCell::new(VecDeque::new())),
            recv: Rc::new(RefCell::new(VecDeque::new())),
        }
    }

    /// Create a connected pair of transports (server ↔ client).
    #[deprecated(note = "use create_transport_pair() instead")]
    pub fn pair() -> (Self, Self) {
        create_transport_pair()
    }
}

impl WaylandTransport for RingBufTransport {
    fn send(&mut self, msg: &WaylandMessage) -> Result<(), TransportError> {
        let bytes = wire::encode(msg);
        self.send.borrow_mut().push_back(bytes);
        Ok(())
    }

    fn receive(&mut self) -> Result<WaylandMessage, TransportError> {
        let bytes = self.recv.borrow_mut()
            .pop_front()
            .ok_or(TransportError::ConnectionClosed)?;
        let (msg, _) = wire::decode(&bytes)
            .map_err(|e| TransportError::DecodeError(e))?;
        Ok(msg)
    }

    fn has_pending(&self) -> bool {
        !self.recv.borrow().is_empty()
    }

    fn flush(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::WaylandMessage;

    #[test]
    fn connected_transport_roundtrip() {
        let (mut server, mut client) = create_transport_pair();

        // Server sends a message
        let msg = WaylandMessage::new(1, 0).arg_string("hello");
        server.send(&msg).unwrap();
        server.flush().unwrap();

        // Client receives it
        assert!(client.has_pending());
        let received = client.receive().unwrap();
        assert_eq!(received.object_id, 1);
        assert_eq!(received.opcode, 0);
    }

    #[test]
    fn client_to_server() {
        let (mut server, mut client) = create_transport_pair();

        // Client sends a request
        let msg = WaylandMessage::new(1, 1).arg_uint(42);
        client.send(&msg).unwrap();

        // Server receives it
        assert!(server.has_pending());
        let received = server.receive().unwrap();
        assert_eq!(received.object_id, 1);
        assert_eq!(received.opcode, 1);
    }

    #[test]
    fn multiple_messages() {
        let (mut server, mut client) = create_transport_pair();

        for i in 0..5 {
            client.send(&WaylandMessage::new(1, i as u16)).unwrap();
        }

        for i in 0..5 {
            let received = server.receive().unwrap();
            assert_eq!(received.opcode, i as u16);
        }
    }

    #[test]
    fn empty_transport_has_no_pending() {
        let (server, _client) = create_transport_pair();
        let (unused, _) = create_transport_pair();
        assert!(!server.has_pending());
        assert!(!unused.has_pending());
    }
}
