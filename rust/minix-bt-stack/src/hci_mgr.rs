//! # HCI Manager — HCI Device Management
//!
//! Manages the HCI transport device (`/dev/hci0`) for the Bluetooth daemon.
//! Provides:
//!
//! - Opening/closing the HCI character device
//! - Sending HCI commands and receiving events
//! - HCI device state machine (Reset → Configuring → Up → Down)
//! - Controller initialisation sequence (reset, read version, BD_ADDR, etc.)
//! - Event dispatching to registered callbacks
//! - Periodic polling for events
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────┐     ┌──────────────┐
//! │  BT Daemon Core  │────▶│  HciManager  │
//! │  (bt_daemon.rs)  │     │              │
//! │  ┌────────────┐  │     │  - open()    │
//! │  │Connection  │  │     │  - cmd()     │
//! │  │Manager     │  │     │  - poll()    │
//! │  ├────────────┤  │     │  - events    │
//! │  │Protocol    │  │     └──────┬───────┘
//! │  │Multiplexer │  │            │ send/recv
//! │  ├────────────┤  │     ┌──────▼───────┐
//! │  │Service     │  │     │   /dev/hci0  │
//! │  │Registry    │  │     │  chardev     │
//! │  └────────────┘  │     └──────────────┘
//! └──────────────────┘
//! ```

#![allow(dead_code)]

use crate::types::BdAddr;

use core::ffi::c_int;

// ============================================================================
// HCI Device State
// ============================================================================

/// State of the HCI device as managed by the daemon.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HciDevState {
    /// Not opened or initialised.
    Down,
    /// Opening /dev/hci0.
    Opening,
    /// HCI Reset in progress.
    Resetting,
    /// Reading controller version / features.
    Configuring,
    /// Controller is operational.
    Up,
    /// Fatal error — needs reset.
    Error,
}

// ============================================================================
// HCI Device Info
// ============================================================================

/// Information about the HCI controller, read during init.
#[derive(Clone, Debug)]
pub struct HciDevInfo {
    /// Local BD_ADDR.
    pub bdaddr: BdAddr,
    /// HCI version (spec version).
    pub hci_version: u8,
    /// HCI revision (manufacturer-specific).
    pub hci_revision: u16,
    /// LMP version.
    pub lmp_version: u8,
    /// LMP subversion.
    pub lmp_subversion: u16,
    /// Manufacturer ID (from Bluetooth SIG).
    pub manufacturer: u16,
    /// ACL data packet length.
    pub acl_mtu: u16,
    /// SCO data packet length.
    pub sco_mtu: u8,
    /// Total number of ACL buffers.
    pub acl_pkts: u16,
    /// Device name (local name).
    pub name: [u8; 248],
    /// Device class.
    pub class_of_device: [u8; 3],
}

impl HciDevInfo {
    pub fn new() -> Self {
        Self {
            bdaddr: BdAddr::null(),
            hci_version: 0,
            hci_revision: 0,
            lmp_version: 0,
            lmp_subversion: 0,
            manufacturer: 0,
            acl_mtu: 0,
            sco_mtu: 0,
            acl_pkts: 0,
            name: [0u8; 248],
            class_of_device: [0u8; 3],
        }
    }
}

// ============================================================================
// Event Callback Types
// ============================================================================

/// Callback for HCI events (Command Complete, LE Meta, etc.).
/// Receives the raw event bytes (including event code + params).
pub type HciEventCallback = Box<dyn FnMut(u8, &[u8]) + Send>;

/// Callback for ACL data received from the controller.
/// Receives the raw ACL data (handle+flags + payload).
pub type HciAclCallback = Box<dyn FnMut(u16, u8, u8, &[u8]) + Send>;

/// Callback for SCO data received from the controller.
pub type HciScoCallback = Box<dyn FnMut(u16, &[u8]) + Send>;

// ============================================================================
// HCI Manager
// ============================================================================

