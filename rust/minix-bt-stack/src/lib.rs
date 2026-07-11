//! # GergiOS Bluetooth Stack (BlueZ Phase 8)
//!
//! A native Rust Bluetooth stack for GergiOS, implementing:
//!
//! - **L2CAP** — Logical Link Control and Adaptation Protocol
//! - **RFCOMM** — Serial port emulation
//! - **SDP** — Service Discovery Protocol
//! - **GATT** — Generic Attribute Profile (BLE)
//!
//! This is a userspace library that communicates with the HCI driver
//! (`/dev/hci0`) from the `minix-bt-hci` crate.
//!
//! ## Architecture
//!
//! ```text
//! ┌────────────────────────────────────┐
//! │           Applications              │
//! ├────────────────────────────────────┤
//! │   libbluetooth (C API / Rust API)  │
//! ├────────────────────────────────────┤
//! │     Bluetooth Daemon (bt_daemon)   │
//! │  ┌─────┐ ┌──────┐ ┌────┐ ┌──────┐ │
//! │  │SDP  │ │GATT  │ │RFCOMM│ │HSP   │ │
//! │  └──┬──┘ └──┬───┘ └──┬──┘ └──┬───┘ │
//! │     └────────┼────────┴────────┘     │
//! │              │ L2CAP                 │
//! │              └──────────┬────────────│
//! ├─────────────────────────┼────────────┤
//! │              ┌──────────▼──────────┐ │
//! │              │   /dev/hci0 (HCI)   │ │
//! │              │  (minix-bt-hci)     │ │
//! │              └─────────────────────┘ │
//! └──────────────────────────────────────┘
//! ```

pub mod types;
pub mod l2cap;
pub mod sdp_record;
pub mod sdp;
pub mod rfcomm;
pub mod att;
pub mod gatt;
pub mod hci_mgr;
pub mod bt_daemon;
pub mod minix_ipc;
pub mod hidp;

// Re-export common types at crate root
pub use types::*;
pub use sdp_record::{
    DataElement, DataElementType, ServiceRecord, SdpAttrId, ServiceDatabase,
};
pub use hci_mgr::HciManager;
pub use bt_daemon::{
    BtDaemon, ConnectionManager, ProtocolMultiplexer, ServiceRegistry,
    DaemonConfig, RemoteDevice, Connection, ConnDirection, ConnState,
    DeviceType, BondFlags, BtDaemonCmd, MAX_DEVICES, MAX_CONNECTIONS,
};
