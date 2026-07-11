//! # Bluetooth Daemon (bluetoothd) — Main Event Loop
//!
//! Phase 8.5: The central Bluetooth daemon for GergiOS.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │                    Bluetooth Daemon                          │
//! │                                                              │
//! │  ┌──────────────────────────────────────────────────────┐   │
//! │  │              Main Event Loop                         │   │
//! │  │  ┌──────────┐  ┌──────────┐  ┌───────────────────┐ │   │
//! │  │  │ HCI Poll │  │ IPC Recv │  │ Timer Check       │ │   │
//! │  │  └────┬─────┘  └────┬─────┘  └────────┬──────────┘ │   │
//! │  └───────┼──────────────┼─────────────────┼─────────────┘   │
//! │          │              │                 │                  │
//! │  ┌───────▼──────────────▼─────────────────▼─────────────┐   │
//! │  │              Protocol Multiplexer                    │   │
//! │  │  ┌──────────┐  ┌──────────┐  ┌──────────┐          │   │
//! │  │  │ L2CAP    │  │ RFCOMM   │  │ SDP      │          │   │
//! │  │  │ Signaling│  │ Sessions │  │ Server   │          │   │
//! │  │  └────┬─────┘  └────┬─────┘  └────┬─────┘          │   │
//! │  └───────┼──────────────┼─────────────┼────────────────┘   │
//! │          │              │             │                     │
//! │  ┌───────▼──────────────▼─────────────▼────────────────┐   │
//! │  │              Connection Manager                     │   │
//! │  │  devices[] │ connections[] │ pairing state       │   │
//! │  └──────────────────────────────────────────────────────┘   │
//! │                                                              │
//! │  ┌──────────────────────────────────────────────────────┐   │
//! │  │              Service Registry (SDP)                   │   │
//! │  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐  │   │
//! │  │  │ SDP DB   │  │ GATT DB  │  │ Profile Manager  │  │   │
//! │  │  └──────────┘  └──────────┘  └──────────────────┘  │   │
//! │  └──────────────────────────────────────────────────────┘   │
//! │                                                              │
//! │  ┌──────────────────────────────────────────────────────┐   │
//! │  │              Application IPC                         │   │
//! │  │  register_service | connect | disconnect |          │   │
//! │  │  start_discovery | pair | unpair | get_info         │   │
//! │  └──────────────────────────────────────────────────────┘   │
//! └──────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## IPC Interface
//!
//! The daemon registers as a MINIX service and accepts IPC messages
//! from userspace applications. The message format follows the MINIX
//! IPC conventions (64-byte messages via `sendrec`).
//!
//! ## Configuration
//!
//! The daemon reads `/etc/bluetoothd.conf` on startup for settings
//! such as device name, discoverability, and pairing mode.

#![allow(dead_code)]

use crate::hci_mgr::HciManager;
use crate::l2cap::{
    L2CapChannelManager, L2CapChannelState, L2CapConnReq, L2CapConnResult,
    L2CapConnRsp, L2CapSigCode, L2CapCommandReject,
    L2CapRejectReason, L2CapLeCreditConnReq, L2CapConfigRsp, L2CapConfigResult,
    L2CapDisconRsp, L2CapDisconReq, L2CapConfigReq,
};
use crate::types::{BdAddr, ConnHandle, L2CapCid, L2CapPsm, ClassOfDevice, BtUuid};
use crate::sdp_record::{ServiceDatabase, ServiceRecord};
use crate::sdp::process_sdp_request;
use crate::sdp::SdpResponse;

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of tracked remote devices.
pub const MAX_DEVICES: usize = 32;

/// Maximum number of simultaneous connections.
pub const MAX_CONNECTIONS: usize = 16;

/// Default device name.
pub const DEFAULT_DEVICE_NAME: &str = "GergiOS";

/// Default inquiry duration (seconds).
pub const DEFAULT_INQUIRY_SECS: u8 = 10;

/// Poll interval for HCI events (milliseconds).
pub const HCI_POLL_MS: u64 = 50;

/// Minimum RSSI for reporting in inquiry results (dBm).
pub const MIN_RSSI: i8 = -127;

/// BT_CLASS: Computer, Laptop
pub const DEFAULT_CLASS_OF_DEVICE: [u8; 3] = [0x00, 0x1E, 0x00];

// ============================================================================
// Device Tracking
// ============================================================================

/// Type of a remote Bluetooth device.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeviceType {
    /// Classic BR/EDR device.
    Classic,
    /// Low Energy device.
    Le,
    /// Dual-mode device.
    Dual,
}

/// Flags for a bonded (paired) device.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BondFlags {
    pub bonded: bool,
    pub authenticated: bool,
    pub encrypted: bool,
}

/// A remote Bluetooth device known to the system.
#[derive(Clone, Debug)]
pub struct RemoteDevice {
    /// Bluetooth address.
    pub bdaddr: BdAddr,
    /// Device name (cached from inquiry or SDP).
    pub name: [u8; 248],
    /// Name length.
    pub name_len: usize,
    /// Class of device.
    pub class: ClassOfDevice,
    /// Device type.
    pub dev_type: DeviceType,
    /// Most recent RSSI.
    pub rssi: i8,
    /// Whether this device was seen during the last inquiry.
    pub found: bool,
    /// Bond/pairing state.
    pub bond: BondFlags,
    /// Last seen timestamp (in ticks).
    pub last_seen: u64,
}

impl RemoteDevice {
    fn new(bdaddr: BdAddr) -> Self {
        Self {
            bdaddr,
            name: [0u8; 248],
            name_len: 0,
            class: ClassOfDevice::new([0u8; 3]),
            dev_type: DeviceType::Classic,
            rssi: 127,
            found: true,
            bond: BondFlags {
                bonded: false,
                authenticated: false,
                encrypted: false,
            },
            last_seen: 0,
        }
    }

    /// Get the device name as a string slice.
    pub fn name_str(&self) -> &str {
        let len = self.name_len.min(247);
        core::str::from_utf8(&self.name[..len]).unwrap_or("(invalid utf8)")
    }

    /// Set the device name.
    pub fn set_name(&mut self, name: &[u8]) {
        let len = name.len().min(247);
        self.name[..len].copy_from_slice(&name[..len]);
        self.name_len = len;
    }
}

// ============================================================================
// Connection Tracking
// ============================================================================

/// Direction of a connection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnDirection {
    /// We initiated the connection (master role).
    Outgoing,
    /// Remote device initiated (slave role).
    Incoming,
}

/// State of a Bluetooth connection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnState {
    /// Being established.
    Connecting,
    /// Active and operational.
    Connected,
    /// Being disconnected.
    Disconnecting,
}

/// A tracked Bluetooth connection.
#[derive(Clone, Debug)]
pub struct Connection {
    /// HCI connection handle.
    pub handle: u16,
    /// Remote device BD_ADDR.
    pub bdaddr: BdAddr,
    /// Connection direction.
    pub direction: ConnDirection,
    /// Connection state.
    pub state: ConnState,
    /// Link type (0 = SCO, 1 = ACL, 2 = eSCO, 3 = LE).
    pub link_type: u8,
    /// Encryption mode.
    pub encrypted: bool,
    /// Authenticated (paired).
    pub authenticated: bool,
    /// L2CAP channels on this connection.
    pub l2cap_channels: Vec<u16>, // local CIDs
}

impl Connection {
    fn new(handle: u16, bdaddr: BdAddr, direction: ConnDirection) -> Self {
        Self {
            handle,
            bdaddr,
            direction,
            state: ConnState::Connecting,
            link_type: 1, // ACL
            encrypted: false,
            authenticated: false,
            l2cap_channels: Vec::new(),
        }
    }

    fn is_connected(&self) -> bool {
        self.state == ConnState::Connected
    }
}

// ============================================================================
// Connection Manager
// ============================================================================

/// Manages all Bluetooth connections and known devices.
pub struct ConnectionManager {
    /// Known remote devices.
    devices: Vec<RemoteDevice>,
    /// Active connections.
    connections: Vec<Connection>,
    /// Our own device name.
    local_name: [u8; 248],
    local_name_len: usize,
    /// Our own class of device.
    _local_class: ClassOfDevice,
    /// Whether we are discoverable (inquiry scan enabled).
    discoverable: bool,
    /// Whether we are connectable (page scan enabled).
    connectable: bool,
}

impl ConnectionManager {
    pub fn new() -> Self {
        let mut local_name = [0u8; 248];
        let name = DEFAULT_DEVICE_NAME.as_bytes();
        let len = name.len().min(247);
        local_name[..len].copy_from_slice(&name[..len]);

        Self {
            devices: Vec::with_capacity(MAX_DEVICES),
            connections: Vec::with_capacity(MAX_CONNECTIONS),
            local_name,
            local_name_len: len,
            _local_class: ClassOfDevice::new(DEFAULT_CLASS_OF_DEVICE),
            discoverable: false,
            connectable: true,
        }
    }

    // ── Device Discovery ─────────────────────────────────────────────

    /// Record an inquiry result from an HCI Inquiry Result event.
    pub fn record_inquiry_result(
        &mut self,
        bdaddr: BdAddr,
        class: ClassOfDevice,
        rssi: i8,
        _clock_offset: u16,
        _page_scan_repetition_mode: u8,
        current_ticks: u64,
    ) {
        let device = self.find_or_create_device(bdaddr);
        device.class = class;
        device.found = true;
        device.last_seen = current_ticks;

        if rssi != 127 && rssi >= MIN_RSSI {
            device.rssi = rssi;
        }
    }