/// Manages the HCI device — open, command, event, state.
pub struct HciManager {
    /// Current device state.
    pub state: HciDevState,
    /// Device info (populated after init).
    pub dev_info: HciDevInfo,
    /// The file descriptor for /dev/hci0 (if opened).
    fd: Option<c_int>,
    /// Event callbacks (keyed by event code).
    event_handlers: Vec<(u8, HciEventCallback)>,
    /// ACL data callback (one handler, typically the protocol multiplexer).
    acl_handler: Option<HciAclCallback>,
    /// SCO data callback.
    sco_handler: Option<HciScoCallback>,
    /// Number of sent commands awaiting Command Complete/Status events.
    pending_cmds: u32,
    /// Generated command sequence number (for matching responses).
    cmd_seq: u32,
}

impl HciManager {
    /// Create a new HCI manager (initially Down).
    pub fn new() -> Self {
        Self {
            state: HciDevState::Down,
            dev_info: HciDevInfo::new(),
            fd: None,
            event_handlers: Vec::new(),
            acl_handler: None,
            sco_handler: None,
            pending_cmds: 0,
            cmd_seq: 0,
        }
    }

    // ── Device Lifecycle ──────────────────────────────────────────────

    /// Open the HCI device. On MINIX this opens `/dev/hci0` via VFS IPC.
    /// On host (test), uses a file descriptor stubs.
    ///
    /// Returns true on success.
    pub fn open(&mut self) -> bool {
        if self.fd.is_some() {
            return true; // Already open
        }

        self.state = HciDevState::Opening;

        #[cfg(target_os = "minix")]
        {
            // On real MINIX, open /dev/hci0 via VFS IPC
            // This is a placeholder for the actual VFS open call
            // which would use sendrec to VFS with VFS_OPEN
            self.fd = Some(0); // Placeholder
        }

        #[cfg(not(target_os = "minix"))]
        {
            // On host (test), stub
            self.fd = Some(1); // Simulated fd
        }

        self.state = HciDevState::Down;
        self.fd.is_some()
    }

    /// Close the HCI device.
    pub fn close(&mut self) {
        if let Some(_fd) = self.fd.take() {
            #[cfg(target_os = "minix")]
            {
                // VFS_CLOSE via IPC
            }
        }
        self.state = HciDevState::Down;
        self.pending_cmds = 0;
    }

    /// Check if the device is open.
    pub fn is_open(&self) -> bool {
        self.fd.is_some()
    }

    // ── IOCTL ─────────────────────────────────────────────────────────

    /// Send an ioctl to the HCI device.
    /// On MINIX this sends VFS_IOCTL via IPC to VFS.
    /// Returns 0 on success, negative errno on error.
    pub fn ioctl(&self, _request: u64, _arg: u64) -> c_int {
        if self.fd.is_none() {
            return -16; // EBUSY
        }

        #[cfg(target_os = "minix")]
        {
            // Placeholder: send VFS_IOCTL message
            // let mut msg = minix_rs::Message::new();
            // msg.set_type(minix_rs::VFS_IOCTL);
            // ... setup request/grant ...
            // minix_rs::sendrec(minix_rs::VFS_PROC_NR, minix_rs::VFS_IOCTL, &mut msg)
            0
        }

        #[cfg(not(target_os = "minix"))]
        {
            0 // Stub
        }
    }

    // ── HCI Commands ──────────────────────────────────────────────────

    /// Send an HCI command to the controller.
    /// `ogf` = OpCode Group Field, `ocf` = OpCode Command Field.
    /// `params` = command parameters (without header).
    ///
    /// Returns true if the command was sent successfully.
    pub fn send_cmd(&mut self, ogf: u8, ocf: u16, params: &[u8]) -> bool {
        let opcode = ((ogf as u16) << 10) | (ocf & 0x03FF);
        let param_len = params.len() as u8;

        // Build HCI command packet: [Type(1) | OpCode(2) | ParamLen(1) | Params(N)]
        let mut pkt = Vec::with_capacity(4 + params.len());
        pkt.push(0x01); // HCI Command type
        pkt.extend_from_slice(&opcode.to_le_bytes());
        pkt.push(param_len);
        pkt.extend_from_slice(params);

        let success = self.write_hci(&pkt);
        if success {
            self.pending_cmds += 1;
            self.cmd_seq += 1;
        }
        success
    }

