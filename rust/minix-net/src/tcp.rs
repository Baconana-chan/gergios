//! # TCP Game Networking
//!
//! Simple TCP server and client for games with message-based protocol.
//!
//! ## Server
//!
//! ```no_run
//! let mut server = NetServer::bind("0.0.0.0:12345")?;
//! server.listen(4)?;
//!
//! loop {
//!     match server.recv()? {
//!         Some(ServerEvent::Message(client_id, msg)) => {
//!             server.send(client_id, &msg)?;
//!         }
//!         Some(ServerEvent::Connected(client_id)) => {
//!             println!("Client {} connected", client_id);
//!         }
//!         Some(ServerEvent::Disconnected(client_id)) => {
//!             println!("Client {} disconnected", client_id);
//!         }
//!         None => {} // timeout, no events
//!     }
//! }
//! ```
//!
//! ## Client
//!
//! ```no_run
//! let mut client = NetClient::connect("127.0.0.1:12345")?;
//! client.send(&Message::new(1, b"hello"))?;
//!
//! if let Some(msg) = client.recv()? {
//!     println!("Got: {:?}", msg);
//! }
//! ```

use std::collections::{HashMap, VecDeque};
use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream, SocketAddr, ToSocketAddrs};
use std::os::unix::io::{AsRawFd, RawFd};
use std::time::Duration;

use crate::Message;

/// Client identifier (index into the server's client list).
pub type ClientId = usize;

/// Events that a server can receive.
#[derive(Clone, Debug)]
pub enum ServerEvent {
    /// A new client connected.
    Connected(ClientId),
    /// A message from a client.
    Message(ClientId, Message),
    /// A client disconnected.
    Disconnected(ClientId),
}

// ============================================================================
// TCP Server
// ============================================================================

/// A non-blocking TCP game server.
///
/// Accepts clients, buffers incoming messages per client,
/// and provides `send()`, `broadcast()`, and `recv()` methods.
pub struct NetServer {
    /// TCP listener.
    listener: TcpListener,
    /// Connected clients and their receive buffers.
    clients: HashMap<ClientId, ClientState>,
    /// Next client ID to assign.
    next_id: ClientId,
    /// Pending events to deliver.
    events: VecDeque<ServerEvent>,
    /// Timeout for recv() (0 = non-blocking).
    timeout_ms: u32,
}

struct ClientState {
    stream: TcpStream,
    buffer: Vec<u8>,
    peer_addr: SocketAddr,
}

