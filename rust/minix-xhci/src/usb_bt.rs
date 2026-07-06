//! # Bluetooth HCI USB Class Driver
//!
//! Implements the Bluetooth HCI USB transport as a USB class driver within
//! the xHCI framework. Detects BT adapters (class 0xE0, subclass 0x01),
//! configures bulk endpoints, and provides HCI command/event/ACL transport.
//!
//! ## Endpoint Map (USB Bluetooth Adapter)
//!
//! ```text
//! EP0   — Control (standard USB — descriptor reading, SET_FEATURE)
//! Bulk OUT — HCI commands + ACL data (host→controller)
//! Bulk IN  — HCI events + ACL data (controller→host)
//! Interrupt IN — HCI events (alt, some adapters only)
//! Isoc OUT/IN — SCO/eSCO audio (optional, BT 2.0+)
//! ```
//!
//! ## HCI Transport Protocol
//!
//! HCI packets are wrapped in USB frames with a 1-byte type indicator:
//!
//! | Type | Packet | EP Direction |
//! |------|--------|-------------|
//! | 0x01 | HCI Command | Bulk OUT |
//! | 0x02 | ACL Data | Bulk IN/OUT |
//! | 0x03 | SCO Data | Isoc IN/OUT |
//! | 0x04 | HCI Event | Bulk IN |
//! | 0x05 | ISO Data | Bulk/Isoc (BT 5.2+) |
//!
//! ## References
//!
//! - Bluetooth Core Specification v5.4, Vol 2, Part E (HCI)
//! - USB-IF: Wireless Controller Class (0xE0)
//! - Linux drivers/bluetooth/btusb.c

use crate::ffi;
use crate::registers::{self, usb_class};
use crate::ring::RingMem;
use crate::usb_device::{self, UsbDeviceInfo, ProbeResult, UsbClassDriver};
use crate::xhci::XhciController;

// HCI protocol types from the minix-bt-hci crate
use minix_bt_hci::hci;

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of BT adapters we can track.
pub const MAX_BT_DEVICES: usize = 4;

/// DMA buffer size for HCI transfers (ACL max = 65535 + header).
pub const HCI_DMA_BUF_SIZE: usize = 65536;

/// Number of DMA buffers per endpoint (round-robin).
pub const HCI_NUM_DMA_BUFS: usize = 4;

// ============================================================================
// BT Device Endpoint Configuration
// ============================================================================

/// Endpoint description found in the config descriptor.
#[derive(Clone, Copy, Debug)]
pub struct BtEndpointInfo {
    pub address: u8,
    pub transfer_type: u8, // 2=Bulk, 3=Interrupt, 1=Isochronous
    pub max_packet_size: u16,
}

/// Parsed Bluetooth HCI interface from a USB config descriptor.
#[derive(Clone, Copy, Debug)]
pub struct BtInterfaceInfo {
    pub interface_num: u8,
    pub bulk_out_ep: Option<BtEndpointInfo>,
    pub bulk_in_ep: Option<BtEndpointInfo>,
    pub intr_in_ep: Option<BtEndpointInfo>,
    pub isoc_out_ep: Option<BtEndpointInfo>,
    pub isoc_in_ep: Option<BtEndpointInfo>,
}