    /// Write raw HCI data to the device (type byte + payload).
    fn write_hci(&self, data: &[u8]) -> bool {
        if self.fd.is_none() || data.is_empty() {
            return false;
        }

        #[cfg(target_os = "minix")]
        {
            // Placeholder: send VFS_WRITE via IPC
            true
        }

        #[cfg(not(target_os = "minix"))]
        {
            // Host stub — accept writes silently
            true
        }
    }

    /// Read raw HCI data from the device.
    /// Returns the number of bytes read, or 0 if none available.
    fn read_hci(&self, buf: &mut [u8]) -> usize {
        if self.fd.is_none() || buf.is_empty() {
            return 0;
        }

        #[cfg(target_os = "minix")]
        {
            // Placeholder: send VFS_READ via IPC
            0
        }

        #[cfg(not(target_os = "minix"))]
        {
            // Host stub — no data available
            0
        }
    }

    // ── Polling / Event Dispatch ──────────────────────────────────────

    /// Poll the HCI device for incoming events and data.
    /// Should be called periodically from the daemon's main loop.
    ///
    /// Returns the number of events processed.
    pub fn poll(&mut self) -> usize {
        if self.fd.is_none() || self.state != HciDevState::Up {
            return 0;
        }

        let mut count = 0;
        let mut buf = [0u8; 65536]; // Max ACL packet

        loop {
            let n = self.read_hci(&mut buf);
            if n == 0 {
                break;
            }

            let pkt_type = buf[0];
            let payload = &buf[1..n];

            match pkt_type {
                0x01 => {
                    // HCI Command Complete/Status (echo back — shouldn't happen)
                }
                0x02 => {
                    // ACL Data
                    self.dispatch_acl(payload);
                }
                0x03 => {
                    // SCO Data
                    self.dispatch_sco(payload);
                }
                0x04 => {
                    // HCI Event
                    self.dispatch_event(payload);
                }
                _ => {}
            }

            count += 1;
            if count >= 16 {
                break; // Limit iterations per poll
            }
        }

        count
    }

    /// Dispatch an HCI event to registered handlers.
    fn dispatch_event(&mut self, payload: &[u8]) {
        if payload.len() < 2 {
            return;
        }

        let event_code = payload[0];
        let event_len = payload[1] as usize;

        if payload.len() < 2 + event_len {
            return;
        }

        let event_params = &payload[2..2 + event_len];

        // Decrement pending command count for Command Complete / Status
        if event_code == 0x0E || event_code == 0x0F {
            self.pending_cmds = self.pending_cmds.saturating_sub(1);
        }

        // Dispatch to registered handlers
        for (code, handler) in &mut self.event_handlers {
            if *code == event_code || *code == 0x00 {
                // 0x00 = wildcard (all events)
                handler(event_code, event_params);
            }
        }
    }

    /// Dispatch ACL data to the registered ACL handler.
    fn dispatch_acl(&mut self, payload: &[u8]) {
        if payload.len() < 4 {
            return;
        }

        let handle_flags = u16::from_le_bytes([payload[0], payload[1]]);
        let handle = handle_flags & 0x0FFF;
        let pb_flag = ((handle_flags >> 12) & 0x03) as u8;
        let bc_flag = ((handle_flags >> 14) & 0x03) as u8;
        let data_len = u16::from_le_bytes([payload[2], payload[3]]) as usize;

        if payload.len() < 4 + data_len {
            return;
        }

        let data = &payload[4..4 + data_len];

        if let Some(ref mut handler) = self.acl_handler {
            handler(handle, pb_flag, bc_flag, data);
        }
    }

    /// Dispatch SCO data to the registered SCO handler.
    fn dispatch_sco(&mut self, payload: &[u8]) {
        if payload.len() < 3 {
            return;
        }

        let handle = u16::from_le_bytes([payload[0] & 0x0F, payload[1] & 0x0F]);
        let data_len = payload[2] as usize;

        if payload.len() < 3 + data_len {
            return;
        }

        let data = &payload[3..3 + data_len];

        if let Some(ref mut handler) = self.sco_handler {
            handler(handle, data);
        }
    }

    // ── Event Handler Registration ────────────────────────────────────

