//! # MINIX Networking for Games
//!
//! Simple game networking over TCP/UDP using MINIX POSIX sockets.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────┐     ┌─────────────────────────┐
//! │       Game Client       │     │       Game Server       │
//! ├─────────────────────────┤     ├─────────────────────────┤
//! │  NetClient              │     │  NetServer              │
//! │  connect("host:port")   │     │  bind(port)             │
//! │  send(msg)              │◄───►│  accept()               │
//! │  recv() → Message       │     │  broadcast(msg)         │
//! └─────────────────────────┘     └─────────────────────────┘
//!
//! Or via UDP:
//! ┌─────────────────────────┐     ┌─────────────────────────┐
//! │  UdpPeer                │     │  UdpPeer                │
//! │  bind(port)             │◄───►│  bind(port)             │
//! │  send_to(addr, msg)     │     │  send_to(addr, msg)     │
//! │  recv_from() → (addr,   │     │  recv_from() → (addr,   │
//! │                   msg)  │     │                   msg)  │
//! └─────────────────────────┘     └─────────────────────────┘
//! ```
//!
//! ## Quick Start — TCP Client/Server
//!
//! ```no_run
//! use minix_net::{NetServer, NetClient, Message};
//!
//! // Server
//! let mut server = NetServer::bind("0.0.0.0:12345").unwrap();
//! server.listen(4).unwrap();
//!
//! // Client connects
//! let mut client = NetClient::connect("127.0.0.1:12345").unwrap();
//!
//! // Client sends a message
//! client.send(&Message::new(1, b"hello")).unwrap();
//!
//! // Server receives it
//! let (client_id, msg) = server.recv().unwrap().unwrap();
//! println!("Got msg type {} from client {}", msg.tag(), client_id);
//! ```
//!
//! ## Quick Start — UDP
//!
//! ```no_run
//! use minix_net::UdpPeer;
//!
//! let mut peer = UdpPeer::bind("0.0.0.0:12345").unwrap();
//! peer.send_to(b"hello", "127.0.0.1:12346").unwrap();
//! let (data, addr) = peer.recv_from().unwrap().unwrap();
//! ```

mod message;
mod tcp;
mod udp;

pub use message::Message;
pub use tcp::{NetServer, NetClient, ClientId, ServerEvent};
pub use udp::UdpPeer;

/// Re-export common io types.
pub use std::io::{self, Error, ErrorKind};