/// Parse config descriptor and extract BT interface endpoints.
pub fn find_bt_endpoints(config_data: &[u8]) -> Option<BtInterfaceInfo> {
    let mut result = BtInterfaceInfo {
        interface_num: 0,
        bulk_out_ep: None,
        bulk_in_ep: None,
        intr_in_ep: None,
        isoc_out_ep: None,
        isoc_in_ep: None,
    };
    let mut in_bt_interface = false;
    let mut i = 0;

    while i + 1 < config_data.len() {
        let len = config_data[i] as usize;
        let desc_type = config_data[i + 1];
        if len < 2 { break; }

        match desc_type {
            registers::usb_descriptor::INTERFACE => {
                if i + 9 > config_data.len() { break; }
                if let Some(iface) = registers::InterfaceDescriptor::parse(&config_data[i..]) {
                    if iface.bInterfaceClass == usb_class::WIRELESS
                        && iface.bInterfaceSubClass == 0x01
                        && (iface.bInterfaceProtocol == 0x01 || iface.bInterfaceProtocol == 0x02)
                    {
                        in_bt_interface = true;
                        result.interface_num = iface.bInterfaceNumber;
                    } else {
                        in_bt_interface = false;
                    }
                }
            }
            registers::usb_descriptor::ENDPOINT => {
                if in_bt_interface && i + 7 <= config_data.len() {
                    if let Some(ep) = registers::EndpointDescriptor::parse(&config_data[i..]) {
                        let ep_info = BtEndpointInfo {
                            address: ep.bEndpointAddress,
                            transfer_type: ep.bmAttributes & 0x03,
                            max_packet_size: u16::from_le_bytes(ep.wMaxPacketSize) & 0x07FF,
                        };
                        let dir_in = (ep.bEndpointAddress & 0x80) != 0;
                        let xfer_type = ep.bmAttributes & 0x03;

                        match (xfer_type, dir_in) {
                            (2, false) => result.bulk_out_ep = Some(ep_info), // Bulk OUT
                            (2, true)  => result.bulk_in_ep = Some(ep_info),  // Bulk IN
                            (3, true)  => result.intr_in_ep = Some(ep_info),  // Interrupt IN
                            (1, false) => result.isoc_out_ep = Some(ep_info), // Isoc OUT
                            (1, true)  => result.isoc_in_ep = Some(ep_info),  // Isoc IN
                            _ => {} // Other (control endpoint, etc.)
                        }
                    }
                }
            }
            _ => {}
        }

        if len == 0 { break; }
        i += len;
    }

    // Must have at least bulk OUT and bulk IN for HCI
    if result.bulk_out_ep.is_some() && result.bulk_in_ep.is_some() {
        Some(result)
    } else {
        None
    }
}

// ============================================================================
// HCI Transport State
// ============================================================================

/// HCI USB transport backed by the xHCI controller.
pub struct BtHciTransport {
    /// xHCI device slot ID.
    pub slot_id: u8,
    /// USB speed code.
    pub speed_code: u8,
    /// Endpoint info from config descriptor.
    pub ep_info: BtInterfaceInfo,
    /// Vendor ID.
    pub vendor_id: u16,
    /// Product ID.
    pub product_id: u16,

    // DMA buffers
    /// DMA buffer for HCI command transfer (out).
    pub cmd_dma: RingMem,
    /// DMA buffer for HCI event transfer (in).
    pub evt_dma: RingMem,
    /// DMA buffer for ACL OUT transfers.
    pub acl_out_dma: RingMem,
    /// DMA buffer for ACL IN transfers.
    pub acl_in_dma: RingMem,

    /// xHCI EP0 transfer ring setup is done.
    pub ep0_ready: bool,
    /// Bulk OUT endpoint configured.
    pub bulk_out_ready: bool,
    /// Bulk IN endpoint configured.
    pub bulk_in_ready: bool,

    /// HCI controller state.
    pub hci_state: hci::HciState,
    /// BD_ADDR (read during init).
    pub bd_addr: hci::BdAddr,
    /// Controller version info.
    pub hci_version: u8,
    pub hci_revision: u16,
    pub manufacturer: u16,
    /// LMP version (from Read Local Version).
    pub lmp_version: u8,
    /// LMP subversion.
    pub lmp_subversion: u16,
}

impl BtHciTransport {
    /// Size of the command DMA buffer.
    pub const CMD_DMA_SIZE: usize = 256 + 4; // HCI cmd max
    /// Size of the event DMA buffer.
    pub const EVT_DMA_SIZE: usize = 256 + 2; // HCI event max
    /// ACL data DMA buffer size.
    pub const ACL_DMA_SIZE: usize = 65536; // 64KB for max ACL