    /// Register a handler for a specific HCI event code.
    /// Use `event_code = 0` for a wildcard handler that receives all events.
    pub fn on_event<F>(&mut self, event_code: u8, handler: F)
    where
        F: FnMut(u8, &[u8]) + 'static + Send,
    {
        self.event_handlers.push((event_code, Box::new(handler)));
    }

    /// Set the ACL data handler (usually the protocol multiplexer).
    pub fn on_acl<F>(&mut self, handler: F)
    where
        F: FnMut(u16, u8, u8, &[u8]) + 'static + Send,
    {
        self.acl_handler = Some(Box::new(handler));
    }

    /// Set the SCO data handler.
    pub fn on_sco<F>(&mut self, handler: F)
    where
        F: FnMut(u16, &[u8]) + 'static + Send,
    {
        self.sco_handler = Some(Box::new(handler));
    }

    // ── Controller Initialisation ─────────────────────────────────────

    /// Run the full HCI controller initialisation sequence.
    ///
    /// Steps:
    /// 1. HCI Reset
    /// 2. Read Local Version
    /// 3. Read BD_ADDR
    /// 4. Read Buffer Size
    /// 5. Set Event Mask (enable all)
    /// 6. Write LE Host Support (enable BLE)
    /// 7. Write Local Name
    /// 8. Read Class of Device
    /// 9. Read Local Features
    ///
    /// Returns true if all steps succeeded.
    pub fn init_controller(&mut self) -> bool {
        if self.fd.is_none() {
            return false;
        }

        self.state = HciDevState::Resetting;

        // Step 1: Reset
        if !self.send_cmd(0x03, 0x0003, &[]) {
            // OGF=0x03 (Ctrl+Baseband), OCF=0x0003 (Reset)
            self.state = HciDevState::Error;
            return false;
        }

        // On real hardware, we'd wait for Command Complete here.
        // For the init sequence, we simulate success.
        self.state = HciDevState::Configuring;

        // Step 2: Read Local Version
        self.send_cmd(0x04, 0x0001, &[]); // OGF=0x04 (Info), OCF=0x0001

        // Populate with simulated version info for now
        self.dev_info.hci_version = 12; // BT 5.3
        self.dev_info.hci_revision = 0x1000;
        self.dev_info.lmp_version = 12;
        self.dev_info.lmp_subversion = 0x1000;
        self.dev_info.manufacturer = 2; // Intel (example)

        // Step 3: Read BD_ADDR
        self.send_cmd(0x04, 0x0009, &[]); // OGF=0x04, OCF=0x0009

        // Step 4: Read Buffer Size
        self.send_cmd(0x04, 0x0005, &[]); // OGF=0x04, OCF=0x0005
        self.dev_info.acl_mtu = 1024;
        self.dev_info.sco_mtu = 64;
        self.dev_info.acl_pkts = 16;

        // Step 5: Set Event Mask (enable all BR/EDR events)
        let all_events = [0xFFu8; 8];
        self.send_cmd(0x03, 0x0001, &all_events); // OGF=0x03, OCF=0x0001

        // Step 6: Write LE Host Support
        self.send_cmd(0x03, 0x006D, &[0x01]); // OGF=0x03, OCF=0x006D

        // Step 7: Write Local Name
        let mut name = [0u8; 248];
        let name_bytes = b"GergiOS BT Adapter";
        name[..name_bytes.len()].copy_from_slice(name_bytes);
        self.send_cmd(0x03, 0x0013, &name); // OGF=0x03, OCF=0x0013
        self.dev_info.name = name;

        // Step 8: Read Class of Device
        self.send_cmd(0x03, 0x0023, &[]); // OGF=0x03, OCF=0x0023
        self.dev_info.class_of_device = [0x00, 0x1E, 0x00]; // Computer, Laptop

        // Set fake BD_ADDR for testing (stub doesn't read from real hardware)
        self.dev_info.bdaddr = BdAddr::parse("00:11:22:33:44:55").unwrap_or(BdAddr::null());

        // Step 9: Read Local Features
        self.send_cmd(0x04, 0x0003, &[]); // OGF=0x04, OCF=0x0003

        // Mark as Up
        self.state = HciDevState::Up;

        // Clear pending command counter (simulated)
        self.pending_cmds = 0;

        true
    }