impl NetServer {
    /// Create a server bound to the given address.
    ///
    /// Does NOT start listening yet — call `listen()`.
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        Ok(Self {
            listener,
            clients: HashMap::new(),
            next_id: 0,
            events: VecDeque::new(),
            timeout_ms: 0,
        })
    }

    /// Start listening for incoming connections.
    ///
    /// `backlog`: maximum pending connections.
    pub fn listen(&mut self, backlog: u32) -> io::Result<()> {
        // TcpListener is already bound; non-blocking mode is handled by poll
        self.listener
            .set_nonblocking(true)?;
        // Backlog is already set by bind() on most systems, but we store it
        let _ = backlog;
        Ok(())
    }

    /// Set the receive timeout in milliseconds.
    /// 0 (default) = non-blocking.
    pub fn set_timeout(&mut self, ms: u32) {
        self.timeout_ms = ms;
    }

    /// Accept any pending connections.
    fn accept_pending(&mut self) -> io::Result<()> {
        loop {
            match self.listener.accept() {
                Ok((stream, addr)) => {
                    stream.set_nonblocking(true)?;
                    let id = self.next_id;
                    self.next_id += 1;
                    self.clients.insert(
                        id,
                        ClientState {
                            stream,
                            buffer: Vec::new(),
                            peer_addr: addr,
                        },
                    );
                    self.events.push_back(ServerEvent::Connected(id));
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    break; // no more pending connections
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Read data from all clients and buffer it.
    fn read_from_clients(&mut self) -> io::Result<()> {
        let mut disconnected = Vec::new();

        for (&id, state) in &mut self.clients {
            // Read available data into a temp buffer
            let mut tmp = [0u8; 4096];
            loop {
                match state.stream.read(&mut tmp) {
                    Ok(0) => {
                        // Connection closed
                        disconnected.push(id);
                        break;
                    }
                    Ok(n) => {
                        state.buffer.extend_from_slice(&tmp[..n]);
                    }
                    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        break; // no more data from this client
                    }
                    Err(e) => return Err(e),
                }
            }
        }

        // Process disconnected clients
        for id in disconnected {
            if let Some(state) = self.clients.remove(&id) {
                self.events.push_back(ServerEvent::Disconnected(id));
            }
        }

        Ok(())
    }

    /// Parse complete messages from client buffers and queue them as events.
    fn parse_messages(&mut self) {
        let mut to_remove = Vec::new();

        for (&id, state) in &mut self.clients {
            loop {
                match Message::decode(&state.buffer) {
                    Some((msg, consumed)) => {
                        state.buffer.drain(..consumed);
                        self.events
                            .push_back(ServerEvent::Message(id, msg));
                    }
                    None => break, // incomplete message
                }
            }
        }

        // Remove disconnected clients
        for id in to_remove {
            self.clients.remove(&id);
        }
    }

    /// Receive the next event (message, connect, disconnect).
    ///
    /// Returns `None` if no events are pending and the timeout expires
    /// (in non-blocking mode, returns immediately).
    pub fn recv(&mut self) -> io::Result<Option<ServerEvent>> {
        // First, drain any already-parsed events
        if let Some(event) = self.events.pop_front() {
            return Ok(Some(event));
        }

        // Poll for new data/connections using select()
        self.poll()?;

        // Accept new connections
        self.accept_pending()?;

        // Read data from clients
        self.read_from_clients()?;

        // Parse messages from buffered data
        self.parse_messages();

        // Return the first event, if any
        Ok(self.events.pop_front())
    }

    /// Poll for activity on listener and all clients.
    fn poll(&self) -> io::Result<()> {
        unsafe {
            let listener_fd = self.listener.as_raw_fd();
            let mut max_fd = listener_fd;

            // Build fd sets
            let mut read_fds: libc::fd_set = std::mem::zeroed();
            libc::FD_SET(listener_fd, &mut read_fds);

            for state in self.clients.values() {
                let fd = state.stream.as_raw_fd();
                libc::FD_SET(fd, &mut read_fds);
                if fd > max_fd {
                    max_fd = fd;
                }
            }

            let mut tv = libc::timeval {
                tv_sec: (self.timeout_ms / 1000) as libc::time_t,
                tv_usec: ((self.timeout_ms % 1000) * 1000) as libc::suseconds_t,
            };

            let ret = libc::select(
                max_fd + 1,
                &mut read_fds,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut tv,
            );

            if ret < 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(())
    }

    /// Send a message to a specific client.
    pub fn send(&mut self, client_id: ClientId, msg: &Message) -> io::Result<()> {
        if let Some(state) = self.clients.get(&client_id) {
            let encoded = msg.encode();
            let mut stream = &state.stream;
            stream.write_all(&encoded)?;
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "client not found",
            ))
        }
    }

    /// Send a message to all connected clients.
    pub fn broadcast(&mut self, msg: &Message) -> io::Result<()> {
        let encoded = msg.encode();
        let mut disconnected = Vec::new();

        for (&id, state) in &mut self.clients {
            match state.stream.write_all(&encoded) {
                Ok(()) => {}
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // Would block — try again later
                }
                Err(_) => {
                    disconnected.push(id);
                }
            }
        }

        for id in disconnected {
            self.clients.remove(&id);
            self.events.push_back(ServerEvent::Disconnected(id));
        }

        Ok(())
    }

    /// Send a message to all clients except one.
    pub fn broadcast_except(&mut self, exclude: ClientId, msg: &Message) -> io::Result<()> {
        let encoded = msg.encode();
        let mut disconnected = Vec::new();

        for (&id, state) in &mut self.clients {
            if id == exclude {
                continue;
            }
            match state.stream.write_all(&encoded) {
                Ok(()) => {}
                Err(_) => {
                    disconnected.push(id);
                }
            }
        }

        for id in disconnected {
            self.clients.remove(&id);
            self.events.push_back(ServerEvent::Disconnected(id));
        }

        Ok(())
    }

    /// Get the number of connected clients.
    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    /// Get the peer address of a client.
    pub fn client_addr(&self, client_id: ClientId) -> Option<SocketAddr> {
        self.clients.get(&client_id).map(|s| s.peer_addr)
    }

    /// Disconnect a client.
    pub fn disconnect(&mut self, client_id: ClientId) {
        self.clients.remove(&client_id);
        self.events.push_back(ServerEvent::Disconnected(client_id));
    }

    /// Get the local address of the server.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.listener.local_addr()
    }
}