    /// Allocate DMA buffers and initialise the transport.
    pub fn new(slot_id: u8, speed_code: u8, ep_info: BtInterfaceInfo,
        vendor_id: u16, product_id: u16
    ) -> Option<Self> {
        let cmd_dma = RingMem::alloc((Self::CMD_DMA_SIZE + 63) / 64 * 64)?;
        let evt_dma = RingMem::alloc((Self::EVT_DMA_SIZE + 63) / 64 * 64)?;
        let acl_out_dma = RingMem::alloc((Self::ACL_DMA_SIZE + 63) / 64 * 64)?;
        let acl_in_dma = RingMem::alloc((Self::ACL_DMA_SIZE + 63) / 64 * 64)?;

        Some(Self {
            slot_id, speed_code, ep_info,
            vendor_id, product_id,
            cmd_dma, evt_dma, acl_out_dma, acl_in_dma,
            ep0_ready: false,
            bulk_out_ready: false,
            bulk_in_ready: false,
            hci_state: hci::HciState::Reset,
            bd_addr: hci::BdAddr([0u8; 6]),
            hci_version: 0, hci_revision: 0,
            manufacturer: 0, lmp_version: 0, lmp_subversion: 0,
        })
    }

    /// Configure HCI endpoints on the xHCI controller.
    /// Must be called after `setup_ep0_transfer_ring()` on the controller.
    pub fn configure_endpoints(&mut self, xhc: &mut XhciController) -> bool {
        // EP0 should already be set up
        self.ep0_ready = true;

        // Configure bulk OUT endpoint
        if let Some(ep) = &self.ep_info.bulk_out_ep {
            let ep_num = ep.address & 0x0F;
            let dci = XhciController::ep_num_to_dci(ep_num, false);
            let ep_type = 2u8; // Bulk OUT
            let cerr = 3u8;
            if !xhc.configure_endpoint(self.slot_id, dci, ep_type,
                ep.max_packet_size, cerr, ep.max_packet_size)
            {
                ffi::print(b"xHCI-BT: bulk OUT config failed\0");
                return false;
            }
            self.bulk_out_ready = true;
        } else {
            ffi::print(b"xHCI-BT: no bulk OUT endpoint\0");
            return false;
        }

        // Configure bulk IN endpoint
        if let Some(ep) = &self.ep_info.bulk_in_ep {
            let ep_num = ep.address & 0x0F;
            let dci = XhciController::ep_num_to_dci(ep_num, true);
            let ep_type = 6u8; // Bulk IN
            let cerr = 3u8;
            if !xhc.configure_endpoint(self.slot_id, dci, ep_type,
                ep.max_packet_size, cerr, ep.max_packet_size)
            {
                ffi::print(b"xHCI-BT: bulk IN config failed\0");
                return false;
            }
            self.bulk_in_ready = true;
        } else {
            ffi::print(b"xHCI-BT: no bulk IN endpoint\0");
            return false;
        }

        // Optionally configure interrupt IN endpoint
        if let Some(ep) = &self.ep_info.intr_in_ep {
            let ep_num = ep.address & 0x0F;
            let dci = XhciController::ep_num_to_dci(ep_num, true);
            let ep_type = 7u8; // Interrupt IN
            let cerr = 3u8;
            let _ = xhc.configure_endpoint(self.slot_id, dci, ep_type,
                ep.max_packet_size, cerr, ep.max_packet_size);
        }

        ffi::print(b"xHCI-BT: endpoints configured\0");
        true
    }