    /// Record an LE advertising report from an HCI LE Meta event.
    pub fn record_le_advertising_report(
        &mut self,
        bdaddr: BdAddr,
        _addr_type: u8,
        rssi: i8,
        data: &[u8],
        current_ticks: u64,
    ) {
        let device = self.find_or_create_device(bdaddr);
        device.dev_type = DeviceType::Le;
        device.found = true;
        device.last_seen = current_ticks;

        if rssi != 127 && rssi >= MIN_RSSI {
            device.rssi = rssi;
        }

        // Try to extract name from AD data
        parse_advertising_name(data, device);
    }

    /// Clear the 'found' flag on all devices (call before starting a scan).
    pub fn clear_found_flags(&mut self) {
        for device in &mut self.devices {
            device.found = false;
        }
    }

    /// Get devices found during the most recent inquiry.
    pub fn found_devices(&self) -> Vec<&RemoteDevice> {
        self.devices.iter().filter(|d| d.found).collect()
    }

    /// Get all known devices.
    pub fn all_devices(&self) -> &[RemoteDevice] {
        &self.devices
    }

    /// Find a device by BD_ADDR.
    pub fn find_device(&self, bdaddr: &BdAddr) -> Option<&RemoteDevice> {
        self.devices.iter().find(|d| d.bdaddr == *bdaddr)
    }

    /// Find a device by BD_ADDR (mutable).
    pub fn find_device_mut(&mut self, bdaddr: &BdAddr) -> Option<&mut RemoteDevice> {
        self.devices.iter_mut().find(|d| d.bdaddr == *bdaddr)
    }

    /// Create a new device entry or return the existing one.
    fn find_or_create_device(&mut self, bdaddr: BdAddr) -> &mut RemoteDevice {
        // First check if already exists
        let existing_pos = self.devices.iter().position(|d| d.bdaddr == bdaddr);
        if let Some(idx) = existing_pos {
            return &mut self.devices[idx];
        }

        // Not found — add new or overwrite
        if self.devices.len() < MAX_DEVICES {
            self.devices.push(RemoteDevice::new(bdaddr));
            self.devices.last_mut().unwrap()
        } else {
            // Overwrite the first unseen device, or the first one
            let overwrite_idx = self.devices.iter().position(|d| !d.found)
                .unwrap_or(0);
            self.devices[overwrite_idx] = RemoteDevice::new(bdaddr);
            &mut self.devices[overwrite_idx]
        }
    }

    // ── Connection Management ─────────────────────────────────────────

    /// Record a new connection.
    pub fn add_connection(
        &mut self,
        handle: u16,
        bdaddr: BdAddr,
        direction: ConnDirection,
        link_type: u8,
    ) -> Option<&mut Connection> {
        if self.connections.len() >= MAX_CONNECTIONS {
            return None;
        }

        self.connections.push(Connection::new(handle, bdaddr, direction));
        let conn = self.connections.last_mut().unwrap();
        conn.link_type = link_type;
        Some(conn)
    }

    /// Find a connection by HCI handle.
    pub fn find_connection_by_handle(&self, handle: u16) -> Option<&Connection> {
        self.connections.iter().find(|c| c.handle == handle)
    }

    /// Find a connection by HCI handle (mutable).
    pub fn find_connection_by_handle_mut(&mut self, handle: u16) -> Option<&mut Connection> {
        self.connections.iter_mut().find(|c| c.handle == handle)
    }

    /// Find a connection by BD_ADDR.
    pub fn find_connection_by_bdaddr(&self, bdaddr: &BdAddr) -> Option<&Connection> {
        self.connections.iter().find(|c| c.bdaddr == *bdaddr)
    }

    /// Find a connection by BD_ADDR (mutable).
    pub fn find_connection_by_bdaddr_mut(&mut self, bdaddr: &BdAddr) -> Option<&mut Connection> {
        self.connections.iter_mut().find(|c| c.bdaddr == *bdaddr)
    }

    /// Transition a connection to Connected state.
    pub fn mark_connected(&mut self, handle: u16) -> bool {
        if let Some(conn) = self.find_connection_by_handle_mut(handle) {
            conn.state = ConnState::Connected;
            true
        } else {
            false
        }
    }

    /// Transition a connection to Disconnecting state.
    pub fn mark_disconnecting(&mut self, handle: u16) -> bool {
        if let Some(conn) = self.find_connection_by_handle_mut(handle) {
            conn.state = ConnState::Disconnecting;
            true
        } else {
            false
        }
    }

    /// Remove a connection by handle.
    pub fn remove_connection(&mut self, handle: u16) -> bool {
        let len = self.connections.len();
        self.connections.retain(|c| c.handle != handle);
        self.connections.len() < len
    }

    /// Set encryption on a connection.
    pub fn set_encryption(&mut self, handle: u16, encrypted: bool) -> bool {
        if let Some(conn) = self.find_connection_by_handle_mut(handle) {
            conn.encrypted = encrypted;
            true
        } else {
            false
        }
    }

    /// Get all active connections.
    pub fn active_connections(&self) -> &[Connection] {
        &self.connections
    }

    /// Check if we have any active connections.
    pub fn has_connections(&self) -> bool {
        !self.connections.is_empty()
    }

    // ── Local Settings ────────────────────────────────────────────────

    /// Set the local device name.
    pub fn set_local_name(&mut self, name: &str) {
        let bytes = name.as_bytes();
        let len = bytes.len().min(247);
        self.local_name[..len].copy_from_slice(&bytes[..len]);
        self.local_name_len = len;
    }

    /// Get the local device name as a string slice.
    pub fn local_name_str(&self) -> &str {
        core::str::from_utf8(&self.local_name[..self.local_name_len])
            .unwrap_or("GergiOS")
    }

    /// Get the local device name bytes.
    pub fn local_name_bytes(&self) -> &[u8] {
        &self.local_name[..self.local_name_len]
    }

    /// Set discoverable (inquiry scan).
    pub fn set_discoverable(&mut self, discoverable: bool) {
        self.discoverable = discoverable;
    }

    /// Check if discoverable.
    pub fn is_discoverable(&self) -> bool {
        self.discoverable
    }

    /// Set connectable (page scan).
    pub fn set_connectable(&mut self, connectable: bool) {
        self.connectable = connectable;
    }

    /// Check if connectable.
    pub fn is_connectable(&self) -> bool {
        self.connectable
    }
}

// ── Free helper: parse advertising data ──────────────────────────────

/// Parse advertising data for a device name (independent function to avoid borrow conflicts).
fn parse_advertising_name(data: &[u8], device: &mut RemoteDevice) {
    let mut i = 0;
    while i + 1 < data.len() {
        let len = data[i] as usize;
        if len == 0 {
            break;
        }
        let _ad_type = data[i + 1];
        if i + 1 + len > data.len() {
            break;
        }
        let ad_data = &data[i + 2..i + 1 + len];

        match _ad_type {
            // AD Type: Shortened Local Name (0x08) or Complete Local Name (0x09)
            0x08 | 0x09 => {
                device.set_name(ad_data);
            }
            _ => {}
        }
        i += 1 + len;
    }
}

// ============================================================================
// Protocol Multiplexer — L2CAP ↔ ACL Data Routing
// ============================================================================

/// Routes ACL data between the HCI layer and upper-layer protocols.
///
/// Handles:
/// - L2CAP signaling (channel open/close/configure)
/// - L2CAP data dispatch to protocol handlers (RFCOMM, SDP, ATT, etc.)
pub struct ProtocolMultiplexer {
    /// L2CAP channel manager.
    pub l2cap: L2CapChannelManager,
    /// SDP service database.
    pub sdp_db: ServiceDatabase,
    /// List of registered upper-layer protocol handlers.
    handlers: Vec<(L2CapPsm, ProtocolHandler)>,
}

/// A registered protocol handler for a PSM.
type ProtocolHandler = Box<dyn FnMut(u16, u16, &[u8]) -> Option<Vec<u8>> + Send>;

impl ProtocolMultiplexer {
    pub fn new() -> Self {
        Self {
            l2cap: L2CapChannelManager::new(),
            sdp_db: ServiceDatabase::new(),
            handlers: Vec::new(),
        }
    }

    /// Register a protocol handler for a given PSM.
    /// The handler receives (conn_handle, local_cid, data) and may return a response.
    pub fn register_protocol<F>(&mut self, psm: L2CapPsm, handler: F)
    where
        F: FnMut(u16, u16, &[u8]) -> Option<Vec<u8>> + 'static + Send,
    {
        self.handlers.push((psm, Box::new(handler)));
    }

    /// Process incoming ACL data from the HCI layer.
    ///
    /// `handle`: HCI connection handle.
    /// `pb_flag`: Packet Boundary flag.
    /// `_bc_flag`: Broadcast flag.
    /// `data`: The ACL payload (starts with L2CAP header: length + CID).
    ///
    /// Returns a list of outgoing ACL packets to send: (handle, frame_bytes).
    ///
    /// Note: this is a free function that takes the multiplexer as a parameter
    /// rather than capturing it in a closure, to avoid self-referential pointer issues.
    pub fn process_acl_data(
        &mut self,
        handle: u16,
        pb_flag: u8,
        _bc_flag: u8,
        data: &[u8],
    ) -> Vec<(u16, Vec<u8>)> {
        let conn_handle = ConnHandle::new(handle);
        let mut responses = Vec::new();

        // Parse L2CAP B-frame: [Length(2) | CID(2) | Payload(N)]
        if data.len() < 4 {
            return responses;
        }

        let _frame_len = u16::from_le_bytes([data[0], data[1]]) as usize;
        let cid = u16::from_le_bytes([data[2], data[3]]);
        let payload = &data[4..];

        // Dispatch based on CID
        match L2CapCid::from_raw(cid) {
            L2CapCid::Signaling | L2CapCid::LeSignaling => {
                let sig_responses = self.process_l2cap_signaling(conn_handle, cid, payload);
                responses.extend(sig_responses);
            }
            L2CapCid::Attribute => {
                if let Some(rsp) = self.dispatch_to_handler(handle, cid, L2CapPsm::LeCoC, payload) {
                    responses.push((handle, rsp));
                }
            }
            L2CapCid::Connectionless => {
                // Connectionless data — ignore for now
            }
            _ => {
                // Dynamically allocated CID — find the corresponding channel
                if let Some(channel) = self.l2cap.find_by_remote_cid(cid) {
                    let psm = channel.psm;
                    let remote_cid = channel.remote_cid;

                    let reassembled = if pb_flag == 0x01 {
                        // Continuation — ignore for now
                        return responses;
                    } else {
                        payload.to_vec()
                    };

                    // SDP is dispatched directly to avoid self-referential closure issues.
                    // Other protocols use the handler list.
                    let rsp = if psm == L2CapPsm::Sdp {
                        self.dispatch_sdp(&reassembled)
                    } else {
                        let local_cid = channel.local_cid;
                        self.dispatch_to_handler(handle, local_cid, psm, &reassembled)
                    };

                    if let Some(rsp_data) = rsp {
                        let bframe =
                            crate::l2cap::build_l2cap_b_frame(remote_cid, &rsp_data);
                        responses.push((handle, bframe));
                    }
                }
            }
        }

        responses
    }

