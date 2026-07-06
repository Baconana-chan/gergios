//! # USB HCI Transport Layer
//!
//! Implements the Bluetooth HCI transport over USB using bulk endpoints.
//! HCI USB uses:
//!   - USB class 0xE0 (Wireless Controller), subclass 0x01, protocol 0x01
//!   - Bulk OUT endpoint (0x02) for HCI commands + ACL OUT + SCO OUT
//!   - Bulk IN endpoint (0x81) for HCI events + ACL IN + SCO IN
//!   - Interrupt IN endpoint (0x83) as alternate for HCI events
//!   - Isochronous IN/OUT for SCO/eSCO (BT 2.0+) and ISO data (BT 5.2+)
//!
//! ## Endpoint assignment (USB BT adapter)
//!
//! ```text
//! EP0   — Control (standard USB)
//! EP1   — Bulk OUT: HCI commands + ACL data (host→controller)
//! EP2   — Bulk IN:  HCI events + ACL data (controller→host)
//! EP3   — Interrupt IN: HCI events (alt, for controllers without bulk event support)
//! EP4   — Isochronous OUT: SCO audio (host→controller, optional)
//! EP5   — Isochronous IN:  SCO audio (controller→host, optional)
//! ```

#![allow(dead_code)]

use crate::ffi;
use crate::hci;

/// USB BT device interface descriptor parsing.
pub mod usb_desc {
    // USB standard descriptor types
    pub const DT_DEVICE: u8          = 1;
    pub const DT_CONFIG: u8          = 2;
    pub const DT_STRING: u8          = 3;
    pub const DT_INTERFACE: u8       = 4;
    pub const DT_ENDPOINT: u8        = 5;

    // USB class codes
    pub const CLASS_WIRELESS: u8     = 0xE0;  // Wireless Controller
    pub const SUBCLASS_RF: u8        = 0x01;  // RF Controller
    pub const PROTOCOL_BLUETOOTH: u8 = 0x01;  // Bluetooth Primary
    pub const PROTOCOL_BT_ALT: u8    = 0x02;  // Bluetooth AMP (alternate MAC/PHY)

    // Endpoint attributes
    pub const EP_ATTR_CONTROL: u8    = 0x00;
    pub const EP_ATTR_ISOCHRONOUS: u8 = 0x01;
    pub const EP_ATTR_BULK: u8       = 0x02;
    pub const EP_ATTR_INTERRUPT: u8  = 0x03;

    // Endpoint direction
    pub const EP_DIR_OUT: u8         = 0x00;
    pub const EP_DIR_IN: u8          = 0x80;

    /// Parsed endpoint descriptor from USB configuration.
    #[derive(Clone, Copy, Debug)]
    pub struct EndpointDesc {
        pub address: u8,     // bEndpointAddress (bit 7 = direction)
        pub attr: u8,        // bmAttributes (transfer type)
        pub max_packet: u16, // wMaxPacketSize
        pub interval: u8,    // bInterval
    }

    /// Parsed interface descriptor for a BT HCI interface.
    #[derive(Clone, Copy, Debug)]
    pub struct BtInterfaceDesc {
        pub interface_num: u8,
        pub endpoints: [EndpointDesc; 4], // bulk OUT, bulk IN, intr IN, isoc OUT
        pub num_endpoints: u8,
    }
}

// ============================================================================
// HCI USB Transfer Buffer
// ============================================================================

/// DMA buffer for HCI USB transfers.
pub struct HciUsbBuf {
    /// Virtual address.
    pub virt: *mut u8,
    /// Physical address.
    pub phys: u64,
    /// Size in bytes.
    pub size: usize,
}

impl HciUsbBuf {
    /// Allocate a DMA-safe buffer.
    pub fn alloc(size: usize) -> Option<Self> {
        let aligned = (size + 63) / 64 * 64;
        let (virt, phys) = ffi::alloc_contig_ffi(aligned)?;
        Some(Self {
            virt: virt as *mut u8,
            phys,
            size: aligned,
        })
    }

    /// Free the DMA buffer.
    pub fn free(&mut self) {
        if !self.virt.is_null() {
            ffi::free_contig_ffi(self.virt as *mut core::ffi::c_void, self.size);
            self.virt = core::ptr::null_mut();
            self.phys = 0;
            self.size = 0;
        }
    }
}

// ============================================================================
// HCI USB Transport State
// ============================================================================