    /// Send an HCI command via the bulk OUT endpoint.
    /// `data` includes the HCI type byte (0x01) + opcode + params.
    pub fn send_command(&mut self, xhc: &mut XhciController, data: &[u8]) -> bool {
        if !self.bulk_out_ready || data.len() < 4 || data[0] != 0x01 {
            return false;
        }
        if data.len() > self.cmd_dma.size {
            return false;
        }

        // Copy data to DMA buffer
        unsafe {
            core::ptr::copy_nonoverlapping(
                data.as_ptr(), self.cmd_dma.virt as *mut u8, data.len()
            );
        }

        let ep_num = self.ep_info.bulk_out_ep.unwrap().address & 0x0F;
        xhc.queue_bulk_transfer(self.slot_id, ep_num, false,
            self.cmd_dma.phys, data.len() as u32, true)
            && xhc.poll_transfer_event(5_000_000)
    }

    /// Receive an HCI event from the bulk IN endpoint.
    /// Returns number of bytes received, or 0 on failure.
    pub fn recv_event(&mut self, xhc: &mut XhciController, out_buf: &mut [u8]) -> usize {
        if !self.bulk_in_ready {
            return 0;
        }

        let ep = match self.ep_info.bulk_in_ep {
            Some(e) => e,
            None => return 0,
        };
        let ep_num = ep.address & 0x0F;
        let max_recv = core::cmp::min(self.evt_dma.size, out_buf.len());

        // Submit bulk IN transfer
        if !xhc.queue_bulk_transfer(self.slot_id, ep_num, true,
            self.evt_dma.phys, max_recv as u32, true)
        {
            return 0;
        }

        // Wait for completion
        if !xhc.poll_transfer_event(30_000_000) {
            return 0;
        }

        // Copy from DMA buffer to output
        unsafe {
            core::ptr::copy_nonoverlapping(
                self.evt_dma.virt as *const u8, out_buf.as_mut_ptr(), max_recv
            );
        }

        max_recv
    }

    /// Send ACL data via bulk OUT endpoint.
    pub fn send_acl(&mut self, xhc: &mut XhciController, data: &[u8]) -> bool {
        if !self.bulk_out_ready || data.len() < 5 || data[0] != 0x02 {
            return false;
        }
        if data.len() > self.acl_out_dma.size {
            return false;
        }

        unsafe {
            core::ptr::copy_nonoverlapping(
                data.as_ptr(), self.acl_out_dma.virt as *mut u8, data.len()
            );
        }

        let ep_num = self.ep_info.bulk_out_ep.unwrap().address & 0x0F;
        xhc.queue_bulk_transfer(self.slot_id, ep_num, false,
            self.acl_out_dma.phys, data.len() as u32, true)
            && xhc.poll_transfer_event(30_000_000)
    }

    /// Receive ACL data from bulk IN endpoint.
    pub fn recv_acl(&mut self, xhc: &mut XhciController, out_buf: &mut [u8]) -> usize {
        if !self.bulk_in_ready {
            return 0;
        }

        let ep = match self.ep_info.bulk_in_ep {
            Some(e) => e,
            None => return 0,
        };
        let ep_num = ep.address & 0x0F;
        let max_recv = core::cmp::min(self.acl_in_dma.size, out_buf.len());

        if !xhc.queue_bulk_transfer(self.slot_id, ep_num, true,
            self.acl_in_dma.phys, max_recv as u32, true)
        {
            return 0;
        }

        if !xhc.poll_transfer_event(30_000_000) {
            return 0;
        }

        unsafe {
            core::ptr::copy_nonoverlapping(
                self.acl_in_dma.virt as *const u8, out_buf.as_mut_ptr(), max_recv
            );
        }

        max_recv
    }

    /// Perform HCI Reset command.
    pub fn hci_reset(&mut self, xhc: &mut XhciController) -> bool {
        let mut buf = [0u8; 8];
        let len = hci::build_hci_cmd(&mut buf, hci::ctrl_bb::RESET, &[]);
        if len == 0 { return false; }
        if !self.send_command(xhc, &buf[..len]) { return false; }

        let mut evt = [0u8; hci::HCI_MAX_EVT_SIZE];
        let recv = self.recv_event(xhc, &mut evt);
        if recv == 0 { return false; }
        hci::check_cmd_success(&evt[..recv], hci::ctrl_bb::RESET)
    }