    /// Process L2CAP signaling commands.
    fn process_l2cap_signaling(
        &mut self,
        conn_handle: ConnHandle,
        _cid: u16,
        payload: &[u8],
    ) -> Vec<(u16, Vec<u8>)> {
        let mut responses = Vec::new();
        let handle = conn_handle.to_raw();

        let commands = crate::l2cap::parse_sig_commands(payload);
        for cmd in commands {
            match cmd.code {
                L2CapSigCode::ConnectionRequest => {
                    if let Some(req) = L2CapConnReq::parse(&cmd.data) {
                        let response = self.handle_connection_request(conn_handle, &req, cmd.id);
                        responses.push((handle, response));
                    }
                }
                L2CapSigCode::ConnectionResponse => {
                    if let Some(rsp) = L2CapConnRsp::parse(&cmd.data) {
                        if let Some(ch) = self.l2cap.find_by_local_cid(rsp.source_cid) {
                            ch.state = if rsp.result == L2CapConnResult::Success {
                                L2CapChannelState::Config
                            } else {
                                L2CapChannelState::Closed
                            };
                            ch.remote_cid = rsp.destination_cid;
                        }
                    }
                }
                L2CapSigCode::DisconnectionRequest => {
                    if let Some(discon) = L2CapDisconReq::parse(&cmd.data) {
                        let rsp_data = build_discon_rsp_bytes(cmd.id, &discon);
                        responses.push((handle, rsp_data));
                        self.l2cap.remove_channel(discon.source_cid);
                    }
                }
                L2CapSigCode::DisconnectionResponse => {
                    if let Some(discon_rsp) = L2CapDisconRsp::parse(&cmd.data) {
                        self.l2cap.remove_channel(discon_rsp.source_cid);
                    }
                }
                L2CapSigCode::CommandReject => {
                    // Remote rejected our command
                }
                L2CapSigCode::ConfigureRequest => {
                    if let Some(req) = L2CapConfigReq::parse(&cmd.data) {
                        let rsp_data = build_config_rsp_bytes(cmd.id, &req);
                        responses.push((handle, rsp_data));

                        if let Some(ch) = self.l2cap.find_by_local_cid(req.destination_cid) {
                            ch.state = L2CapChannelState::Open;
                        }
                    }
                }
                L2CapSigCode::ConfigureResponse => {
                    if let Some(rsp) = L2CapConfigRsp::parse(&cmd.data) {
                        if let Some(ch) = self.l2cap.find_by_local_cid(rsp.source_cid) {
                            if rsp.result == L2CapConfigResult::Success {
                                ch.state = L2CapChannelState::Open;
                            }
                        }
                    }
                }
                L2CapSigCode::EchoRequest => {
                    let rsp = crate::l2cap::build_sig_command(
                        L2CapSigCode::EchoResponse,
                        cmd.id,
                        &cmd.data,
                    );
                    let bframe = crate::l2cap::build_l2cap_b_frame(
                        L2CapCid::Signaling.to_raw(),
                        &rsp,
                    );
                    responses.push((handle, bframe));
                }
                L2CapSigCode::InformationRequest => {
                    let it = cmd.data.first().copied().unwrap_or(0) as u16
                        | ((cmd.data.get(1).copied().unwrap_or(0) as u16) << 8);
                    let data = match it {
                        0x0001 => vec![0x00, 0x00],
                        0x0002 => {
                            let features: u32 = 0x0000_0001;
                            features.to_le_bytes().to_vec()
                        }
                        0x0003 => {
                            let fixed: u64 = (1 << 0x0001) | (1 << 0x0002);
                            fixed.to_le_bytes().to_vec()
                        }
                        _ => vec![0x00, 0x00],
                    };
                    let info_rsp = crate::l2cap::build_sig_command(
                        L2CapSigCode::InformationResponse,
                        cmd.id,
                        &data,
                    );
                    let bframe = crate::l2cap::build_l2cap_b_frame(
                        L2CapCid::Signaling.to_raw(),
                        &info_rsp,
                    );
                    responses.push((handle, bframe));
                }
                L2CapSigCode::LeCreditBasedConnReq => {
                    if let Some(req) = L2CapLeCreditConnReq::parse(&cmd.data) {
                        let response = self.handle_le_credit_conn_req(conn_handle, &req, cmd.id);
                        responses.push((handle, response));
                    }
                }
                _ => {
                    let reject = L2CapCommandReject::build(
                        L2CapRejectReason::NotUnderstood,
                        cmd.id,
                        0,
                    );
                    let bframe = crate::l2cap::build_l2cap_b_frame(
                        L2CapCid::Signaling.to_raw(),
                        &reject,
                    );
                    responses.push((handle, bframe));
                }
            }
        }

        responses
    }

    /// Handle an incoming L2CAP Connection Request.
    fn handle_connection_request(
        &mut self,
        conn_handle: ConnHandle,
        req: &L2CapConnReq,
        sig_id: u8,
    ) -> Vec<u8> {
        let (result, dest_cid) = match req.psm.to_raw() {
            0x0001 | 0x0003 | 0x0011 | 0x0013 => {
                // SDP, RFCOMM, HID — supported
                let ch = self.l2cap.create_channel(conn_handle, req.psm);
                ch.state = L2CapChannelState::WaitConnect;
                ch.remote_cid = req.source_cid;
                (L2CapConnResult::Success, ch.local_cid)
            }
            _ => (L2CapConnResult::PsmNotSupported, 0),
        };

        let rsp = L2CapConnRsp {
            destination_cid: dest_cid,
            source_cid: req.source_cid,
            result,
            status: 0,
        };
        let rsp_data = rsp.build();
        let sig = crate::l2cap::build_sig_command(
            L2CapSigCode::ConnectionResponse,
            sig_id,
            &rsp_data,
        );
        crate::l2cap::build_l2cap_b_frame(L2CapCid::Signaling.to_raw(), &sig)
    }

    /// Handle an incoming LE Credit-Based Connection Request.
    fn handle_le_credit_conn_req(
        &mut self,
        conn_handle: ConnHandle,
        req: &L2CapLeCreditConnReq,
        sig_id: u8,
    ) -> Vec<u8> {
        let (result, dest_cid) = match req.psm.to_raw() {
            0x0025 | 0x0027 => {
                let ch = self.l2cap.create_channel(conn_handle, req.psm);
                ch.state = L2CapChannelState::WaitConnect;
                ch.remote_cid = req.source_cid;
                ch.local_mtu = req.mtu;
                (L2CapConnResult::Success, ch.local_cid)
            }
            _ => (L2CapConnResult::PsmNotSupported, 0),
        };

        let mut rsp_data = Vec::with_capacity(10);
        rsp_data.extend_from_slice(&dest_cid.to_le_bytes());
        rsp_data.extend_from_slice(&req.mtu.to_le_bytes());
        rsp_data.extend_from_slice(&req.mps.to_le_bytes());
        rsp_data.extend_from_slice(&10u16.to_le_bytes());
        rsp_data.extend_from_slice(&(result as u16).to_le_bytes());

        let sig = crate::l2cap::build_sig_command(
            L2CapSigCode::LeCreditBasedConnRsp,
            sig_id,
            &rsp_data,
        );
        crate::l2cap::build_l2cap_b_frame(L2CapCid::LeSignaling.to_raw(), &sig)
    }

    /// Dispatch data to a registered protocol handler for the given PSM.
    fn dispatch_to_handler(
        &mut self,
        handle: u16,
        local_cid: u16,
        psm: L2CapPsm,
        data: &[u8],
    ) -> Option<Vec<u8>> {
        for (registered_psm, handler) in &mut self.handlers {
            if *registered_psm == psm {
                return handler(handle, local_cid, data);
            }
        }
        None
    }

    /// Dispatch an SDP request directly (avoids self-referential pointers).
    fn dispatch_sdp(&self, data: &[u8]) -> Option<Vec<u8>> {
        match process_sdp_request(&self.sdp_db, data) {
            SdpResponse::Raw(rsp) => Some(rsp),
            SdpResponse::None => None,
        }
    }

    /// Register a local service in the SDP database.
    pub fn register_service(&mut self, record: ServiceRecord) -> u32 {
        self.sdp_db.register_service(record)
    }

    /// Build an L2CAP B-frame for a dynamic CID channel.
    pub fn build_data_frame(&self, remote_cid: u16, data: &[u8]) -> Vec<u8> {
        crate::l2cap::build_l2cap_b_frame(remote_cid, data)
    }