    /// Reset the controller (asynchronous — will generate Command Complete).
    pub fn reset_controller(&mut self) -> bool {
        self.send_cmd(0x03, 0x0003, &[])
    }

    /// Set scan mode (page scan and/or inquiry scan).
    ///
    /// `scan_mode`: 0 = none, 1 = page scan, 2 = inquiry scan, 3 = both.
    pub fn set_scan_mode(&mut self, scan_mode: u8) -> bool {
        self.send_cmd(0x03, 0x001A, &[scan_mode & 0x03])
        // OGF=0x03, OCF=0x001A (Write Scan Enable)
    }

    /// Start an HCI inquiry to discover nearby devices.
    ///
    /// `lap`: Lower Address Part (standard = 0x9E8B33).
    /// `inquiry_length`: max 0x30 (≈ 61 seconds).
    /// `num_responses`: 0 = unlimited.
    pub fn start_inquiry(&mut self, lap: [u8; 3], inquiry_length: u8, num_responses: u8) -> bool {
        let mut params = Vec::with_capacity(5);
        params.extend_from_slice(&lap);
        params.push(inquiry_length);
        params.push(num_responses);
        self.send_cmd(0x01, 0x0001, &params) // OGF=0x01 (Link Ctrl), OCF=0x0001
    }

    /// Cancel an ongoing inquiry.
    pub fn stop_inquiry(&mut self) -> bool {
        self.send_cmd(0x01, 0x0002, &[]) // OGF=0x01, OCF=0x0002
    }

    /// Create a connection to a remote device.
    ///
    /// BD_ADDR is transmitted LSB-first (natural byte order of BdAddr).
    pub fn create_connection(&mut self, bdaddr: &BdAddr, pkt_type: u16) -> bool {
        let mut params = Vec::with_capacity(13);
        // BD_ADDR on HCI wire: LSB-first = bytes in stored order
        // BdAddr stores bytes as [LSB, ..., MSB], so we send them directly
        for i in 0..6 {
            params.push(bdaddr.0[i]);
        }
        params.extend_from_slice(&pkt_type.to_le_bytes()); // Packet type
        params.push(0x00); // Page scan repetition mode
        params.push(0x00); // Reserved
        params.extend_from_slice(&[0x00, 0x00]); // Clock offset
        params.push(0x00); // Allow role switch (0 = allow)

        self.send_cmd(0x01, 0x0005, &params) // OGF=0x01, OCF=0x0005
    }

    /// Disconnect a connection.
    pub fn disconnect(&mut self, handle: u16, reason: u8) -> bool {
        let params = [handle.to_le_bytes()[0], handle.to_le_bytes()[1], reason];
        self.send_cmd(0x01, 0x0006, &params) // OGF=0x01, OCF=0x0006
    }

    /// Create a BLE connection to a device.
    pub fn le_create_connection(&mut self, bdaddr: &BdAddr, addr_type: u8) -> bool {
        let mut params = Vec::with_capacity(25);
        // LE_Create_Connection parameters
        params.extend_from_slice(&[0x00, 0x00]); // Scan interval
        params.extend_from_slice(&[0x00, 0x00]); // Scan window
        params.push(0x00); // Initiator filter policy (0 = not use whitelist)
        params.push(addr_type); // Peer address type
        // BD_ADDR on HCI wire: LSB-first = bytes in stored order
        for i in 0..6 {
            params.push(bdaddr.0[i]);
        }
        params.push(addr_type); // Own address type
        params.extend_from_slice(&[0x00, 0x00]); // Min connection interval
        params.extend_from_slice(&[0x00, 0x00]); // Max connection interval
        params.push(0x00); // Connection latency
        params.extend_from_slice(&[0x00, 0x00]); // Supervision timeout
        params.extend_from_slice(&[0x00, 0x00]); // Min CE length
        params.extend_from_slice(&[0x00, 0x00]); // Max CE length

        self.send_cmd(0x08, 0x000D, &params) // OGF=0x08 (LE), OCF=0x000D
    }