    /// Read controller version information.
    pub fn read_local_version(&mut self, xhc: &mut XhciController) -> bool {
        let mut buf = [0u8; 8];
        let len = hci::build_hci_cmd(&mut buf, hci::info::READ_LOCAL_VERSION, &[]);
        if len == 0 { return false; }
        if !self.send_command(xhc, &buf[..len]) { return false; }

        let mut evt = [0u8; hci::HCI_MAX_EVT_SIZE];
        let recv = self.recv_event(xhc, &mut evt);
        if recv == 0 { return false; }

        if let Some((opcode, status, poff)) = hci::parse_cmd_complete(&evt[..recv]) {
            if opcode != hci::info::READ_LOCAL_VERSION || status != 0 { return false; }
            if poff + 8 > recv { return false; }
            self.hci_version = evt[poff];
            self.hci_revision = (evt[poff + 1] as u16) | ((evt[poff + 2] as u16) << 8);
            self.lmp_version = evt[poff + 3];
            self.manufacturer = (evt[poff + 4] as u16) | ((evt[poff + 5] as u16) << 8);
            self.lmp_subversion = (evt[poff + 6] as u16) | ((evt[poff + 7] as u16) << 8);
            return true;
        }
        false
    }

    /// Read the controller's BD_ADDR.
    pub fn read_bd_addr(&mut self, xhc: &mut XhciController) -> bool {
        let mut buf = [0u8; 8];
        let len = hci::build_hci_cmd(&mut buf, hci::info::READ_BD_ADDR, &[]);
        if len == 0 { return false; }
        if !self.send_command(xhc, &buf[..len]) { return false; }

        let mut evt = [0u8; hci::HCI_MAX_EVT_SIZE];
        let recv = self.recv_event(xhc, &mut evt);
        if recv == 0 { return false; }

        if let Some((opcode, status, poff)) = hci::parse_cmd_complete(&evt[..recv]) {
            if opcode != hci::info::READ_BD_ADDR || status != 0 { return false; }
            if poff + 6 > recv { return false; }
            let mut addr = [0u8; 6];
            addr.copy_from_slice(&evt[poff..poff + 6]);
            self.bd_addr = hci::BdAddr(addr);
            return true;
        }
        false
    }

    /// Full HCI controller initialisation sequence.
    pub fn init_sequence(&mut self, xhc: &mut XhciController) -> bool {
        self.hci_state = hci::HciState::Reset;

        // Stage 1: Reset
        if !self.hci_reset(xhc) {
            self.hci_state = hci::HciState::Error;
            return false;
        }

        // Stage 2: Read version
        if !self.read_local_version(xhc) {
            self.hci_state = hci::HciState::Error;
            return false;
        }

        // Stage 3: Read BD_ADDR
        if !self.read_bd_addr(xhc) {
            self.hci_state = hci::HciState::Error;
            return false;
        }

        // Stage 4: Set event mask (enable all BR/EDR events)
        {
            let evt_mask = [0xFFu8; 8];
            let mut cmd_buf = [0u8; 16];
            let len = hci::build_hci_cmd(&mut cmd_buf, hci::ctrl_bb::SET_EVENT_MASK, &evt_mask);
            if len > 0 {
                self.send_command(xhc, &cmd_buf[..len]);
                let mut evt = [0u8; hci::HCI_MAX_EVT_SIZE];
                let _ = self.recv_event(xhc, &mut evt);
            }
        }

        // Stage 5: Mark ready
        self.hci_state = hci::HciState::Up;

        let mut addr_buf = [0u8; 18];
        self.bd_addr.format(&mut addr_buf);
        ffi::print(b"xHCI-BT: adapter ready, BD_ADDR: \0");

        true
    }

    // ========================================================================
    // Power Management: USB Suspend/Resume
    // ========================================================================