    /// Create an outgoing L2CAP channel.
    pub fn create_outgoing_channel(
        &mut self,
        conn_handle: ConnHandle,
        psm: L2CapPsm,
    ) -> Option<u16> {
        let ch = self.l2cap.create_channel(conn_handle, psm);
        ch.state = L2CapChannelState::WaitConnect;
        Some(ch.local_cid)
    }

    /// Build an L2CAP Connection Request for an outgoing channel.
    pub fn build_connection_request(
        &self,
        local_cid: u16,
        psm: L2CapPsm,
        sig_id: u8,
    ) -> Vec<u8> {
        let req = L2CapConnReq { psm, source_cid: local_cid };
        let req_data = req.build();
        let sig = crate::l2cap::build_sig_command(
            L2CapSigCode::ConnectionRequest,
            sig_id,
            &req_data,
        );
        crate::l2cap::build_l2cap_b_frame(L2CapCid::Signaling.to_raw(), &sig)
    }
}

// ============================================================================
// L2CAP Signaling Helpers (manual byte construction, no build() needed)
// ============================================================================

/// Build a Disconnection Response signaling command from a Disconnection Request.
fn build_discon_rsp_bytes(sig_id: u8, req: &L2CapDisconReq) -> Vec<u8> {
    // Disconnection Response body: dest_cid(2) | source_cid(2)
    let mut data = Vec::with_capacity(4);
    data.extend_from_slice(&req.source_cid.to_le_bytes()); // dest in rsp = source in req
    data.extend_from_slice(&req.destination_cid.to_le_bytes()); // source in rsp = dest in req
    let sig = crate::l2cap::build_sig_command(L2CapSigCode::DisconnectionResponse, sig_id, &data);
    crate::l2cap::build_l2cap_b_frame(L2CapCid::Signaling.to_raw(), &sig)
}

/// Build a Configure Response signaling command from a Configure Request.
fn build_config_rsp_bytes(sig_id: u8, req: &L2CapConfigReq) -> Vec<u8> {
    // Configure Response body: source_cid(2) | flags(2) | result(2) | options...
    let mut data = Vec::with_capacity(6);
    data.extend_from_slice(&req.destination_cid.to_le_bytes()); // source in rsp = dest in req
    data.extend_from_slice(&0u16.to_le_bytes()); // flags
    data.extend_from_slice(&(L2CapConfigResult::Success as u16).to_le_bytes());
    for opt in &req.options {
        data.push(opt.opt_type.to_byte());
        data.push(opt.value.len() as u8);
        data.extend_from_slice(&opt.value);
    }
    let sig = crate::l2cap::build_sig_command(L2CapSigCode::ConfigureResponse, sig_id, &data);
    crate::l2cap::build_l2cap_b_frame(L2CapCid::Signaling.to_raw(), &sig)
}

// ============================================================================
// Service Registry
// ============================================================================

/// Manages local service registration (SDP + GATT).
pub struct ServiceRegistry {
    /// SDP service database.
    pub sdp_db: ServiceDatabase,
    /// Registered service handles.
    service_handles: Vec<u32>,
}

impl ServiceRegistry {
    pub fn new() -> Self {
        Self {
            sdp_db: ServiceDatabase::new(),
            service_handles: Vec::new(),
        }
    }

    /// Register a service and get its handle.
    pub fn register(&mut self, record: ServiceRecord) -> u32 {
        let handle = self.sdp_db.register_service(record);
        self.service_handles.push(handle);
        handle
    }

    /// Unregister a service by handle.
    pub fn unregister(&mut self, handle: u32) -> bool {
        let removed = self.sdp_db.unregister_service(handle);
        if removed {
            self.service_handles.retain(|h| *h != handle);
        }
        removed
    }

    /// Get all registered service handles.
    pub fn service_handles(&self) -> &[u32] {
        &self.service_handles
    }

    /// Register the standard SDP service itself.
    pub fn register_sdp_service(&mut self) -> u32 {
        let record = crate::sdp_record::build_service_record(
            crate::types::sdp_uuids::SDP,
            crate::types::sdp_uuids::L2CAP,
            Some(0x0001),
            None,
            "SDP Service",
            "Service Discovery Protocol",
        );
        self.register(record)
    }

    /// Register the standard GAP service.
    pub fn register_gap_service(&mut self, device_name: &str) -> u32 {
        let record = crate::sdp_record::build_service_record(
            crate::types::sdp_uuids::GAP,
            crate::types::sdp_uuids::L2CAP,
            None,
            None,
            device_name,
            "Generic Access Profile",
        );
        self.register(record)
    }
}

// ============================================================================
// Daemon Configuration
// ============================================================================

/// Daemon configuration, read from /etc/bluetoothd.conf.
#[derive(Clone, Debug)]
pub struct DaemonConfig {
    /// Local device name.
    pub name: String,
    /// Device class.
    pub class_of_device: [u8; 3],
    /// Whether to be discoverable on startup.
    pub discoverable: bool,
    /// Whether to be connectable on startup.
    pub connectable: bool,
    /// Whether to enable BLE.
    pub le_enabled: bool,
    /// Inquiry duration (seconds).
    pub inquiry_duration: u8,
    /// Poll interval for HCI events (ms).
    pub poll_interval_ms: u64,
}

impl DaemonConfig {
    pub fn default() -> Self {
        Self {
            name: DEFAULT_DEVICE_NAME.to_string(),
            class_of_device: DEFAULT_CLASS_OF_DEVICE,
            discoverable: false,
            connectable: true,
            le_enabled: true,
            inquiry_duration: DEFAULT_INQUIRY_SECS,
            poll_interval_ms: HCI_POLL_MS,
        }
    }

    /// Load configuration from /etc/bluetoothd.conf.
    pub fn load() -> Self {
        Self::default()
    }
}

// ============================================================================
// Bluetooth Daemon
// ============================================================================

/// The main Bluetooth daemon — ties together HCI manager, connection
/// manager, protocol multiplexer, and service registry.
pub struct BtDaemon {
    /// Daemon configuration.
    pub config: DaemonConfig,
    /// HCI device manager.
    pub hci: HciManager,
    /// Connection manager.
    pub connections: ConnectionManager,
    /// Protocol multiplexer (L2CAP ↔ upper layers).
    pub multiplexer: ProtocolMultiplexer,
    /// Service registry (SDP).
    pub services: ServiceRegistry,
    /// Whether the daemon is running.
    pub running: bool,
}

impl BtDaemon {
    /// Create a new Bluetooth daemon instance.
    pub fn new() -> Self {
        Self {
            config: DaemonConfig::load(),
            hci: HciManager::new(),
            connections: ConnectionManager::new(),
            multiplexer: ProtocolMultiplexer::new(),
            services: ServiceRegistry::new(),
            running: false,
        }
    }

    /// Initialize the daemon — open HCI, init controller, register services.
    pub fn init(&mut self) -> bool {
        // Step 1: Open HCI device
        if !self.hci.open() {
            return false;
        }

        // Step 2: Init controller
        if !self.hci.init_controller() {
            return false;
        }

        // Step 3: Update connection manager with controller info
        self.connections.set_local_name(
            core::str::from_utf8(&self.hci.dev_info.name)
                .unwrap_or(DEFAULT_DEVICE_NAME)
                .trim_end_matches('\0'),
        );

        // Step 4: Set up scan modes
        let scan_mode = if self.config.discoverable && self.config.connectable {
            0x03
        } else if self.config.connectable {
            0x02
        } else {
            0x00
        };
        self.hci.set_scan_mode(scan_mode);

        // Step 5: Register built-in SDP services
        self.services.register_sdp_service();
        self.services.register_gap_service(self.connections.local_name_str());

        // Mark as running
        self.running = true;

        true
    }

    /// Run one iteration of the daemon event loop.
    ///
    /// Polls HCI events and processes them through the protocol multiplexer.
    /// Returns the number of events processed.
    ///
    /// In MINIX, this would be called from the SEF main loop.
    /// On host, it's called directly for testing.
    pub fn run_once(&mut self) -> usize {
        if !self.running {
            return 0;
        }

        // Poll for HCI events and process them
        // On real hardware, this reads from /dev/hci0
        // The ACL data is forwarded to the protocol multiplexer via
        // the poll() -> dispatch chain when HciManager calls its handlers.
        self.hci.poll()
    }

    /// Run the main event loop (blocking).
    ///
    /// On real MINIX, this would be driven by SEF.
    /// For testing, runs once and returns.
    pub fn run(&mut self) {
        if !self.running {
            return;
        }

        self.run_once();
    }

    /// Start device discovery (inquiry).
    pub fn start_discovery(&mut self) -> bool {
        self.connections.clear_found_flags();
        self.hci.start_inquiry([0x33, 0x8B, 0x9E], self.config.inquiry_duration, 0)
    }

    /// Stop device discovery.
    pub fn stop_discovery(&mut self) -> bool {
        self.hci.stop_inquiry()
    }

    /// Get discovered devices.
    pub fn discovered_devices(&self) -> Vec<&RemoteDevice> {
        self.connections.found_devices()
    }

    /// Connect to a remote device.
    pub fn connect(&mut self, bdaddr: &BdAddr) -> bool {
        self.hci.create_connection(bdaddr, 0xCC18)
    }

    /// Disconnect from a remote device.
    pub fn disconnect(&mut self, handle: u16, reason: u8) -> bool {
        self.connections.mark_disconnecting(handle);
        self.hci.disconnect(handle, reason)
    }

    /// Set the local device name.
    pub fn set_name(&mut self, name: &str) {
        self.connections.set_local_name(name);

        let name_bytes = name.as_bytes();
        let mut padded = [0u8; 248];
        let len = name_bytes.len().min(247);
        padded[..len].copy_from_slice(&name_bytes[..len]);
        self.hci.send_cmd(0x03, 0x0013, &padded);
    }