/// Number of DMA buffers for USB transfers.
const NUM_XFER_BUFS: usize = 4;

/// Per-endpoint transfer state.
struct EndpointState {
    /// USB endpoint address.
    address: u8,
    /// DMA buffers for this endpoint.
    bufs: [Option<HciUsbBuf>; NUM_XFER_BUFS],
    /// Current buffer index (round-robin).
    buf_idx: usize,
    /// Max packet size.
    max_packet: u16,
    /// Whether this endpoint is open/available.
    active: bool,
}

impl EndpointState {
    fn new() -> Self {
        const NONE: Option<HciUsbBuf> = None;
        Self {
            address: 0,
            bufs: [NONE; NUM_XFER_BUFS],
            buf_idx: 0,
            max_packet: 64,
            active: false,
        }
    }

    /// Allocate DMA buffers for this endpoint.
    fn alloc_bufs(&mut self, buf_size: usize) -> bool {
        for i in 0..NUM_XFER_BUFS {
            self.bufs[i] = HciUsbBuf::alloc(buf_size);
            if self.bufs[i].is_none() {
                // Free any allocated so far
                for j in 0..i {
                    if let Some(ref mut b) = self.bufs[j] {
                        b.free();
                    }
                    self.bufs[j] = None;
                }
                return false;
            }
        }
        true
    }

    /// Get next buffer for a transfer (round-robin).
    fn next_buf(&mut self) -> Option<&mut HciUsbBuf> {
        let idx = self.buf_idx % NUM_XFER_BUFS;
        self.buf_idx = (self.buf_idx + 1) % NUM_XFER_BUFS;
        self.bufs[idx].as_mut()
    }

    fn free_all(&mut self) {
        for i in 0..NUM_XFER_BUFS {
            if let Some(ref mut b) = self.bufs[i] {
                b.free();
            }
            self.bufs[i] = None;
        }
        self.active = false;
    }
}

/// HCI USB Transport — manages USB endpoints and raw data transfer.
pub struct HciUsbTransport {
    /// USB device vendor ID.
    pub vendor_id: u16,
    /// USB device product ID.
    pub product_id: u16,
    /// Vendor name string (from device descriptor).
    pub vendor_str: [u8; 16],
    /// Product name string.
    pub product_str: [u8; 32],

    /// Bulk OUT endpoint (HCI commands + ACL data to controller).
    bulk_out: EndpointState,
    /// Bulk IN endpoint (HCI events + ACL data from controller).
    bulk_in: EndpointState,
    /// Interrupt IN endpoint (alt for events).
    intr_in: EndpointState,
    /// Isochronous OUT endpoint (SCO audio to controller, optional).
    isoc_out: EndpointState,
    /// Isochronous IN endpoint (SCO audio from controller, optional).
    isoc_in: EndpointState,

    /// Current HCI state.
    pub state: hci::HciState,
    /// Local BD_ADDR (read from controller during init).
    pub bd_addr: hci::BdAddr,
    /// HCI version info (from Read Local Version).
    pub hci_version: u8,
    pub hci_revision: u16,
    pub lmp_version: u8,
    pub lmp_subversion: u16,
    /// Manufacturer ID.
    pub manufacturer: u16,
    /// Whether transport is initialised.
    pub ready: bool,
}

impl HciUsbTransport {
    /// Probe for a Bluetooth USB adapter.
    /// Walks the USB device tree looking for class 0xE0, subclass 0x01.
    pub fn probe() -> Result<Self, i32> {
        // On MINIX, this would walk USB devices via the xHCI driver.
        // For now, return a default-constructed transport (found) or Err.
        let mut t = Self::new();
        t.configure_endpoints(
            0x02,  // bulk OUT addr (EP 1 OUT)
            0x81,  // bulk IN addr  (EP 1 IN)
            0x83,  // intr IN addr  (EP 3 IN)
            None,  // isoc OUT
            None,  // isoc IN
            64,    // bulk max packet
            64,    // intr max packet
            48,    // isoc max packet
        );
        Ok(t)
    }

    /// Create a new HCI USB transport (not yet initialised).
    pub fn new() -> Self {
        Self {
            vendor_id: 0,
            product_id: 0,
            vendor_str: [0u8; 16],
            product_str: [0u8; 32],
            bulk_out: EndpointState::new(),
            bulk_in: EndpointState::new(),
            intr_in: EndpointState::new(),
            isoc_out: EndpointState::new(),
            isoc_in: EndpointState::new(),
            state: hci::HciState::Reset,
            bd_addr: hci::BdAddr([0u8; 6]),
            hci_version: 0,
            hci_revision: 0,
            lmp_version: 0,
            lmp_subversion: 0,
            manufacturer: 0,
            ready: false,
        }
    }