    /// Set BLE advertising parameters.
    pub fn le_set_adv_params(&mut self, interval_min: u16, interval_max: u16) -> bool {
        let mut params = Vec::with_capacity(15);
        params.extend_from_slice(&interval_min.to_le_bytes());
        params.extend_from_slice(&interval_max.to_le_bytes());
        params.push(0x00); // Advertising type (0 = connectable undirected)
        params.push(0x00); // Own address type
        params.push(0x00); // Peer address type
        params.extend_from_slice(&[0u8; 6]); // Peer address
        params.push(0x00); // Filter policy (0 = any)
        params.push(0x07); // TX power (7 = high)
        // BT 5.0+ only: Primary advertising PHY, secondary, skip, SID, scan req notify
        // We omit those for BT 4.x compatibility.

        self.send_cmd(0x08, 0x0006, &params)
    }

    /// Enable/disable BLE advertising.
    pub fn le_set_adv_enable(&mut self, enable: bool) -> bool {
        self.send_cmd(0x08, 0x000A, &[enable as u8])
    }

    // ── Pairing / Security ────────────────────────────────────────────

    /// Initiate BR/EDR authentication (pairing) on a connection.
    /// OGF=0x01 (Link Control), OCF=0x0020 (Authentication_Requested).
    /// Params: handle(2) — connection handle.
    pub fn authentication_requested(&mut self, handle: u16) -> bool {
        self.send_cmd(0x01, 0x0020, &handle.to_le_bytes())
    }

    /// Enable or disable Secure Simple Pairing (SSP) mode.
    /// OGF=0x03 (Host Controller), OCF=0x0058 (Write_Simple_Pairing_Mode).
    /// Params: mode(1) — 0=disabled, 1=enabled.
    pub fn set_simple_pairing_mode(&mut self, enable: bool) -> bool {
        self.send_cmd(0x03, 0x0058, &[enable as u8])
    }

    /// Set local IO Capability for SSP.
    /// OGF=0x01, OCF=0x002B (IO_Capability_Request_Reply).
    /// Params: bdaddr(6) | io_capability(1) | oob_data(1) | auth_requirements(1).
    /// io_capability: 0=DisplayOnly, 1=DisplayYesNo, 2=KeyboardOnly, 3=NoInputNoOutput.
    pub fn io_capability_request_reply(
        &mut self,
        bdaddr: &BdAddr,
        io_capability: u8,
        oob_data: bool,
        auth_requirements: u8,
    ) -> bool {
        let mut params = Vec::with_capacity(9);
        params.extend_from_slice(&bdaddr.0); // BD_ADDR (6 bytes, LSB-first)
        params.push(io_capability & 0x03);   // IO Capability
        params.push(if oob_data { 0x01 } else { 0x00 }); // OOB Data Present
        params.push(auth_requirements & 0x07); // Authentication Requirements
        self.send_cmd(0x01, 0x002B, &params)
    }

    /// Reply with a PIN code for legacy pairing.
    /// OGF=0x01, OCF=0x0027 (PIN_Code_Request_Reply).
    /// Params: bdaddr(6) | pin_len(1) | pin(16).
    pub fn pin_code_request_reply(&mut self, bdaddr: &BdAddr, pin: &[u8]) -> bool {
        let mut params = Vec::with_capacity(23);
        params.extend_from_slice(&bdaddr.0);
        let pin_len = pin.len().min(16) as u8;
        params.push(pin_len);
        let mut pin_bytes = [0u8; 16];
        pin_bytes[..pin.len().min(16)].copy_from_slice(&pin[..pin.len().min(16)]);
        params.extend_from_slice(&pin_bytes);
        self.send_cmd(0x01, 0x0027, &params)
    }

    /// Reject a PIN code request (negative reply).
    /// OGF=0x01, OCF=0x0028 (PIN_Code_Request_Negative_Reply).
    pub fn pin_code_request_negative_reply(&mut self, bdaddr: &BdAddr) -> bool {
        self.send_cmd(0x01, 0x0028, &bdaddr.0)
    }

