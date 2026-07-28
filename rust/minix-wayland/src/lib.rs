//! # minix-wayland — Wayland Protocol Implementation
//!
//! A bare-metal Wayland protocol implementation for MINIX, built on top
//! of `minix-compositor`. Provides:
//!
//! - **Wire format** (`wire`): Encode/decode Wayland messages
//! - **Protocol objects** (`protocol`): wl_display, wl_compositor,
//!   wl_surface, xdg_shell, wl_seat, etc.
//! - **Server** (`server`): Wayland server with connection management
//! - **Transport** (`transport`): Abstraction over Unix sockets (host)
//!   and MINIX IPC (target)
//!
//! ## Architecture
//!
//! ```text
//! Client App (speaks Wayland)
//!     │
//!     │ Unix socket (host) / MINIX IPC (target)
//!     ▼
//! ┌────────────────────────────────────────┐
//! │  WaylandServer                         │
//! │  ├── ConnectionManager                 │
//! │  ├── ProtocolObjectRegistry            │
//! │  └── MessageDispatcher                 │
//! ├────────────────────────────────────────┤
//! │  Protocol Implementations              │
//! │  ├── WlDisplay  ── wl_display         │
//! │  ├── WlCompositor ── wl_compositor    │
//! │  ├── WlSurface   ── wl_surface        │
//! │  ├── XdgWmBase   ── xdg_wm_base      │
//! │  ├── WlSeat      ── wl_seat           │
//! │  └── WlShm       ── wl_shm            │
//! ├────────────────────────────────────────┤
//! │  minix-compositor (software renderer)  │
//! └────────────────────────────────────────┘
//! ```

#![no_std]
#![allow(dead_code)]

extern crate alloc;

pub mod wire;
pub mod protocol;
pub mod server;
pub mod shell;
pub mod tiling;
pub mod floating;
pub mod workspace;
pub mod decorator;
pub mod keybindings;
pub mod panel;
pub mod transport;