    /// Parse USB configuration descriptor to find BT interface and endpoints.
    /// This would be called during device enumeration by the USB class driver.
    /// For now, we use fixed endpoint assignment (common for BT adapters).
    pub fn configure_endpoints(&mut self,
        bulk_out_addr: u8, bulk_in_addr: u8,
        intr_in_addr: u8,
        isoc_out_addr: Option<u8>, isoc_in_addr: Option<u8>,
        bulk_mps: u16, intr_mps: u16, isoc_mps: u16
    ) -> bool {
        // Bulk OUT (for HCI commands + ACL OUT)
        self.bulk_out.address = bulk_out_addr;
        self.bulk_out.max_packet = bulk_mps;
        if !self.bulk_out.alloc_bufs(hci::HCI_MAX_ACL_SIZE) {
            return false;
        }
        self.bulk_out.active = true;

        // Bulk IN (for HCI events + ACL IN)
        self.bulk_in.address = bulk_in_addr;
        self.bulk_in.max_packet = bulk_mps;
        if !self.bulk_in.alloc_bufs(hci::HCI_MAX_ACL_SIZE) {
            return false;
        }
        self.bulk_in.active = true;

        // Interrupt IN (alt for HCI events)
        self.intr_in.address = intr_in_addr;
        self.intr_in.max_packet = intr_mps;
        if !self.intr_in.alloc_bufs(hci::HCI_MAX_EVT_SIZE) {
            return false;
        }
        self.intr_in.active = true;

        // Isochronous endpoints (optional — for SCO/eSCO)
        if let Some(addr) = isoc_out_addr {
            self.isoc_out.address = addr;
            self.isoc_out.max_packet = isoc_mps;
            let _ = self.isoc_out.alloc_bufs(hci::HCI_MAX_SCO_SIZE);
            self.isoc_out.active = true;
        }
        if let Some(addr) = isoc_in_addr {
            self.isoc_in.address = addr;
            self.isoc_in.max_packet = isoc_mps;
            let _ = self.isoc_in.alloc_bufs(hci::HCI_MAX_SCO_SIZE);
            self.isoc_in.active = true;
        }

        true
    }