// ============================================================================
// TCP Client
// ============================================================================

/// A non-blocking TCP game client.
///
/// Connects to a `NetServer` and provides `send()`/`recv()` for messages.
pub struct NetClient {
    /// TCP connection to server.
    stream: TcpStream,
    /// Receive buffer for incomplete messages.
    buffer: Vec<u8>,
    /// Receive timeout.
    timeout_ms: u32,
}

impl NetClient {
    /// Connect to a game server.
    pub fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_nonblocking(true)?;
        Ok(Self {
            stream,
            buffer: Vec::new(),
            timeout_ms: 0,
        })
    }

    /// Set the receive timeout in milliseconds.
    /// 0 (default) = non-blocking.
    pub fn set_timeout(&mut self, ms: u32) {
        self.timeout_ms = ms;
    }

    /// Poll for available data using select().
    fn poll(&self) -> io::Result<bool> {
        unsafe {
            let fd = self.stream.as_raw_fd();
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

    /// Send a message to the server.
    pub fn send(&mut self, msg: &Message) -> io::Result<()> {
        let encoded = msg.encode();
        self.stream.write_all(&encoded)
    }

    /// Receive a message from the server.
    ///
    /// Returns `None` if no message is available (non-blocking or timeout).
    pub fn recv(&mut self) -> io::Result<Option<Message>> {
        // First check if we already have a complete message in the buffer
        if let Some((msg, _)) = Message::decode(&self.buffer) {
            // Remove consumed bytes
            // We need to drain by wire size; re-decode to get wire size
            if let Some((_, consumed)) = Message::decode(&self.buffer) {
                self.buffer.drain(..consumed);
            }
            return Ok(Some(msg));
        }

        // Poll for data
        if !self.poll()? {
            return Ok(None); // timeout or no data
        }

        // Read available data
        let mut tmp = [0u8; 4096];
        loop {
            match self.stream.read(&mut tmp) {
                Ok(0) => {
                    // Connection closed by server
                    return Ok(None);
                }
                Ok(n) => {
                    self.buffer.extend_from_slice(&tmp[..n]);
                    // Check if we have a complete message now
                    if let Some((msg, consumed)) = Message::decode(&self.buffer) {
                        self.buffer.drain(..consumed);
                        return Ok(Some(msg));
                    }
                    // Try reading more
                    continue;
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    break; // no more data right now
                }
                Err(e) => return Err(e),
            }
        }

        // Try one more time with whatever we have
        if let Some((msg, consumed)) = Message::decode(&self.buffer) {
            self.buffer.drain(..consumed);
            return Ok(Some(msg));
        }

        Ok(None)
    }

    /// Check if the connection is still alive.
    pub fn is_connected(&self) -> bool {
        self.stream.peer_addr().is_ok()
    }

    /// Get the peer address.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.stream.peer_addr()
    }

    /// Get the local address.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.stream.local_addr()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Find an available port by binding to port 0.
    fn find_port() -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    }

    #[test]
    fn test_server_bind_and_listen() {
        let port = find_port();
        let mut server = NetServer::bind(format!("127.0.0.1:{}", port)).unwrap();
        server.listen(4).unwrap();
        assert_eq!(server.client_count(), 0);
    }

    #[test]
    fn test_client_connect_and_send() {
        let port = find_port();
        let mut server = NetServer::bind(format!("127.0.0.1:{}", port)).unwrap();
        server.listen(4).unwrap();

        let mut client = NetClient::connect(format!("127.0.0.1:{}", port)).unwrap();

        // Give server time to accept
        std::thread::sleep(Duration::from_millis(50));

        // Let the server process events
        let event = server.recv().unwrap();
        assert!(matches!(event, Some(ServerEvent::Connected(_))));

        // Client sends a message
        let msg = Message::new(1, b"ping");
        client.send(&msg).unwrap();

        std::thread::sleep(Duration::from_millis(50));

        // Server receives it
        let event = server.recv().unwrap();
        match event {
            Some(ServerEvent::Message(id, received)) => {
                assert_eq!(received.tag(), 1);
                assert_eq!(received.payload(), b"ping");
            }
            other => panic!("Expected Message event, got: {:?}", other),
        }
    }

    #[test]
    fn test_server_broadcast() {
        let port = find_port();
        let mut server = NetServer::bind(format!("127.0.0.1:{}", port)).unwrap();
        server.listen(4).unwrap();

        let mut client = NetClient::connect(format!("127.0.0.1:{}", port)).unwrap();
        std::thread::sleep(Duration::from_millis(50));

        // Accept client
        let _event = server.recv().unwrap();

        // Broadcast to all clients
        let broadcast = Message::new(0, b"welcome");
        server.broadcast(&broadcast).unwrap();

        // Client receives it
        std::thread::sleep(Duration::from_millis(50));
        let received = client.recv().unwrap().unwrap();
        assert_eq!(received.payload(), b"welcome");
    }

    #[test]
    fn test_client_disconnect() {
        let port = find_port();
        let mut server = NetServer::bind(format!("127.0.0.1:{}", port)).unwrap();
        server.listen(4).unwrap();

        {
            let mut _client = NetClient::connect(format!("127.0.0.1:{}", port)).unwrap();
            std::thread::sleep(Duration::from_millis(50));
            let _event = server.recv().unwrap();
            // client drops here
        }

        std::thread::sleep(Duration::from_millis(50));

        // Server should detect disconnection
        let event = server.recv().unwrap();
        assert!(matches!(event, Some(ServerEvent::Disconnected(_))));
    }

    #[test]
    fn test_multiple_clients() {
        let port = find_port();
        let mut server = NetServer::bind(format!("127.0.0.1:{}", port)).unwrap();
        server.listen(4).unwrap();

        let mut client1 = NetClient::connect(format!("127.0.0.1:{}", port)).unwrap();
        let mut client2 = NetClient::connect(format!("127.0.0.1:{}", port)).unwrap();
        std::thread::sleep(Duration::from_millis(100));

        // Accept both clients
        let _c1 = server.recv().unwrap();
        let _c2 = server.recv().unwrap();
        assert_eq!(server.client_count(), 2);

        // Client 1 sends a message
        client1.send(&Message::new(1, b"from c1")).unwrap();
        std::thread::sleep(Duration::from_millis(50));

        // Server receives from client 1
        let event = server.recv().unwrap();
        match event {
            Some(ServerEvent::Message(id, msg)) => {
                assert_eq!(msg.payload(), b"from c1");
            }
            other => panic!("Expected Message, got: {:?}", other),
        }
    }

    #[test]
    fn test_server_send_to_client() {
        let port = find_port();
        let mut server = NetServer::bind(format!("127.0.0.1:{}", port)).unwrap();
        server.listen(4).unwrap();

        let mut client = NetClient::connect(format!("127.0.0.1:{}", port)).unwrap();
        std::thread::sleep(Duration::from_millis(50));

        let event = server.recv().unwrap();
        let client_id = match event {
            Some(ServerEvent::Connected(id)) => id,
            other => panic!("Expected Connected, got: {:?}", other),
        };

        // Server sends to this client
        let reply = Message::new(2, b"reply");
        server.send(client_id, &reply).unwrap();

        std::thread::sleep(Duration::from_millis(50));
        let received = client.recv().unwrap().unwrap();
        assert_eq!(received.payload(), b"reply");
    }
}