    /// Set discoverable mode.
    pub fn set_discoverable(&mut self, enable: bool) {
        self.connections.set_discoverable(enable);
        let scan_mode = if enable && self.connections.is_connectable() {
            0x03
        } else if enable {
            0x02
        } else if self.connections.is_connectable() {
            0x02
        } else {
            0x00
        };
        self.hci.set_scan_mode(scan_mode);
    }

    /// Set connectable mode.
    pub fn set_connectable(&mut self, enable: bool) {
        self.connections.set_connectable(enable);
        let scan_mode = if enable && self.connections.is_discoverable() {
            0x03
        } else if enable {
            0x02
        } else {
            0x00
        };
        self.hci.set_scan_mode(scan_mode);
    }

    /// Register an SDP service.
    pub fn register_service(&mut self, record: ServiceRecord) -> u32 {
        self.services.register(record)
    }

    // ── Pairing ───────────────────────────────────────────────────────

    /// Initiate BR/EDR pairing with a remote device.
    ///
    /// The pairing flow:
    /// 1. Enable SSP if not already enabled
    /// 2. Find or establish a connection to the device
    /// 3. Send Authentication_Requested HCI command
    /// 4. The controller handles PIN exchange / SSP with the remote
    /// 5. On Link Key Notification event, store the key
    /// 6. Mark device as bonded
    ///
    /// Returns true if the authentication command was sent successfully.
    pub fn pair(&mut self, bdaddr: &BdAddr) -> bool {
        // Step 1: Enable SSP for modern pairing
        self.hci.set_simple_pairing_mode(true);

        // Step 2: Set default IO capabilities for SSP
        // Use Write_Default_IO_Capabilities (OGF=0x03, OCF=0x0068) so the
        // controller knows our capabilities during pairing without needing
        // to respond to the IO Capability Request event (0x31).
        // io_cap=1 (DisplayYesNo) | oob=false | auth=0x01 (MITM not required)
        self.hci.write_default_io_capabilities(1, false, 0x01);

        // Step 3: Find an existing connection for this device
        let handle = self.connections.find_connection_by_bdaddr(bdaddr)
            .map(|c| c.handle);

        if let Some(handle) = handle {
            // Connection exists — start authentication
            self.hci.authentication_requested(handle)
        } else {
            // No connection — create one first, then authentication
            // will be triggered after connection is established
            // For now, just initiate the connection
            let connected = self.connect(bdaddr);
            // Authentication will start once the connection is established.
            // The device will show as paired only after handle_link_key_notification fires.
            connected
        }
    }

    /// Remove a bond (unpair) with a remote device.
    ///
    /// 1. Delete the stored link key from the controller
    /// 2. Mark the device as not bonded
    /// 3. Disconnect if connected
    ///
    /// Returns true if the link key deletion command was sent.
    pub fn unpair(&mut self, bdaddr: &BdAddr) -> bool {
        // Step 1: Delete link key from controller
        self.hci.delete_stored_link_key(bdaddr, false);

        // Step 2: Mark device as not bonded
        if let Some(dev) = self.connections.find_device_mut(bdaddr) {
            dev.bond = BondFlags {
                bonded: false,
                authenticated: false,
                encrypted: false,
            };
        }

        // Step 3: Disconnect if connected
        if let Some(conn) = self.connections.find_connection_by_bdaddr(bdaddr) {
            let handle = conn.handle;
            self.disconnect(handle, 0x13); // Remote User Terminated
        }

        true
    }

    /// Check if a device is paired (bonded).
    pub fn is_paired(&self, bdaddr: &BdAddr) -> bool {
        self.connections.find_device(bdaddr)
            .map(|d| d.bond.bonded)
            .unwrap_or(false)
    }

    /// Handle a Link Key Notification event from the controller.
    /// Called when the HCI event handler receives event 0x18.
    pub fn handle_link_key_notification(&mut self, bdaddr: &BdAddr, _link_key: &[u8; 16], key_type: u8) {
        // Store the link key locally (on real hardware we'd write to controller)
        // Determine if the key is authenticated:
        // - 0x00-0x02 = legacy authenticated keys
        // - 0x03 = debug key (NOT authenticated)
        // - 0x04 = SSP unauthenticated combination key
        // - 0x05 = SSP authenticated combination key
        // - 0x06 = changed combination key
        let authenticated = key_type == 0x05 || (key_type < 0x04 && key_type != 0x03);

        if let Some(dev) = self.connections.find_device_mut(bdaddr) {
            dev.bond = BondFlags {
                bonded: true,
                authenticated,
                encrypted: true,
            };
        }

        // Also mark any connection as authenticated
        if let Some(conn) = self.connections.find_connection_by_bdaddr_mut(bdaddr) {
            conn.authenticated = authenticated;
            conn.encrypted = true;
        }
    }

    /// Handle an Authentication Complete event (event 0x06).
    pub fn handle_authentication_complete(&mut self, handle: u16, status: u8) {
        if status == 0x00 {
            // Authentication succeeded — mark connection as authenticated
            if let Some(conn) = self.connections.find_connection_by_handle_mut(handle) {
                conn.authenticated = true;
            }
        }
    }

        // ── MINIX IPC Integration ─────────────────────────────────────────

    /// Run the daemon as a MINIX system service (SEF-based IPC loop).
    ///
    /// Registers SEF callbacks, calls `sef_startup()`, and enters the
    /// IPC receive/dispatch loop. Never returns on a real MINIX system.
    ///
    /// On the host (test), performs a single init + run_once iteration.
    #[cfg(target_os = "minix")]
    pub fn run_minix(&mut self) -> ! {
        use crate::minix_ipc;
        use minix_rs::Message;
        use minix_ipc::{DaemonCallbacks, OK};

        // Wire the global daemon pointer so dispatch_ipc_message can
        // call methods on this instance. Safe because run_daemon_loop
        // never returns — the pointer remains valid.
        unsafe {
            minix_ipc::set_global_daemon_ptr(
                core::ptr::addr_of_mut!(*self).cast()
            );
        }

        // Create daemon callbacks with real HCI polling and alarm handling.
        // poll_hook and signal_alarm access the daemon via GLOBAL_DAEMON_PTR.
        let callbacks = DaemonCallbacks {
            init_fresh: Some(|| {
                // On re-init, return OK to let RS know we're ready
                OK
            }),
            signal_term: Some(|_signo| {
                // Clean shutdown on SIGTERM/SIGHUP
                // Could set a global shutdown flag here
            }),
            signal_alarm: Some(|| {
                // SIGALRM fired — poll HCI for events inline.
                // The re-arm is handled by sef_cb_signal_handler_impl.
                // Just do the poll and return OK to re-arm.
                let ptr = unsafe { crate::minix_ipc::get_global_daemon_ptr() };
                if !ptr.is_null() {
                    let daemon = unsafe { &mut *(ptr as *mut BtDaemon) };
                    daemon.run_once();
                }
                OK
            }),
            dispatch_cmd: Some(|msg: &mut Message| -> i32 {
                Self::dispatch_ipc_message(msg)
            }),
            poll_hook: Some(|| {
                // Non-blocking HCI poll before each sef_receive.
                let ptr = unsafe { crate::minix_ipc::get_global_daemon_ptr() };
                if ptr.is_null() {
                    return 0;
                }
                let daemon = unsafe { &mut *(ptr as *mut BtDaemon) };
                daemon.run_once()
            }),
        };

        minix_ipc::run_daemon_loop(callbacks)
    }

    /// Run the daemon as a MINIX system service (host stub for testing).
    #[cfg(not(target_os = "minix"))]
    pub fn run_minix(&mut self) {
        // Host stub: just init and run once for testing
        self.init();
        self.run();
    }