    /// Send an HCI command via the bulk OUT endpoint.
    /// `data` must include the HCI type byte (0x01) + opcode + params.
    pub fn send_command(&mut self, data: &[u8]) -> bool {
        if !self.bulk_out.active { return false; }
        if data.len() < 4 { return false; }
        if data[0] != 0x01 { return false; }  // Must be HCI Command type

        let buf = match self.bulk_out.next_buf() {
            Some(b) => b,
            None => return false,
        };

        if data.len() > buf.size {
            return false;
        }

        // Copy command data to DMA buffer
        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), buf.virt, data.len());
        }

        // Submit USB bulk OUT transfer — stub: returns true for init flow.
        true
    }

    /// Send ACL data via the bulk OUT endpoint.
    pub fn send_acl(&mut self, data: &[u8]) -> bool {
        if !self.bulk_out.active { return false; }
        if data.len() < 5 { return false; }
        if data[0] != 0x02 { return false; }  // Must be HCI ACL type

        let buf = match self.bulk_out.next_buf() {
            Some(b) => b,
            None => return false,
        };

        if data.len() > buf.size { return false; }

        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), buf.virt, data.len());
        }

        true
    }

    /// Send SCO data via the isochronous OUT endpoint.
    pub fn send_sco(&mut self, data: &[u8]) -> bool {
        if !self.isoc_out.active { return false; }
        if data.len() < 4 { return false; }
        if data[0] != 0x03 { return false; }  // Must be HCI SCO type

        let buf = match self.isoc_out.next_buf() {
            Some(b) => b,
            None => return false,
        };

        if data.len() > buf.size { return false; }

        unsafe {
            core::ptr::copy_nonoverlapping(data.as_ptr(), buf.virt, data.len());
        }

        true
    }

    /// Receive an HCI event from the bulk IN endpoint.
    /// Returns the number of bytes received, or 0 on failure.
    pub fn recv_event(&mut self, out_buf: &mut [u8]) -> usize {
        if !self.bulk_in.active { return 0; }

        let buf = match self.bulk_in.next_buf() {
            Some(b) => b,
            None => return 0,
        };

        // Stub: platform_bulk_in returns 0 (no data available).
        // Real implementation would submit USB bulk IN transfer.
        0
    }

    /// Receive ACL data from the bulk IN endpoint.
    pub fn recv_acl(&mut self, out_buf: &mut [u8]) -> usize {
        if !self.bulk_in.active { return 0; }

        let buf = match self.bulk_in.next_buf() {
            Some(b) => b,
            None => return 0,
        };

        // Stub: platform_bulk_in returns 0 (no data available).
        0
    }

    /// Perform HCI Reset command.
    pub fn hci_reset(&mut self) -> bool {
        let mut buf = [0u8; 8];
        let len = hci::build_hci_cmd(&mut buf, hci::ctrl_bb::RESET, &[]);
        if len == 0 { return false; }

        if !self.send_command(&buf[..len]) { return false; }

        // Wait for Command Complete event
        let mut evt = [0u8; hci::HCI_MAX_EVT_SIZE];
        let recv = self.recv_event(&mut evt);
        if recv == 0 { return false; }

        hci::check_cmd_success(&evt[..recv], hci::ctrl_bb::RESET)
    }

    /// Read controller version information.
    pub fn read_local_version(&mut self) -> bool {
        let mut buf = [0u8; 8];
        let len = hci::build_hci_cmd(&mut buf, hci::info::READ_LOCAL_VERSION, &[]);
        if len == 0 { return false; }

        if !self.send_command(&buf[..len]) { return false; }

        let mut evt = [0u8; hci::HCI_MAX_EVT_SIZE];
        let recv = self.recv_event(&mut evt);
        if recv == 0 { return false; }

        // Parse Command Complete for Read Local Version
        // After the standard header: opcode(2) + status(1) + hci_ver(1) + hci_rev(2) + lmp_ver(1) + manufacturer(2) + lmp_sub(2)
        if let Some((opcode, status, poff)) = hci::parse_cmd_complete(&evt[..recv]) {
            if opcode != hci::info::READ_LOCAL_VERSION || status != 0 {
                return false;
            }
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
    pub fn read_bd_addr(&mut self) -> bool {
        let mut buf = [0u8; 8];
        let len = hci::build_hci_cmd(&mut buf, hci::info::READ_BD_ADDR, &[]);
        if len == 0 { return false; }

        if !self.send_command(&buf[..len]) { return false; }

        let mut evt = [0u8; hci::HCI_MAX_EVT_SIZE];
        let recv = self.recv_event(&mut evt);
        if recv == 0 { return false; }

        if let Some((opcode, status, poff)) = hci::parse_cmd_complete(&evt[..recv]) {
            if opcode != hci::info::READ_BD_ADDR || status != 0 {
                return false;
            }
            if poff + 6 > recv { return false; }

            let mut addr = [0u8; 6];
            addr.copy_from_slice(&evt[poff..poff + 6]);
            self.bd_addr = hci::BdAddr(addr);
            return true;
        }

        false
    }

    /// Read buffer size (ACL + SCO buffer sizes from controller).
    pub fn read_buffer_size(&mut self) -> Option<(u16, u8, u16)> {
        // Returns (acl_data_packet_len, sco_data_packet_len, total_num_acl_packets)
        let mut buf = [0u8; 8];
        let len = hci::build_hci_cmd(&mut buf, hci::info::READ_BUFFER_SIZE, &[]);
        if len == 0 { return None; }

        if !self.send_command(&buf[..len]) { return None; }

        let mut evt = [0u8; hci::HCI_MAX_EVT_SIZE];
        let recv = self.recv_event(&mut evt);
        if recv == 0 { return None; }

        if let Some((opcode, status, poff)) = hci::parse_cmd_complete(&evt[..recv]) {
            if opcode != hci::info::READ_BUFFER_SIZE || status != 0 {
                return None;
            }
            if poff + 7 > recv { return None; }

            let acl_len = (evt[poff] as u16) | ((evt[poff + 1] as u16) << 8);
            let sco_len = evt[poff + 2];
            let total_acl = (evt[poff + 3] as u16) | ((evt[poff + 4] as u16) << 8);
            // evt[poff+5..poff+7] = total_sco_packets (unused)

            Some((acl_len, sco_len, total_acl))
        } else {
            None
        }
    }

    /// Perform full HCI controller initialisation sequence.
    pub fn init_sequence(&mut self) -> bool {
        // Stage 1: Reset the controller
        self.state = hci::HciState::Reset;
        if !self.hci_reset() {
            self.state = hci::HciState::Error;
            return false;
        }

        // Stage 2: Read version information
        if !self.read_local_version() {
            self.state = hci::HciState::Error;
            return false;
        }

        // Stage 3: Read BD_ADDR
        if !self.read_bd_addr() {
            self.state = hci::HciState::Error;
            return false;
        }

        // Stage 4: Set event mask
        {
            // Enable all BR/EDR events
            let mut evt_mask = [0xFFu8; 8];
            let mut cmd_buf = [0u8; 16];
            let len = hci::build_hci_cmd(&mut cmd_buf,
                hci::ctrl_bb::SET_EVENT_MASK, &evt_mask);
            if len > 0 {
                self.send_command(&cmd_buf[..len]);
                let mut evt = [0u8; hci::HCI_MAX_EVT_SIZE];
                let _ = self.recv_event(&mut evt);
            }
        }

        // Stage 5: Enable LE support (BT 4.0+)
        {
            let params = [0x01u8];  // LE Supported = true
            let mut cmd_buf = [0u8; 16];
            let len = hci::build_hci_cmd(&mut cmd_buf,
                hci::ctrl_bb::WRITE_LE_HOST_SUPPORT, &params);
            if len > 0 {
                self.send_command(&cmd_buf[..len]);
                let mut evt = [0u8; hci::HCI_MAX_EVT_SIZE];
                let _ = self.recv_event(&mut evt);
            }
        }

        // Stage 6: Write Local Name (optional, for identification)
        {
            let name = b"GergiOS BT Adapter\0";
            let mut pad = [0u8; 248];
            let copy_len = core::cmp::min(name.len(), 247);
            pad[..copy_len].copy_from_slice(&name[..copy_len]);
            let mut cmd_buf = [0u8; 256];
            let len = hci::build_hci_cmd(&mut cmd_buf,
                hci::ctrl_bb::WRITE_LOCAL_NAME, &pad);
            if len > 0 {
                self.send_command(&cmd_buf[..len]);
                let mut evt = [0u8; hci::HCI_MAX_EVT_SIZE];
                let _ = self.recv_event(&mut evt);
            }
        }

        // Stage 7: Mark as ready
        self.state = hci::HciState::Up;
        self.ready = true;

        ffi::print(b"bt-hci: controller initialised successfully\0");

        // Print device info
        let mut addr_buf = [0u8; 18];
        self.bd_addr.format(&mut addr_buf);
        ffi::print(b"bt-hci: BD_ADDR: \0");
        ffi::print(&addr_buf[..17]);
        ffi::print(b"\0");

        true
    }

    /// Reset the HCI transport.
    pub fn reset(&mut self) {
        self.free_all();
        self.state = hci::HciState::Reset;
        self.ready = false;
    }

    /// Free all DMA buffers.
    pub fn free_all(&mut self) {
        self.bulk_out.free_all();
        self.bulk_in.free_all();
        self.intr_in.free_all();
        self.isoc_out.free_all();
        self.isoc_in.free_all();
    }
}

impl Drop for HciUsbTransport {
    fn drop(&mut self) {
        self.free_all();
    }
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(not(target_os = "minix"), ignore = "requires MINIX alloc_contig")]
    fn test_hci_usb_buf_alloc_free() {
        let mut buf = HciUsbBuf::alloc(1024).expect("should alloc");
        assert!(!buf.virt.is_null());
        assert!(buf.phys != 0);
        assert!(buf.size >= 1024);
        buf.free();
        assert!(buf.virt.is_null());
    }

    #[test]
    #[cfg_attr(not(target_os = "minix"), ignore = "requires MINIX alloc_contig")]
    fn test_endpoint_alloc_bufs() {
        let mut ep = EndpointState::new();
        assert!(ep.alloc_bufs(512));
        assert!(ep.bufs[0].is_some());
        assert!(ep.bufs[3].is_some());
        ep.free_all();
        assert!(ep.bufs[0].is_none());
    }

    #[test]
    fn test_transport_create() {
        let t = HciUsbTransport::new();
        assert!(!t.ready);
        assert_eq!(t.state, hci::HciState::Reset);
    }
}