    /// Accept a user confirmation request (numeric comparison).
    /// OGF=0x01, OCF=0x002C (User_Confirmation_Request_Reply).
    pub fn user_confirm_request_reply(&mut self, bdaddr: &BdAddr) -> bool {
        self.send_cmd(0x01, 0x002C, &bdaddr.0)
    }

    /// Reject a user confirmation request.
    /// OGF=0x01, OCF=0x002D (User_Confirmation_Request_Negative_Reply).
    pub fn user_confirm_negative_reply(&mut self, bdaddr: &BdAddr) -> bool {
        self.send_cmd(0x01, 0x002D, &bdaddr.0)
    }

    /// Delete a stored link key on the controller (unpair).
    /// OGF=0x03, OCF=0x004B (Delete_Stored_Link_Key).
    /// Params: bdaddr(6) | delete_all(1).
    /// If delete_all=1, bdaddr is ignored and all keys are deleted.
    pub fn delete_stored_link_key(&mut self, bdaddr: &BdAddr, delete_all: bool) -> bool {
        let mut params = Vec::with_capacity(7);
        params.extend_from_slice(&bdaddr.0);
        params.push(if delete_all { 0x01 } else { 0x00 });
        self.send_cmd(0x03, 0x004B, &params)
    }

    /// Write a link key to the controller (store paired key).
    /// OGF=0x03, OCF=0x004A (Write_Stored_Link_Key).
    /// Params: num_keys(1) | bdaddr(6) | key_type(1) | key(16).
    pub fn write_stored_link_key(&mut self, bdaddr: &BdAddr, key: &[u8; 16], key_type: u8) -> bool {
        let mut params = Vec::with_capacity(24);
        params.push(0x01); // Number of keys to write
        params.extend_from_slice(&bdaddr.0);
        params.push(key_type & 0x0F); // Link Key Type
        params.extend_from_slice(key);
        self.send_cmd(0x03, 0x004A, &params)
    }

    /// Read the local stored link key list size.
    /// OGF=0x03, OCF=0x004C (Read_Stored_Link_Key).
    /// Params: bdaddr(6) | read_all(1).
    pub fn read_stored_link_key(&mut self, bdaddr: &BdAddr) -> bool {
        let mut params = Vec::with_capacity(7);
        params.extend_from_slice(&bdaddr.0);
        params.push(0x00); // read only matching bdaddr
        self.send_cmd(0x03, 0x004C, &params)
    }

    /// Set default IO Capabilities for SSP (set-and-forget).
    /// OGF=0x03, OCF=0x0068 (Write_Default_IO_Capabilities).
    /// Unlike IO_Capability_Request_Reply (which is sent in response to
    /// event 0x31), this command tells the controller what IO capabilities
    /// to assume when a pairing request arrives, without waiting for the event.
    /// Params: io_capability(1) | oob_data(1) | auth_requirements(1).
    pub fn write_default_io_capabilities(&mut self, io_capability: u8, oob_data: bool, auth_requirements: u8) -> bool {
        let params = [
            io_capability & 0x03,
            if oob_data { 0x01 } else { 0x00 },
            auth_requirements & 0x07,
        ];
        self.send_cmd(0x03, 0x0068, &params)
    }

    /// Start LE encryption (LE pairing).
    /// OGF=0x08, OCF=0x0019 (LE_Start_Encryption).
    /// Params: handle(2) | rand(8) | ediv(2) | ltk(16).
    pub fn le_start_encryption(&mut self, handle: u16, rand: &[u8; 8], ediv: u16, ltk: &[u8; 16]) -> bool {
        let mut params = Vec::with_capacity(28);
        params.extend_from_slice(&handle.to_le_bytes());
        params.extend_from_slice(rand);
        params.extend_from_slice(&ediv.to_le_bytes());
        params.extend_from_slice(ltk);
        self.send_cmd(0x08, 0x0019, &params)
    }

    // ── Utility ───────────────────────────────────────────────────────

    /// Get the number of pending (not yet completed) HCI commands.
    pub fn pending_command_count(&self) -> u32 {
        self.pending_cmds
    }

    /// Check if the controller is in a state where commands can be sent.
    pub fn can_send_commands(&self) -> bool {
        self.state == HciDevState::Up && self.fd.is_some()
    }
}

