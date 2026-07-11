//! # UDP Peer-to-Peer Networking
//!
//! Simple UDP messaging for games — connectionless, best-effort.
//! Ideal for fast-paced games where occasional packet loss is acceptable.
//!
//! ## Usage
//!
//! ```no_run
//! use minix_net::UdpPeer;
//!
//! let mut peer = UdpPeer::bind("0.0.0.0:12345").unwrap();
//! peer.send_to(b"hello", "127.0.0.1:12346").unwrap();
//!
//! let (data, addr) = peer.recv_from().unwrap().unwrap();
//! println!("Got {} bytes from {}", data.len(), addr);
//! ```

use std::io::{self, Read};
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::os::unix::io::AsRawFd;
use std::time::Duration;

/// A non-blocking UDP peer for game networking.
///
/// Supports `send_to()` and `recv_from()` with optional timeout.
/// No built-in reliability — game code handles retransmission as needed.
pub struct UdpPeer {
    socket: UdpSocket,
    /// Receive timeout in milliseconds (0 = non-blocking).
    timeout_ms: u32,
    /// Temporary receive buffer.
    buf: [u8; 65535],
}

impl UdpPeer {
    /// Create a UDP socket bound to the given address.
    ///
    /// Use `"0.0.0.0:0"` for an ephemeral port.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            timeout_ms: 0,
            buf: [0u8; 65535],
        })
    }

    /// Set the receive timeout in milliseconds.
    /// 0 (default) = non-blocking (returns None immediately if no data).
    pub fn set_timeout(&mut self, ms: u32) {
        self.timeout_ms = ms;
    }

    /// Poll for incoming data using select().
    fn poll(&self) -> io::Result<bool> {
        unsafe {
            let fd = self.socket.as_raw_fd();
            let mut read_fds: libc::fd_set = std::mem::zeroed();
            libc::FD_SET(fd, &mut read_fds);

            let mut tv = libc::timeval {
                tv_sec: (self.timeout_ms / 1000) as libc::time_t,
                tv_usec: ((self.timeout_ms % 1000) * 1000) as libc::suseconds_t,
            };

            let ret = libc::select(
                fd + 1,
                &mut read_fds,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut tv,
            );

            if ret < 0 {
                return Err(io::Error::last_os_error());
            }

            Ok(libc::FD_ISSET(fd, &read_fds))
        }
    }

    /// Send data to a remote address.
    pub fn send_to<A: ToSocketAddrs>(&self, data: &[u8], addr: A) -> io::Result<usize> {
        self.socket.send_to(data, addr)
    }

    /// Receive a datagram and the sender's address.
    ///
    /// Returns `None` if no data is available (non-blocking or timeout).
    pub fn recv_from(&mut self) -> io::Result<Option<(Vec<u8>, SocketAddr)>> {
        if !self.poll()? {
            return Ok(None);
        }

        match self.socket.recv_from(&mut self.buf) {
            Ok((n, addr)) => Ok(Some((self.buf[..n].to_vec(), addr))),
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Send a framed message to a remote address.
    ///
    /// Prepends a length prefix and tag, then sends as one datagram.
    /// Use with `recv_message()` for structured messaging.
    pub fn send_message<A: ToSocketAddrs>(
        &self,
        tag: u8,
        payload: &[u8],
        addr: A,
    ) -> io::Result<usize> {
        let msg = crate::Message::new(tag, payload);
        self.send_to(&msg.encode(), addr)
    }

    /// Receive a framed message.
    ///
    /// Returns `None` if no datagram is available.
    pub fn recv_message(&mut self) -> io::Result<Option<(crate::Message, SocketAddr)>> {
        if let Some((data, addr)) = self.recv_from()? {
            if let Some((msg, _)) = crate::Message::decode(&data) {
                return Ok(Some((msg, addr)));
            }
        }
        Ok(None)
    }

    /// Get the local address of this peer.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Set the socket to broadcast mode.
    pub fn set_broadcast(&self, enabled: bool) -> io::Result<()> {
        self.socket.set_broadcast(enabled)
    }

    /// Send data to all peers on the local network (broadcast).
    ///
    /// Requires `set_broadcast(true)` first.
    pub fn broadcast_to(&self, data: &[u8], port: u16) -> io::Result<usize> {
        self.send_to(data, format!("255.255.255.255:{}", port))
    }

    /// Get the TTL for multicast packets.
    pub fn ttl(&self) -> io::Result<u32> {
        self.socket.ttl()
    }

    /// Set the TTL for multicast packets.
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.socket.set_ttl(ttl)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn find_port() -> u16 {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.local_addr().unwrap().port()
    }

    #[test]
    fn test_udp_peer_bind() {
        let port = find_port();
        let peer = UdpPeer::bind(format!("127.0.0.1:{}", port)).unwrap();
        assert!(peer.local_addr().unwrap().port() > 0);
    }

    #[test]
    fn test_udp_send_recv() {
        let port_a = find_port();
        let port_b = find_port();

        let mut peer_a = UdpPeer::bind(format!("127.0.0.1:{}", port_a)).unwrap();
        let mut peer_b = UdpPeer::bind(format!("127.0.0.1:{}", port_b)).unwrap();

        // A sends to B
        let sent = peer_a
            .send_to(b"hello from A", format!("127.0.0.1:{}", port_b))
            .unwrap();
        assert!(sent > 0);

        std::thread::sleep(Duration::from_millis(50));

        // B receives from A
        let (data, addr) = peer_b.recv_from().unwrap().unwrap();
        assert_eq!(data, b"hello from A");
        assert_eq!(addr.port(), port_a);
    }

    #[test]
    fn test_udp_no_data_nonblocking() {
        let port = find_port();
        let mut peer = UdpPeer::bind(format!("127.0.0.1:{}", port)).unwrap();
        assert!(peer.recv_from().unwrap().is_none());
    }

    #[test]
    fn test_udp_send_and_recv_message() {
        let port_a = find_port();
        let port_b = find_port();

        let peer_a = UdpPeer::bind(format!("127.0.0.1:{}", port_a)).unwrap();
        let mut peer_b = UdpPeer::bind(format!("127.0.0.1:{}", port_b)).unwrap();

        peer_a
            .send_message(42, b"msg payload", format!("127.0.0.1:{}", port_b))
            .unwrap();

        std::thread::sleep(Duration::from_millis(50));

        let (msg, addr) = peer_b.recv_message().unwrap().unwrap();
        assert_eq!(msg.tag(), 42);
        assert_eq!(msg.payload(), b"msg payload");
        assert_eq!(addr.port(), port_a);
    }

    #[test]
    fn test_udp_timeout() {
        let peer_a = UdpPeer::bind("127.0.0.1:0").unwrap();
        let mut peer_b = UdpPeer::bind("127.0.0.1:0").unwrap();
        peer_b.set_timeout(10); // 10ms timeout

        let start = std::time::Instant::now();
        let result = peer_b.recv_from().unwrap();
        let elapsed = start.elapsed();

        assert!(result.is_none());
        assert!(elapsed < Duration::from_millis(50)); // should return within ~10ms
    }

    #[test]
    fn test_udp_bidirectional() {
        let port_a = find_port();
        let port_b = find_port();

        let mut peer_a = UdpPeer::bind(format!("127.0.0.1:{}", port_a)).unwrap();
        let mut peer_b = UdpPeer::bind(format!("127.0.0.1:{}", port_b)).unwrap();

        // A → B
        peer_a
            .send_to(b"ping", format!("127.0.0.1:{}", port_b))
            .unwrap();

        std::thread::sleep(Duration::from_millis(50));

        // B receives from A
        let (data, addr) = peer_b.recv_from().unwrap().unwrap();
        assert_eq!(data, b"ping");

        // B → A (reply)
        peer_b
            .send_to(b"pong", addr)
            .unwrap();

        std::thread::sleep(Duration::from_millis(50));

        // A receives from B
        let (data, _) = peer_a.recv_from().unwrap().unwrap();
        assert_eq!(data, b"pong");
    }

    #[test]
    fn test_udp_broadcast_mode() {
        let mut peer = UdpPeer::bind("0.0.0.0:0").unwrap();
        peer.set_broadcast(true).unwrap();
        assert!(peer.set_broadcast(false).is_ok());
    }

    #[test]
    fn test_udp_ttl() {
        let mut peer = UdpPeer::bind("0.0.0.0:0").unwrap();
        peer.set_ttl(64).unwrap();
        assert_eq!(peer.ttl().unwrap(), 64);
    }
}