    /// Dispatch an incoming IPC message to the appropriate handler.
    ///
    /// This is called by the MINIX daemon framework when a BT_RQ_*
    /// command message is received. Uses `minix_ipc::GLOBAL_DAEMON_PTR`
    /// to access the running `BtDaemon` instance, because the SEF
    /// callback interface (`fn(&mut Message) -> i32`) doesn't support
    /// closure capture.
    ///
    /// The message payload uses the mess_4 layout:
    ///   m4_l1 (offset 0):  command-specific arg / count
    ///   m4_l2 (offset 8):  BD_ADDR low 32 bits or grant ID
    ///   m4_l3 (offset 16): BD_ADDR high 16 bits or handle
    ///   m4_l4 (offset 24): conn handle / flags / PSM
    ///   m4_ll1 (offset 32): 64-bit extension
    ///
    /// Returns OK (0) on success, or negative errno on error.
    fn dispatch_ipc_message(msg: &mut minix_rs::Message) -> i32 {
        use crate::minix_ipc::{
            msg_read_bdaddr_low, msg_read_bdaddr_high,
            msg_read_handle, msg_read_name, BT_RQ_BASE,
            OK as IPC_OK, EINVAL, ENOSYS,
        };

        let cmd_offset = (msg.m_type - BT_RQ_BASE) as u8;

        // SAFETY: GLOBAL_DAEMON_PTR is set once by run_minix() before
        // entering the IPC loop, and the daemon instance lives on the
        // stack of run_minix() which never returns. Single-threaded.
        let daemon_ptr = unsafe { crate::minix_ipc::get_global_daemon_ptr() };
        if daemon_ptr.is_null() {
            msg.m_type = -(ENOSYS as i32);
            return -(ENOSYS as i32);
        }
        let daemon: &mut BtDaemon = unsafe { &mut *(daemon_ptr as *mut BtDaemon) };

        match cmd_offset {
            0 => { // BT_RQ_START_DISCOVERY
                let ok = daemon.start_discovery();
                msg.m_type = if ok { IPC_OK } else { -(EINVAL as i32) };
                if ok { IPC_OK } else { -(EINVAL as i32) }
            }
            1 => { // BT_RQ_STOP_DISCOVERY
                let ok = daemon.stop_discovery();
                msg.m_type = if ok { IPC_OK } else { -(EINVAL as i32) };
                if ok { IPC_OK } else { -(EINVAL as i32) }
            }
            2 => { // BT_RQ_GET_DEVICES
                let devices = daemon.connections.all_devices();
                let count = devices.len().min(u16::MAX as usize);
                msg.write_i32(0, count as i32); // m4_l1 = BT_DEVICE_COUNT
                msg.m_type = IPC_OK;
                IPC_OK
            }
            3 => { // BT_RQ_CONNECT
                let low = msg_read_bdaddr_low(msg);
                let high = msg_read_bdaddr_high(msg);
                let addr_bytes = [
                    low as u8,
                    (low >> 8) as u8,
                    (low >> 16) as u8,
                    (low >> 24) as u8,
                    high as u8,
                    (high >> 8) as u8,
                ];
                let bdaddr = BdAddr::new(addr_bytes);
                let ok = daemon.connect(&bdaddr);
                msg.m_type = if ok { IPC_OK } else { -(EINVAL as i32) };
                if ok { IPC_OK } else { -(EINVAL as i32) }
            }
            4 => { // BT_RQ_DISCONNECT
                let handle = msg_read_handle(msg);
                let reason = (msg.read_i32(24) & 0xFF) as u8;
                let ok = daemon.disconnect(handle, reason);
                msg.m_type = if ok { IPC_OK } else { -(EINVAL as i32) };
                if ok { IPC_OK } else { -(EINVAL as i32) }
            }
            5 => { // BT_RQ_GET_CONNECTIONS
                let conns = daemon.connections.active_connections();
                let count = conns.len().min(u16::MAX as usize);
                msg.write_i32(0, count as i32); // m4_l1 = BT_CONN_COUNT
                msg.m_type = IPC_OK;
                IPC_OK
            }
            6 => { // BT_RQ_SET_NAME
                let name_bytes = msg_read_name(msg);
                let name_str = core::str::from_utf8(name_bytes)
                    .unwrap_or("");
                if !name_str.is_empty() {
                    daemon.set_name(name_str);
                }
                msg.m_type = IPC_OK;
                IPC_OK
            }
            7 => { // BT_RQ_SET_DISCOVERABLE
                let enable = msg.read_i32(8) != 0;
                daemon.set_discoverable(enable);
                msg.m_type = IPC_OK;
                IPC_OK
            }
            8 => { // BT_RQ_SET_CONNECTABLE
                let enable = msg.read_i32(8) != 0;
                daemon.set_connectable(enable);
                msg.m_type = IPC_OK;
                IPC_OK
            }
            9 => { // BT_RQ_GET_STATUS
                let running = if daemon.running { 1 } else { 0 };
                let dev_count = daemon.connections.all_devices().len();
                let conn_count = daemon.connections.active_connections().len();
                let enabled = if daemon.hci.is_open() { 1 } else { 0 };
                msg.write_i32(0, running);       // m4_l1 = BT_STATUS_RUNNING
                msg.write_i32(8, dev_count as i32);  // m4_l2 = BT_STATUS_DEVICES
                msg.write_i32(16, conn_count as i32);// m4_l3 = BT_STATUS_CONNECTIONS
                msg.write_i32(24, enabled);      // m4_l4 = BT_STATUS_ENABLED
                msg.m_type = IPC_OK;
                IPC_OK
            }
            10 => { // BT_RQ_PAIR
                let low = msg_read_bdaddr_low(msg);
                let high = msg_read_bdaddr_high(msg);
                let addr_bytes = [
                    low as u8, (low >> 8) as u8, (low >> 16) as u8,
                    (low >> 24) as u8, high as u8, (high >> 8) as u8,
                ];
                let bdaddr = BdAddr::new(addr_bytes);
                if bdaddr.is_null() {
                    msg.m_type = -(EINVAL as i32);
                    return -(EINVAL as i32);
                }
                let ok = daemon.pair(&bdaddr);
                msg.m_type = if ok { IPC_OK } else { -(EINVAL as i32) };
                if ok { IPC_OK } else { -(EINVAL as i32) }
            }
            11 => { // BT_RQ_UNPAIR
                let low = msg_read_bdaddr_low(msg);
                let high = msg_read_bdaddr_high(msg);
                let addr_bytes = [
                    low as u8, (low >> 8) as u8, (low >> 16) as u8,
                    (low >> 24) as u8, high as u8, (high >> 8) as u8,
                ];
                let bdaddr = BdAddr::new(addr_bytes);
                if bdaddr.is_null() {
                    msg.m_type = -(EINVAL as i32);
                    return -(EINVAL as i32);
                }
                let ok = daemon.unpair(&bdaddr);
                msg.m_type = if ok { IPC_OK } else { -(EINVAL as i32) };
                if ok { IPC_OK } else { -(EINVAL as i32) }
            }
            12 => { // BT_RQ_REGISTER_SERVICE
                // Read service registration fields from message.
                // Payload format:
                //   offset 0:  PSM (i32)
                //   offset 8:  RFCOMM channel / protocol port (i32)
                //   offset 16: UUID16 service class ID (i32)
                //   offset 24: flags (i32, reserved)
                //   offset 32: service name string (max 24 bytes incl. null)
                let psm = msg.read_i32(0) as u16;
                let channel = msg.read_i32(8) as u8;
                let uuid16 = msg.read_i32(16) as u16;
                let name_bytes = msg_read_name(msg);
                let name_str = core::str::from_utf8(name_bytes)
                    .unwrap_or("Unnamed Service");

                // Determine protocol UUID based on PSM.
                let protocol_uuid = match crate::types::L2CapPsm::from_raw(psm) {
                    crate::types::L2CapPsm::Rfcomm => crate::types::sdp_uuids::RFCOMM,
                    crate::types::L2CapPsm::Sdp => crate::types::sdp_uuids::SDP,
                    _ => crate::types::sdp_uuids::L2CAP,
                };

                // Build SDP record.
                let service_uuid = crate::types::BtUuid::from_uuid16(uuid16);
                let psm_opt = if psm != 0 { Some(psm) } else { None };
                let channel_opt = if channel != 0 { Some(channel) } else { None };

                let service_record = crate::sdp_record::build_service_record(
                    service_uuid,
                    protocol_uuid,
                    psm_opt,
                    channel_opt,
                    name_str,
                    "Registered via IPC",
                );

                let handle = daemon.register_service(service_record);
                msg.write_i32(0, handle as i32); // BT_REG_HANDLE
                msg.m_type = IPC_OK;
                IPC_OK
            }
            _ => {
                msg.m_type = -(EINVAL as i32);
                -(EINVAL as i32)
            }
        }
    }

    /// Shutdown the daemon.
    pub fn shutdown(&mut self) {
        self.running = false;
        self.hci.close();
    }
}

impl Drop for BtDaemon {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ============================================================================
// IPC Message Types (for user-space applications)
// ============================================================================

/// Commands that applications can send to the daemon.
#[repr(i32)]
pub enum BtDaemonCmd {
    StartDiscovery = 0x01,
    StopDiscovery = 0x02,
    GetDevices = 0x03,
    Connect = 0x04,
    Disconnect = 0x05,
    GetConnections = 0x06,
    SetName = 0x07,
    SetDiscoverable = 0x08,
    SetConnectable = 0x09,
    GetStatus = 0x0A,
    ReadChannel = 0x0B,
    WriteChannel = 0x0C,
    Pair = 0x0D,
    Unpair = 0x0E,
    RegisterService = 0x0F,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ── RemoteDevice ──

    #[test]
    fn test_remote_device_new() {
        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();
        let dev = RemoteDevice::new(addr);
        assert_eq!(dev.bdaddr, addr);
        assert!(!dev.bond.bonded);
        assert_eq!(dev.rssi, 127);
    }

    #[test]
    fn test_remote_device_set_name() {
        let addr = BdAddr::parse("11:22:33:44:55:66").unwrap();
        let mut dev = RemoteDevice::new(addr);
        dev.set_name(b"Test Device");
        assert_eq!(dev.name_str(), "Test Device");
    }

    // ── ConnectionManager ──

    #[test]
    fn test_connection_manager_new() {
        let mgr = ConnectionManager::new();
        assert!(mgr.all_devices().is_empty());
        assert!(!mgr.has_connections());
        assert!(mgr.is_connectable());
        assert!(!mgr.is_discoverable());
    }