    /// Suspend the BT controller — put the USB port into U3 (selective suspend).
    /// Before suspending, send HCI Reset to put the controller into a known state.
    /// Returns true if suspend was successful.
    pub fn suspend(&mut self, xhc: &mut XhciController) -> bool {
        if self.hci_state == hci::HciState::Down || self.hci_state == hci::HciState::Reset {
            return true; // Already suspended or reset
        }

        // Send HCI Reset to quiesce the controller
        let _ = self.hci_reset(xhc);
        self.hci_state = hci::HciState::Down;

        // Put the USB port into U3 (selective suspend)
        let port = xhc.slot_port(self.slot_id);
        if port > 0 {
            xhc.port_suspend(port);
            if xhc.verbose >= 1 {
                ffi::print(b"xHCI-BT: port suspended (U3)\0");
            }
        }

        true
    }

    /// Resume the BT controller — exit U3, reinitialise HCI.
    /// Returns true if resume + HCI reinit succeeded.
    pub fn resume(&mut self, xhc: &mut XhciController) -> bool {
        let port = xhc.slot_port(self.slot_id);
        if port > 0 {
            // Exit U3 → U0
            if !xhc.port_resume(port) {
                if xhc.verbose >= 1 {
                    ffi::print(b"xHCI-BT: port resume failed\0");
                }
                return false;
            }
            ffi::udelay(50_000); // Wait for the port to stabilise after resume
        }

        // Re-initialise the HCI controller (Reset + version + BD_ADDR + event mask)
        if !self.init_sequence(xhc) {
            if xhc.verbose >= 1 {
                ffi::print(b"xHCI-BT: HCI reinit after resume failed\0");
            }
            self.hci_state = hci::HciState::Error;
            return false;
        }

        if xhc.verbose >= 1 {
            ffi::print(b"xHCI-BT: resumed and reinitialised\0");
        }
        true
    }
}

// ============================================================================
// Bluetooth Class Driver (implements UsbClassDriver trait)
// ============================================================================

/// State of a detected Bluetooth adapter.
pub struct BtAdapterState {
    /// Whether this slot is in use.
    pub used: bool,
    /// The HCI transport instance.
    pub transport: Option<BtHciTransport>,
    /// Whether init has been attempted/completed.
    pub initialized: bool,
    // ── Power Management Fields ────────────────────────────────────────────
    /// Whether the USB port is suspended (U3).
    pub suspended: bool,
    /// Idle counter: incremented each tick, reset on BT activity.
    /// When this exceeds a threshold, the adapter auto-suspends.
    pub idle_counter: u32,
    /// Auto-suspend idle timeout in ticks (each tick ≈ alarm interval ≈ 10ms).
    /// 500 ticks = ~5 seconds of inactivity before auto-suspend.
    pub suspend_idle_threshold: u32,
}

impl BtAdapterState {
    pub fn new() -> Self {
        Self {
            used: false, transport: None, initialized: false,
            suspended: false, idle_counter: 0,
            suspend_idle_threshold: 500, // ~5 seconds at 10ms tick
        }
    }

    /// Reset the idle counter (called on any BT activity).
    pub fn touch(&mut self) {
        self.idle_counter = 0;
    }
}

/// Bluetooth class driver — manages detection and initialisation of BT adapters.
pub struct BtDriver {
    /// Detected BT adapters.
    pub adapters: [BtAdapterState; MAX_BT_DEVICES],
    /// Verbosity level.
    pub verbose: u8,
}

impl BtDriver {
    pub fn new(verbose: u8) -> Self {
        Self {
            adapters: core::array::from_fn(|_| BtAdapterState::new()),
            verbose,
        }
    }

    /// Find a free adapter slot.
    pub fn alloc_slot(&mut self) -> Option<usize> {
        for i in 0..MAX_BT_DEVICES {
            if !self.adapters[i].used {
                self.adapters[i].used = true;
                return Some(i);
            }
        }
        None
    }

    /// Free an adapter slot.
    pub fn free_slot(&mut self, idx: usize) {
        if idx < MAX_BT_DEVICES {
            self.adapters[idx] = BtAdapterState::new();
        }
    }