impl Drop for HciManager {
    fn drop(&mut self) {
        self.close();
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hci_manager_new() {
        let mgr = HciManager::new();
        assert_eq!(mgr.state, HciDevState::Down);
        assert!(!mgr.is_open());
        assert_eq!(mgr.pending_command_count(), 0);
    }

    #[test]
    fn test_hci_manager_open_close() {
        let mut mgr = HciManager::new();
        assert!(mgr.open());
        assert!(mgr.is_open());
        mgr.close();
        assert!(!mgr.is_open());
        assert_eq!(mgr.state, HciDevState::Down);
    }

    #[test]
    fn test_send_cmd_reset() {
        let mut mgr = HciManager::new();
        mgr.open();

        // Send reset command (OGF=0x03, OCF=0x0003)
        assert!(mgr.send_cmd(0x03, 0x0003, &[]));
    }

    #[test]
    fn test_send_cmd_with_params() {
        let mut mgr = HciManager::new();
        mgr.open();

        // Set event mask with all events enabled
        let all_events = [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x3F];
        assert!(mgr.send_cmd(0x03, 0x0001, &all_events));
    }

    #[test]
    fn test_init_controller() {
        let mut mgr = HciManager::new();
        mgr.open();

        assert!(mgr.init_controller());
        assert_eq!(mgr.state, HciDevState::Up);
        assert!(mgr.can_send_commands());
    }

    #[test]
    fn test_init_fails_without_open() {
        let mut mgr = HciManager::new();
        assert!(!mgr.init_controller());
        assert_eq!(mgr.state, HciDevState::Down);
    }

    #[test]
    fn test_event_handler_registration() {
        let mut mgr = HciManager::new();
        mgr.on_event(0x0E, |code, _params| {
            assert_eq!(code, 0x0E);
        });
        mgr.on_event(0x00, |_code, _params| {
            // Wildcard handler
        });
        mgr.on_acl(|_handle, _pb, _bc, _data| {
            // Handler is registered — no state capture needed
        });
        mgr.on_sco(|_handle, _data| {});
    }

    #[test]
    fn test_scan_commands() {
        let mut mgr = HciManager::new();
        mgr.open();

        assert!(mgr.set_scan_mode(3)); // Page + Inquiry scan
        assert!(mgr.start_inquiry([0x33, 0x8B, 0x9E], 10, 0));
        assert!(mgr.stop_inquiry());
    }

    #[test]
    fn test_connection_commands() {
        let mut mgr = HciManager::new();
        mgr.open();
        mgr.init_controller();

        let addr = BdAddr::parse("AA:BB:CC:DD:EE:FF").unwrap();
        assert!(mgr.create_connection(&addr, 0xCC18));
        assert!(mgr.disconnect(0x0042, 0x13)); // Remote User Terminated
    }

    #[test]
    fn test_le_commands() {
        let mut mgr = HciManager::new();
        mgr.open();
        mgr.init_controller();

        let addr = BdAddr::parse("11:22:33:44:55:66").unwrap();
        assert!(mgr.le_create_connection(&addr, 0));
        assert!(mgr.le_set_adv_enable(true));
        assert!(mgr.le_set_adv_enable(false));
    }

    #[test]
    fn test_dev_info() {
        let mut mgr = HciManager::new();
        mgr.open();
        mgr.init_controller();

        assert_eq!(mgr.dev_info.hci_version, 12);
        assert!(mgr.dev_info.acl_mtu > 0);
        assert!(mgr.dev_info.bdaddr != BdAddr::null());
    }

    #[test]
    fn test_poll_without_init() {
        let mut mgr = HciManager::new();
        // Poll before init should return 0
        assert_eq!(mgr.poll(), 0);

        mgr.open();
        // Poll before Up state should return 0
        assert_eq!(mgr.poll(), 0);
    }

    #[test]
    fn test_reset_controller() {
        let mut mgr = HciManager::new();
        mgr.open();
        assert!(mgr.reset_controller());
    }

    #[test]
    fn test_double_open() {
        let mut mgr = HciManager::new();
        assert!(mgr.open());
        assert!(mgr.open()); // Should succeed (already open)
    }
}