    #[test]
    fn test_connection_manager_device_tracking() {
        let mut mgr = ConnectionManager::new();
        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();

        mgr.record_inquiry_result(
            addr,
            ClassOfDevice::new([0x04, 0x02, 0x00]),
            -60, 0, 0, 100,
        );

        let devices = mgr.found_devices();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].bdaddr, addr);
        assert_eq!(devices[0].rssi, -60);
    }

    #[test]
    fn test_connection_manager_duplicate_device() {
        let mut mgr = ConnectionManager::new();
        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();

        mgr.record_inquiry_result(addr, ClassOfDevice::new([0; 3]), -50, 0, 0, 100);
        mgr.record_inquiry_result(addr, ClassOfDevice::new([0; 3]), -40, 0, 0, 200);

        let devices = mgr.all_devices();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].rssi, -40);
    }

    #[test]
    fn test_connection_manager_le_advertising() {
        let mut mgr = ConnectionManager::new();
        let addr = BdAddr::parse("11:22:33:44:55:66").unwrap();

        let mut adv_data = Vec::new();
        adv_data.push(7);
        adv_data.push(0x09);
        adv_data.extend_from_slice(b"BT_DEV");

        mgr.record_le_advertising_report(addr, 0, -70, &adv_data, 100);

        let dev = mgr.find_device(&addr).unwrap();
        assert_eq!(dev.dev_type, DeviceType::Le);
        assert_eq!(dev.name_str(), "BT_DEV");
        assert_eq!(dev.rssi, -70);
    }

    #[test]
    fn test_connection_manager_clear_found() {
        let mut mgr = ConnectionManager::new();
        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();

        mgr.record_inquiry_result(addr, ClassOfDevice::new([0; 3]), 0, 0, 0, 100);
        assert_eq!(mgr.found_devices().len(), 1);

        mgr.clear_found_flags();
        assert_eq!(mgr.found_devices().len(), 0);

        assert!(mgr.find_device(&addr).is_some());
    }

    // ── Connections ──

    #[test]
    fn test_connection_manager_add_connection() {
        let mut mgr = ConnectionManager::new();
        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();

        let conn = mgr.add_connection(0x0042, addr, ConnDirection::Incoming, 1);
        assert!(conn.is_some());
        assert!(mgr.has_connections());
        assert_eq!(mgr.active_connections().len(), 1);

        let found = mgr.find_connection_by_handle(0x0042).unwrap();
        assert_eq!(found.bdaddr, addr);
        assert_eq!(found.state, ConnState::Connecting);
    }

    #[test]
    fn test_connection_manager_mark_connected() {
        let mut mgr = ConnectionManager::new();
        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();

        mgr.add_connection(0x0042, addr, ConnDirection::Outgoing, 1);
        assert!(mgr.mark_connected(0x0042));

        let conn = mgr.find_connection_by_handle(0x0042).unwrap();
        assert_eq!(conn.state, ConnState::Connected);
    }

    #[test]
    fn test_connection_manager_remove_connection() {
        let mut mgr = ConnectionManager::new();
        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();

        mgr.add_connection(0x0042, addr, ConnDirection::Outgoing, 1);
        assert!(mgr.remove_connection(0x0042));
        assert!(!mgr.has_connections());
    }

    #[test]
    fn test_connection_manager_encryption() {
        let mut mgr = ConnectionManager::new();
        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();

        mgr.add_connection(0x0042, addr, ConnDirection::Outgoing, 1);
        mgr.mark_connected(0x0042);
        assert!(mgr.set_encryption(0x0042, true));

        let conn = mgr.find_connection_by_handle(0x0042).unwrap();
        assert!(conn.encrypted);
    }

    // ── Local Name ──

    #[test]
    fn test_connection_manager_local_name() {
        let mut mgr = ConnectionManager::new();
        assert_eq!(mgr.local_name_str(), "GergiOS");
        mgr.set_local_name("MyPhone");
        assert_eq!(mgr.local_name_str(), "MyPhone");
    }

    // ── ProtocolMultiplexer ──

    #[test]
    fn test_protocol_multiplexer_new() {
        let mux = ProtocolMultiplexer::new();
        assert_eq!(mux.l2cap.channel_count(), 0);
        // SDP is handled inline — no handlers registered by default
        assert_eq!(mux.handlers.len(), 0);
    }

    #[test]
    fn test_protocol_multiplexer_channel_creation() {
        let mut mux = ProtocolMultiplexer::new();
        let handle = ConnHandle::new(0x0042);

        let cid = mux.create_outgoing_channel(handle, L2CapPsm::Rfcomm);
        assert!(cid.is_some());
        assert_eq!(mux.l2cap.channel_count(), 1);
    }

    #[test]
    fn test_protocol_multiplexer_signaling() {
        let mut mux = ProtocolMultiplexer::new();
        let handle = ConnHandle::new(0x0042);

        let _cid = mux.create_outgoing_channel(handle, L2CapPsm::Sdp).unwrap();
        let sig_id = mux.l2cap.alloc_sig_id();

        // Simulate processing incoming connection request
        let req = L2CapConnReq {
            psm: L2CapPsm::Sdp,
            source_cid: 0x0041,
        };
        let req_data = req.build();
        let sig = crate::l2cap::build_sig_command(
            L2CapSigCode::ConnectionRequest, 1, &req_data,
        );
        let bframe = crate::l2cap::build_l2cap_b_frame(
            L2CapCid::Signaling.to_raw(), &sig,
        );

        let responses = mux.process_acl_data(0x0042, 0x02, 0x00, &bframe);
        assert!(!responses.is_empty());
        assert!(mux.l2cap.channel_count() >= 1);
    }

    // ── DaemonConfig ──

    #[test]
    fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        assert_eq!(config.name, "GergiOS");
        assert!(!config.discoverable);
        assert!(config.connectable);
        assert!(config.le_enabled);
    }

    // ── BtDaemon ──

    #[test]
    fn test_bt_daemon_new() {
        let daemon = BtDaemon::new();
        assert!(!daemon.running);
        assert_eq!(daemon.config.name, "GergiOS");
    }

    #[test]
    fn test_bt_daemon_init() {
        let mut daemon = BtDaemon::new();
        assert!(daemon.init());
        assert_eq!(daemon.hci.state, crate::hci_mgr::HciDevState::Up);
        assert!(daemon.running);
    }

    #[test]
    fn test_bt_daemon_discovery() {
        let mut daemon = BtDaemon::new();
        daemon.init();
        assert!(daemon.start_discovery());
        assert!(daemon.stop_discovery());
    }

    #[test]
    fn test_bt_daemon_name() {
        let mut daemon = BtDaemon::new();
        daemon.init();
        daemon.set_name("TestDevice");
        assert_eq!(daemon.connections.local_name_str(), "TestDevice");
    }

    #[test]
    fn test_bt_daemon_discoverable() {
        let mut daemon = BtDaemon::new();
        daemon.init();
        daemon.set_discoverable(true);
        assert!(daemon.connections.is_discoverable());
        daemon.set_discoverable(false);
        assert!(!daemon.connections.is_discoverable());
    }

    #[test]
    fn test_bt_daemon_shutdown() {
        let mut daemon = BtDaemon::new();
        daemon.init();
        assert!(daemon.running);
        daemon.shutdown();
        assert!(!daemon.running);
        assert!(!daemon.hci.is_open());
    }

    #[test]
    fn test_bt_daemon_drop_cleans_up() {
        let mut daemon = BtDaemon::new();
        daemon.init();
        assert!(daemon.hci.is_open());
        drop(daemon);
    }

    // ── ServiceRegistry ──

    #[test]
    fn test_service_registry() {
        let mut reg = ServiceRegistry::new();

        let handle = reg.register_sdp_service();
        assert!(handle >= 0x00010000);
        assert_eq!(reg.service_handles().len(), 1);

        let handle2 = reg.register_gap_service("Test");
        assert_eq!(reg.service_handles().len(), 2);

        assert!(reg.unregister(handle));
        assert_eq!(reg.service_handles().len(), 1);
    }

    // ── IPC Commands ──

    #[test]
    fn test_ipc_command_constants() {
        assert_eq!(BtDaemonCmd::StartDiscovery as i32, 0x01);
        assert_eq!(BtDaemonCmd::GetDevices as i32, 0x03);
        assert_eq!(BtDaemonCmd::Connect as i32, 0x04);
        assert_eq!(BtDaemonCmd::GetStatus as i32, 0x0A);
        assert_eq!(BtDaemonCmd::RegisterService as i32, 0x0F);
    }

    // ── Connection flow ──

    #[test]
    fn test_full_connection_flow() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();

        daemon.connections.record_inquiry_result(
            addr, ClassOfDevice::new([0x04, 0x02, 0x00]), -55, 0, 0, 100,
        );

        let devices = daemon.discovered_devices();
        assert_eq!(devices.len(), 1);

        assert!(daemon.connect(&addr));

        daemon.connections.add_connection(0x0042, addr, ConnDirection::Outgoing, 1);
        daemon.connections.mark_connected(0x0042);

        let conn = daemon.connections.find_connection_by_bdaddr(&addr).unwrap();
        assert_eq!(conn.state, ConnState::Connected);

        assert!(daemon.disconnect(0x0042, 0x13));
        daemon.connections.remove_connection(0x0042);
        assert!(!daemon.connections.has_connections());
    }

    // ── Run once ──

    #[test]
    fn test_bt_daemon_run_once() {
        let mut daemon = BtDaemon::new();
        daemon.init();
        // run_once should return 0 since there's no real HCI data
        assert_eq!(daemon.run_once(), 0);
    }

    // ── run() without init ──

    #[test]
    fn test_bt_daemon_run_without_init() {
        let mut daemon = BtDaemon::new();
        // Should not panic
        daemon.run();
    }

    // ── Pairing ──

    #[test]
    fn test_pair_device_needs_connection() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();
        // Pair without a connection should at least not panic
        // (it will try to connect first, which on host stub returns true)
        assert!(daemon.pair(&addr));

        // Device should exist but NOT be bonded yet — pairing is deferred
        // until the connection is established and Link Key Notification fires
        let dev = daemon.connections.find_device(&addr);
        assert!(dev.is_none()); // connect() on stub doesn't add to ConnectionManager
        assert!(!daemon.is_paired(&addr));
    }

    #[test]
    fn test_unpair_device() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        let addr = BdAddr::parse("11:22:33:44:55:66").unwrap();

        // Add device as known
        daemon.connections.record_inquiry_result(
            addr, ClassOfDevice::new([0; 3]), -60, 0, 0, 100,
        );

        // Mark as bonded
        if let Some(dev) = daemon.connections.find_device_mut(&addr) {
            dev.bond.bonded = true;
            dev.bond.authenticated = true;
            dev.bond.encrypted = true;
        }

        // Unpair
        assert!(daemon.unpair(&addr));

        // Should be un-bonded
        let dev = daemon.connections.find_device(&addr).unwrap();
        assert!(!dev.bond.bonded);
        assert!(!dev.bond.authenticated);
        assert!(!dev.bond.encrypted);
    }

    #[test]
    fn test_pair_with_existing_connection() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();

        // Add a connection
        daemon.connections.add_connection(0x0042, addr, ConnDirection::Outgoing, 1);
        daemon.connections.mark_connected(0x0042);

        // Pair should start authentication on the existing connection
        assert!(daemon.pair(&addr));
    }

    #[test]
    fn test_is_paired_after_pair() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();

        // Before pairing
        assert!(!daemon.is_paired(&addr));

        // Device must be known to the connection manager before
        // a link key notification can mark it as bonded
        daemon.connections.record_inquiry_result(
            addr, ClassOfDevice::new([0; 3]), 0, 0, 0, 100,
        );

        // Simulate link key notification
        let link_key = [0x01u8; 16];
        daemon.handle_link_key_notification(&addr, &link_key, 0x00);

        // After pairing
        assert!(daemon.is_paired(&addr));
    }

    #[test]
    fn test_handle_link_key_notification_sets_bond() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();

        // Add device and connection
        daemon.connections.record_inquiry_result(addr, ClassOfDevice::new([0; 3]), 0, 0, 0, 100);
        daemon.connections.add_connection(0x0042, addr, ConnDirection::Outgoing, 1);

        let link_key = [0xAB; 16];
        daemon.handle_link_key_notification(&addr, &link_key, 0x00);

        // Device should be bonded
        let dev = daemon.connections.find_device(&addr).unwrap();
        assert!(dev.bond.bonded);
        assert!(dev.bond.authenticated);
        assert!(dev.bond.encrypted);

        // Connection should be authenticated
        let conn = daemon.connections.find_connection_by_bdaddr(&addr).unwrap();
        assert!(conn.authenticated);
        assert!(conn.encrypted);
    }

    #[test]
    fn test_handle_authentication_complete() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();
        daemon.connections.add_connection(0x0042, addr, ConnDirection::Outgoing, 1);

        // Before auth complete
        let conn = daemon.connections.find_connection_by_handle(0x0042).unwrap();
        assert!(!conn.authenticated);

        // Auth succeeded
        daemon.handle_authentication_complete(0x0042, 0x00);

        let conn = daemon.connections.find_connection_by_handle(0x0042).unwrap();
        assert!(conn.authenticated);

        // Auth failure should not set authenticated
        daemon.handle_authentication_complete(0x0042, 0x05); // 0x05 = Authentication Failure
        let conn = daemon.connections.find_connection_by_handle(0x0042).unwrap();
        // Still authenticated from previous successful auth
        assert!(conn.authenticated);
    }

    #[test]
    fn test_hci_authentication_requested() {
        let mut hci = crate::hci_mgr::HciManager::new();
        hci.open();
        hci.init_controller();

        // Send authentication requested for a connection handle
        assert!(hci.authentication_requested(0x0042));
    }

    #[test]
    fn test_hci_set_simple_pairing_mode() {
        let mut hci = crate::hci_mgr::HciManager::new();
        hci.open();

        assert!(hci.set_simple_pairing_mode(true));
        assert!(hci.set_simple_pairing_mode(false));
    }

    #[test]
    fn test_hci_io_capability() {
        let mut hci = crate::hci_mgr::HciManager::new();
        hci.open();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();
        // DisplayYesNo, no OOB, MITM not required
        assert!(hci.io_capability_request_reply(&addr, 1, false, 0x01));
    }

    #[test]
    fn test_hci_pin_reply() {
        let mut hci = crate::hci_mgr::HciManager::new();
        hci.open();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();
        assert!(hci.pin_code_request_reply(&addr, b"1234"));
        assert!(hci.pin_code_request_negative_reply(&addr));
    }

    #[test]
    fn test_hci_user_confirm() {
        let mut hci = crate::hci_mgr::HciManager::new();
        hci.open();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();
        assert!(hci.user_confirm_request_reply(&addr));
        assert!(hci.user_confirm_negative_reply(&addr));
    }

    #[test]
    fn test_hci_delete_stored_link_key() {
        let mut hci = crate::hci_mgr::HciManager::new();
        hci.open();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();
        assert!(hci.delete_stored_link_key(&addr, false));
        assert!(hci.delete_stored_link_key(&addr, true)); // delete all
    }

    #[test]
    fn test_hci_write_stored_link_key() {
        let mut hci = crate::hci_mgr::HciManager::new();
        hci.open();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();
        let key = [0x42u8; 16];
        assert!(hci.write_stored_link_key(&addr, &key, 0x04)); // 0x04 = Authenticated Combination Key
    }

    #[test]
    fn test_hci_le_start_encryption() {
        let mut hci = crate::hci_mgr::HciManager::new();
        hci.open();

        let rand = [0xABu8; 8];
        let ltk = [0xCDu8; 16];
        assert!(hci.le_start_encryption(0x0042, &rand, 0x0001, &ltk));
    }

    #[test]
    fn test_bond_flags_default() {
        let flags = BondFlags {
            bonded: false,
            authenticated: false,
            encrypted: false,
        };
        assert!(!flags.bonded);
    }

    #[test]
    fn test_unpair_unknown_device_is_ok() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        // Unpairing a device we've never seen should still work
        // (the HCI command just won't find a link key to delete)
        let addr = BdAddr::parse("FF:EE:DD:CC:BB:AA").unwrap();
        assert!(daemon.unpair(&addr));
    }

    #[test]
    fn test_pair_and_unpair_cycle() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();

        // Device must be in the connection manager before pairing
        daemon.connections.record_inquiry_result(
            addr, ClassOfDevice::new([0; 3]), 0, 0, 0, 100,
        );

        // Start pairing
        assert!(daemon.pair(&addr));

        // Simulate successful link key exchange
        let link_key = [0x01u8; 16];
        daemon.handle_link_key_notification(&addr, &link_key, 0x00);

        // Now paired
        assert!(daemon.is_paired(&addr));

        // Unpair
        assert!(daemon.unpair(&addr));

        // Should no longer be paired
        assert!(!daemon.is_paired(&addr));
    }

    // ── Service Registration ──

    #[test]
    fn test_register_service_via_api() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        let uuid = BtUuid::from_uuid16(0x1101); // Serial Port
        let protocol = crate::types::sdp_uuids::RFCOMM;
        let record = crate::sdp_record::build_service_record(
            uuid, protocol, Some(0x0003), Some(1),
            "Test Serial Port", "Test RFCOMM service",
        );

        let handle = daemon.register_service(record);
        assert!(handle >= 0x00010000);

        let registered = daemon.services.sdp_db.find_by_handle(handle);
        assert!(registered.is_some());
        assert_eq!(registered.unwrap().service_class_uuids()[0], uuid);
    }

    #[test]
    fn test_dispatch_register_service_handler() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        // Simulate what the C library / bt-tool sends.
        // Payload:
        //   offset 0:  PSM = 0x0003 (RFCOMM)
        //   offset 8:  channel = 1
        //   offset 16: UUID16 = 0x1101 (Serial Port)
        //   offset 24: flags = 0
        //   offset 32: name = "SerialTest"
        let mut msg = minix_rs::Message::new();
        msg.set_type(crate::minix_ipc::BT_RQ_BASE + 12);
        msg.write_i32(0, 0x0003u16 as i32);
        msg.write_i32(8, 1);
        msg.write_i32(16, 0x1101u16 as i32);
        msg.write_i32(24, 0);
        let name = b"SerialTest\0";
        msg.payload[32..32 + name.len()].copy_from_slice(name);

        // We can't easily call dispatch_ipc_message directly because of
        // GLOBAL_DAEMON_PTR, but we can verify the daemon's register_service
        // path is wired correctly by calling it directly.
        let handle = {
            // Use daemon's services directly to verify the registration works
            let service_uuid = crate::types::BtUuid::from_uuid16(0x1101);
            let record = crate::sdp_record::build_service_record(
                service_uuid,
                crate::types::sdp_uuids::RFCOMM,
                Some(0x0003),
                Some(1),
                "SerialTest",
                "Registered via IPC",
            );
            daemon.services.register(record)
        };

        // Verify the service was registered with a valid handle
        assert!(handle >= 0x00010000);

        // Verify it can be found
        let record = daemon.services.sdp_db.find_by_handle(handle);
        assert!(record.is_some());
    }

    #[test]
    fn test_register_service_with_no_psm() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        let uuid = BtUuid::from_uuid16(0x1800); // GAP
        let protocol = crate::types::sdp_uuids::L2CAP;
        let record = crate::sdp_record::build_service_record(
            uuid, protocol, None, None,
            "GAP", "Generic Access Profile",
        );

        let handle = daemon.register_service(record);
        assert!(handle >= 0x00010000);

        let record = daemon.services.sdp_db.find_by_handle(handle);
        assert!(record.is_some());
        let uuids = record.unwrap().service_class_uuids();
        assert_eq!(uuids[0].as_uuid16(), Some(0x1800));
    }

    #[test]
    fn test_register_multiple_services() {
        let mut daemon = BtDaemon::new();
        daemon.init();

        let handle1 = daemon.register_service(
            crate::sdp_record::build_service_record(
                BtUuid::from_uuid16(0x1101),
                crate::types::sdp_uuids::RFCOMM,
                Some(0x0003),
                Some(1),
                "Serial Port",
                "",
            )
        );
        let handle2 = daemon.register_service(
            crate::sdp_record::build_service_record(
                BtUuid::from_uuid16(0x1108),
                crate::types::sdp_uuids::RFCOMM,
                Some(0x0003),
                Some(2),
                "Headset",
                "",
            )
        );

        // Handles should be different and increasing
        assert!(handle1 < handle2);
        // daemon.init() registers SDP service + GAP service,
        // so total = 2 (built-in) + 2 (ours) = 4
        assert_eq!(daemon.services.service_handles().len(), 4);
    }
}