    /// Find adapter by slot_id.
    pub fn find_by_slot(&self, slot_id: u8) -> Option<usize> {
        for i in 0..MAX_BT_DEVICES {
            if self.adapters[i].used {
                if let Some(ref t) = self.adapters[i].transport {
                    if t.slot_id == slot_id { return Some(i); }
                }
            }
        }
        None
    }

    /// Create a static version for use as a global.
    pub const fn new_static() -> Self {
        const STATE: BtAdapterState = BtAdapterState {
            used: false, transport: None, initialized: false,
            suspended: false, idle_counter: 0,
            suspend_idle_threshold: 500,
        };
        Self { adapters: [STATE; MAX_BT_DEVICES], verbose: 0 }
    }
}

impl UsbClassDriver for BtDriver {
    fn class_code(&self) -> u8 { usb_class::WIRELESS }

    fn subclass_code(&self) -> u8 { 0x01 }

    fn protocol_code(&self) -> u8 { 0x01 }

    fn name(&self) -> &'static [u8] { b"xHCI-BT\0" }

    fn probe(&mut self, xhc: &mut XhciController, slot_id: u8, dev_info: &UsbDeviceInfo) -> ProbeResult {
        if self.verbose >= 1 {
            ffi::print(b"xHCI-BT: probing Bluetooth adapter\0");
        }

        // Set up EP0 transfer ring for control transfers
        if !xhc.setup_ep0_transfer_ring(slot_id) {
            ffi::print(b"xHCI-BT: EP0 setup failed\0");
            return ProbeResult::Failed;
        }

        // Allocate DMA buffer for descriptor reading
        let mut cfg_buf = match RingMem::alloc(512) {
            Some(b) => b,
            None => return ProbeResult::Failed,
        };

        // Read config descriptor to find BT endpoints
        if !xhc.get_config_descriptor(slot_id, 0, cfg_buf.virt, cfg_buf.phys, 512) {
            cfg_buf.free();
            return ProbeResult::Failed;
        }

        let config_data = unsafe {
            core::slice::from_raw_parts(cfg_buf.virt as *const u8, 512)
        };

        let ep_info = match find_bt_endpoints(config_data) {
            Some(info) => info,
            None => {
                cfg_buf.free();
                if self.verbose >= 1 {
                    ffi::print(b"xHCI-BT: no BT endpoints found\0");
                }
                return ProbeResult::NotMine;
            }
        };

        // Read device descriptor for VID/PID
        let dev_desc = match xhc.get_device_descriptor(slot_id, cfg_buf.virt, cfg_buf.phys) {
            Some(d) => d,
            None => {
                cfg_buf.free();
                return ProbeResult::Failed;
            }
        };

        cfg_buf.free();

        // Allocate transport with DMA buffers
        let mut transport = match BtHciTransport::new(
            slot_id, dev_info.speed_code, ep_info,
            dev_desc.vendor_id(), dev_desc.product_id()
        ) {
            Some(t) => t,
            None => return ProbeResult::Failed,
        };

        // Configure HCI endpoints on the xHCI controller
        if !transport.configure_endpoints(xhc) {
            return ProbeResult::Failed;
        }

        // Run HCI init sequence
        if !transport.init_sequence(xhc) {
            if self.verbose >= 1 {
                ffi::print(b"xHCI-BT: init sequence failed\0");
            }
            // Keep transport for potential recovery
        }

        // Store the transport
        let idx = match self.alloc_slot() {
            Some(i) => i,
            None => return ProbeResult::Failed,
        };
        self.adapters[idx].transport = Some(transport);
        self.adapters[idx].initialized = true;

        if self.verbose >= 1 {
            ffi::print(b"xHCI-BT: Bluetooth adapter ready\0");
        }
        ProbeResult::Claimed
    }

    fn disconnect(&mut self, xhc: &mut XhciController, slot_id: u8) {
        if let Some(idx) = self.find_by_slot(slot_id) {
            if self.verbose >= 1 {
                ffi::print(b"xHCI-BT: adapter disconnected\0");
            }
            // Free xHCI slot resources and set to None for clean state
            let slot = &mut xhc.slots[slot_id as usize];
            if let Some(ctx) = &mut slot.ctx { ctx.free(); slot.ctx = None; }
            if let Some(ictx) = &mut slot.input_ctx { ictx.free(); slot.input_ctx = None; }
            for ring in slot.transfer_rings.iter_mut() {
                if let Some(r) = ring { r.free(); }
                *ring = None;
            }
            slot.assigned = false;
            slot.configured = false;
            self.free_slot(idx);
        }
    }
}

// ============================================================================
// Public Suspend/Resume Helpers
// ============================================================================

/// Suspend a BT adapter by index — used from alarm callback and ioctl.
pub fn suspend_adapter(xhc: &mut XhciController, idx: usize) -> bool {
    let bt_drv = unsafe { &mut *core::ptr::addr_of_mut!(crate::BT_DRIVER) };
    if idx >= bt_drv.adapters.len() || !bt_drv.adapters[idx].used { return false; }
    let adapter = &mut bt_drv.adapters[idx];
    if adapter.suspended { return true; }
    let transport = match &mut adapter.transport {
        Some(t) => t,
        None => return false,
    };
    if transport.suspend(xhc) {
        adapter.suspended = true;
        adapter.idle_counter = 0;
        if bt_drv.verbose >= 1 {
            ffi::print(b"xHCI-BT: adapter suspended\0");
        }
        true
    } else {
        false
    }
}

/// Resume a BT adapter by index — used from alarm callback and ioctl.
pub fn resume_adapter(xhc: &mut XhciController, idx: usize) -> bool {
    let bt_drv = unsafe { &mut *core::ptr::addr_of_mut!(crate::BT_DRIVER) };
    if idx >= bt_drv.adapters.len() || !bt_drv.adapters[idx].used { return false; }
    let adapter = &mut bt_drv.adapters[idx];
    if !adapter.suspended { return true; }
    adapter.suspended = false;
    let transport = match &mut adapter.transport {
        Some(t) => t,
        None => return false,
    };
    if transport.resume(xhc) {
        adapter.idle_counter = 0;
        if bt_drv.verbose >= 1 {
            ffi::print(b"xHCI-BT: adapter resumed\0");
        }
        true
    } else {
        adapter.suspended = true; // Resume failed — keep suspended
        false
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_bt_endpoints_no_config() {
        let config = [0u8; 10];
        assert!(find_bt_endpoints(&config).is_none());
    }

    #[test]
    fn test_bt_interface_info() {
        // Mock a minimal config with BT interface
        // Mock config descriptor with BT interface (class 0xE0, subclass 0x01)
        let config: [u8; 39] = [
            9, 2, 39, 0, 1, 0, 0, 0, 0,  // Config descriptor (9 bytes, total=39)
            9, 4, 0, 0, 3, 0xE0, 0x01, 0x01, 0,  // Interface: 0xE0/0x01/0x01
            7, 5, 0x02, 0x02, 0x40, 0, 0,         // Bulk OUT EP (addr=2, bulk, 64 bytes)
            7, 5, 0x82, 0x02, 0x40, 0, 0,         // Bulk IN EP (addr=0x82, bulk, 64 bytes)
            7, 5, 0x83, 0x03, 0x40, 0, 0,         // Interrupt IN EP (addr=0x83, intr, 64 bytes)
        ];

        let result = find_bt_endpoints(&config);
        assert!(result.is_some());
        let info = result.unwrap();
        assert!(info.bulk_out_ep.is_some());
        assert!(info.bulk_in_ep.is_some());
        assert!(info.intr_in_ep.is_some());
        assert_eq!(info.bulk_out_ep.unwrap().address, 0x02);
        assert_eq!(info.bulk_in_ep.unwrap().address, 0x82);
    }
}
