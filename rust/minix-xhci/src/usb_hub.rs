//! # USB Hub Support — Downstream Port Management
//!
//! Implements the USB Hub class driver for USB 2.0 and USB 3.0 hubs.
//! Handles:
//! - Hub descriptor parsing (USB 2.0 + USB 3.0)
//! - Port power, reset, enable/disable
//! - Port status change detection via interrupt endpoint
//! - Transaction Translator (TT) info for low/full speed behind hub
//! - Downstream device enumeration (recursive)

use crate::ffi;
use crate::registers::{
    self, HubDescriptor, SsHubDescriptor, usb_class, usb_descriptor,
    hub_req, hub_port_feature, hub_port_status, hub_port_change,
    build_setup_packet, InterfaceDescriptor, EndpointDescriptor,
    usb_xfer_type, usb_req, speed,
};
use crate::ring::{RingMem, RING_SIZE};
use crate::xhci::XhciController;
use crate::usb_device::{self, UsbDeviceInfo, UsbDeviceType, ProbeResult, UsbClassDriver};

/// Maximum number of ports on a single hub.
pub const MAX_HUB_PORTS: usize = 8;

/// Maximum number of hubs we can track.
pub const MAX_HUBS: usize = 4;

/// Tracked hub state.
pub struct UsbHubState {
    /// xHCI slot ID for this hub.
    pub slot_id: u8,
    /// Number of downstream ports.
    pub num_ports: u8,
    /// Cached hub descriptor.
    pub hub_desc: HubDescriptor,
    /// Whether this is a USB 3.0 (SuperSpeed) hub.
    pub is_ss: bool,
    /// Interrupt endpoint number (0 if not configured).
    pub intr_ep: u8,
    /// Max packet size of interrupt endpoint.
    pub intr_mps: u16,
    /// DMA buffer for interrupt data (port bitmap).
    pub intr_buf: Option<RingMem>,
    /// Port status change bitmap (cached from interrupt).
    pub port_change_bitmap: u16,
}

impl UsbHubState {
    fn new() -> Self {
        Self {
            slot_id: 0,
            num_ports: 0,
            hub_desc: unsafe { core::mem::zeroed() },
            is_ss: false,
            intr_ep: 0,
            intr_mps: 0,
            intr_buf: None,
            port_change_bitmap: 0,
        }
    }

    /// Create a static instance (for `pub static mut`).
    pub const fn new_static() -> Self {
        Self {
            slot_id: 0,
            num_ports: 0,
            hub_desc: unsafe { core::mem::zeroed() },
            is_ss: false,
            intr_ep: 0,
            intr_mps: 0,
            intr_buf: None,
            port_change_bitmap: 0,
        }
    }
}

// ============================================================================
// Hub Class Driver
// ============================================================================

/// The USB Hub class driver instance.
pub struct UsbHubDriver {
    /// Tracked hub states.
    pub hubs: [UsbHubState; MAX_HUBS],
    /// Number of active hubs.
    pub num_hubs: usize,
    /// Verbose logging.
    pub verbose: u8,
}

impl UsbHubDriver {
    pub fn new(verbose: u8) -> Self {
        Self {
            hubs: core::array::from_fn(|_| UsbHubState::new()),
            num_hubs: 0,
            verbose,
        }
    }

    /// Create a static instance (for `pub static mut`).
    pub const fn new_static() -> Self {
        const HUB: UsbHubState = UsbHubState::new_static();
        Self {
            hubs: [HUB, HUB, HUB, HUB],
            num_hubs: 0,
            verbose: 0,
        }
    }

    pub fn find_hub_by_slot(&self, slot_id: u8) -> Option<&UsbHubState> {
        self.hubs[..self.num_hubs].iter().find(|h| h.slot_id == slot_id)
    }

    pub fn find_hub_by_slot_mut(&mut self, slot_id: u8) -> Option<&mut UsbHubState> {
        self.hubs[..self.num_hubs].iter_mut().find(|h| h.slot_id == slot_id)
    }

    fn remove_hub(&mut self, slot_id: u8) {
        let mut found = None;
        for i in 0..self.num_hubs {
            if self.hubs[i].slot_id == slot_id {
                found = Some(i);
                break;
            }
        }
        if let Some(idx) = found {
            for j in idx..self.num_hubs - 1 {
                let tmp = core::mem::replace(&mut self.hubs[j + 1], UsbHubState::new());
                self.hubs[j] = tmp;
            }
            self.num_hubs -= 1;
        }
    }

    /// Get the hub descriptor from the device via control transfer.
    fn fetch_hub_descriptor(xhc: &mut XhciController, slot_id: u8,
        buf_virt: *mut u8, buf_phys: u64, buf_len: usize
    ) -> bool {
        let pkt = build_setup_packet(
            0xA0, // device-to-host, class, device
            hub_req::GET_HUB_DESCRIPTOR,
            0, 0, buf_len as u16,
        );
        xhc.control_transfer(slot_id, &pkt, true, buf_phys, buf_len as u32)
    }

    /// Set a port feature.
    fn set_port_feature(xhc: &mut XhciController, slot_id: u8,
        port: u8, feature: u16
    ) -> bool {
        let pkt = build_setup_packet(
            0x23, // host-to-device, class, other
            hub_req::SET_PORT_FEATURE,
            feature, port as u16, 0,
        );
        xhc.control_transfer(slot_id, &pkt, false, 0, 0)
    }

    /// Clear a port feature.
    fn clear_port_feature(xhc: &mut XhciController, slot_id: u8,
        port: u8, feature: u16
    ) -> bool {
        let pkt = build_setup_packet(
            0x23, // host-to-device, class, other
            hub_req::CLEAR_PORT_FEATURE,
            feature, port as u16, 0,
        );
        xhc.control_transfer(slot_id, &pkt, false, 0, 0)
    }

    /// Get port status (returns 32-bit status + change).
    fn get_port_status(xhc: &mut XhciController, slot_id: u8, port: u8,
        buf_virt: *mut u8, buf_phys: u64
    ) -> Option<(u16, u16)> {
        let pkt = build_setup_packet(
            0xA3, // device-to-host, class, other
            hub_req::GET_PORT_STATUS,
            0, port as u16, 4,
        );
        if !xhc.control_transfer(slot_id, &pkt, true, buf_phys, 4) {
            return None;
        }
        let data = unsafe { core::slice::from_raw_parts(buf_virt as *const u8, 4) };
        let status = u16::from_le_bytes([data[0], data[1]]);
        let change = u16::from_le_bytes([data[2], data[3]]);
        Some((status, change))
    }

    /// Power on all downstream ports of a hub.
    fn power_on_ports(xhc: &mut XhciController, hub: &UsbHubState) {
        for port in 1..=hub.num_ports {
            Self::set_port_feature(xhc, hub.slot_id, port, hub_port_feature::PORT_POWER);
            ffi::udelay(2_000); // 2ms between port power
        }
        // Wait for power to stabilize
        let delay_ms = (hub.hub_desc.power_on_to_good() as u32) * 2;
        ffi::udelay(delay_ms * 1000);
    }

    /// Reset a downstream hub port and wait for completion.
    fn reset_port(xhc: &mut XhciController, slot_id: u8, port: u8,
        buf_virt: *mut u8, buf_phys: u64
    ) -> bool {
        if !Self::set_port_feature(xhc, slot_id, port, hub_port_feature::PORT_RESET) {
            return false;
        }
        // Wait for reset to complete (port reset change bit)
        for _ in 0..100 {
            ffi::udelay(10_000); // 10ms
            match Self::get_port_status(xhc, slot_id, port, buf_virt, buf_phys) {
                Some((_, change)) => {
                    if (change & hub_port_change::PORT_RESET) != 0 {
                        // Clear the reset change bit
                        Self::clear_port_feature(xhc, slot_id, port, hub_port_feature::PORT_RESET);
                        return true;
                    }
                }
                None => return false,
            }
        }
        false
    }

    /// Get the speed of a device connected to a hub port.
    fn port_speed(xhc: &mut XhciController, slot_id: u8, port: u8,
        buf_virt: *mut u8, buf_phys: u64
    ) -> u8 {
        match Self::get_port_status(xhc, slot_id, port, buf_virt, buf_phys) {
            Some((status, _)) => {
                if (status & hub_port_status::PORT_LOW_SPEED) != 0 {
                    speed::LOW
                } else if (status & hub_port_status::PORT_HIGH_SPEED) != 0 {
                    speed::HIGH
                } else {
                    speed::FULL
                }
            }
            None => 0,
        }
    }

    /// Check if a hub port has a device connected.
    fn port_connected(xhc: &mut XhciController, slot_id: u8, port: u8,
        buf_virt: *mut u8, buf_phys: u64
    ) -> bool {
        match Self::get_port_status(xhc, slot_id, port, buf_virt, buf_phys) {
            Some((status, change)) => {
                (status & hub_port_status::PORT_CONNECTION) != 0 ||
                (change & hub_port_change::PORT_CONNECTION) != 0
            }
            None => false,
        }
    }

    /// Poll the hub's interrupt endpoint for port status change bitmap.
    fn poll_interrupt(xhc: &mut XhciController, hub: &mut UsbHubState) -> bool {
        if hub.intr_ep == 0 || hub.intr_buf.is_none() {
            return false;
        }
        let buf = hub.intr_buf.as_mut().unwrap();
        let num_bytes = ((hub.num_ports as usize) + 7) / 8;
        let transfer_len = core::cmp::max(num_bytes as u32, hub.intr_mps as u32);

        if !xhc.queue_bulk_transfer(hub.slot_id, hub.intr_ep, true, buf.phys, transfer_len, true) {
            return false;
        }
        if !xhc.poll_transfer_event(100_000) { // 100ms timeout for interrupt poll
            return false;
        }

        // Read bitmap from DMA buffer
        let data = unsafe { core::slice::from_raw_parts(buf.virt as *const u8, num_bytes) };
        let mut bitmap = 0u16;
        for i in 0..num_bytes {
            if i < 2 {
                bitmap |= (data[i] as u16) << (i * 8);
            }
        }
        hub.port_change_bitmap = bitmap;
        bitmap != 0
    }

    /// Process port status changes on a hub. Enumerates newly connected devices.
    fn process_port_changes(xhc: &mut XhciController, slot_id: u8,
        change_bitmap: u16, buf_virt: *mut u8, buf_phys: u64
    ) {
        for port in 1..=MAX_HUB_PORTS as u8 {
            if (change_bitmap & (1u16 << port)) != 0 {
                // Clear the port change
                Self::clear_port_feature(xhc, slot_id, port, hub_port_feature::PORT_CONNECTION);

                // Check if a device is connected
                if !Self::port_connected(xhc, slot_id, port, buf_virt, buf_phys) {
                    continue;
                }

                // Reset the port
                if !Self::reset_port(xhc, slot_id, port, buf_virt, buf_phys) {
                    if xhc.verbose >= 1 {
                        ffi::print(b"xHCI: hub port reset failed\0");
                    }
                    continue;
                }

                // Get device speed
                let dev_speed = Self::port_speed(xhc, slot_id, port, buf_virt, buf_phys);
                if dev_speed == 0 {
                    continue;
                }

                // Enable slot and enumerate
                let new_slot_id = xhc.enable_slot();
                if new_slot_id == 0 {
                    continue;
                }

                if !xhc.address_device(new_slot_id, port, dev_speed, true) {
                    if xhc.verbose >= 1 {
                        ffi::print(b"xHCI: hub device address failed\0");
                    }
                    continue;
                }

                if xhc.verbose >= 1 {
                    ffi::print(b"xHCI: hub device enumerated\0");
                }
            }
        }
    }
}

impl UsbClassDriver for UsbHubDriver {
    fn class_code(&self) -> u8 { usb_class::HUB }
    fn subclass_code(&self) -> u8 { 0 }
    fn protocol_code(&self) -> u8 { 0 }

    fn name(&self) -> &'static [u8] { b"USB Hub\0" }

    fn probe(&mut self, xhc: &mut XhciController, slot_id: u8, _dev_info: &UsbDeviceInfo) -> ProbeResult {
        if self.num_hubs >= MAX_HUBS {
            return ProbeResult::Failed;
        }

        if self.verbose >= 1 {
            ffi::print(b"xHCI: probing hub\0");
        }

        // Setup EP0 transfer ring
        if !xhc.setup_ep0_transfer_ring(slot_id) {
            return ProbeResult::Failed;
        }

        // Fetch hub descriptor (USB 2.0 format first)
        let mut hub_buf = match RingMem::alloc(64) {
            Some(b) => b,
            None => return ProbeResult::Failed,
        };

        if !Self::fetch_hub_descriptor(xhc, slot_id, hub_buf.virt, hub_buf.phys, 7) {
            hub_buf.free();
            return ProbeResult::Failed;
        }

        // Parse hub descriptor
        let hub_data = unsafe { core::slice::from_raw_parts(hub_buf.virt as *const u8, 7) };
        let hub_desc = match HubDescriptor::parse(hub_data) {
            Some(d) => d,
            None => { hub_buf.free(); return ProbeResult::Failed; }
        };

        let num_ports = core::cmp::min(hub_desc.num_ports(), MAX_HUB_PORTS as u8);

        // Register the hub
        let hub_idx = self.num_hubs;
        self.hubs[hub_idx].slot_id = slot_id;
        self.hubs[hub_idx].num_ports = num_ports;
        self.hubs[hub_idx].hub_desc = hub_desc;
        self.hubs[hub_idx].is_ss = false;
        self.hubs[hub_idx].intr_ep = 0;
        self.hubs[hub_idx].intr_mps = 0;
        self.hubs[hub_idx].intr_buf = None;
        self.num_hubs += 1;

        // Power on all downstream ports
        Self::power_on_ports(xhc, &self.hubs[hub_idx]);

        // Try to find interrupt endpoint from config descriptor
        let cfg_data = unsafe { core::slice::from_raw_parts(hub_buf.virt as *const u8, hub_buf.size) };

        // For the interrupt endpoint, we need the full config descriptor
        // Use a separate buffer since hub_buf was used for hub descriptor
        hub_buf.free();

        let mut cfg_buf = match RingMem::alloc(256) {
            Some(b) => b,
            None => return ProbeResult::Claimed, // Hub works without interrupt polling
        };

        if xhc.get_config_descriptor(slot_id, 0, cfg_buf.virt, cfg_buf.phys, cfg_buf.size) {
            let cfg_data = unsafe { core::slice::from_raw_parts(cfg_buf.virt as *const u8, cfg_buf.size) };
            // Scan for interrupt endpoint
            let mut i = 0;
            while i + 1 < cfg_data.len() {
                let len = cfg_data[i] as usize;
                let desc_type = cfg_data[i + 1];
                if len < 2 { break; }

                if desc_type == usb_descriptor::ENDPOINT && i + 7 <= cfg_data.len() {
                    let ep = match EndpointDescriptor::parse(&cfg_data[i..]) {
                        Some(e) => e,
                        None => break,
                    };
                    if ep.transfer_type() == usb_xfer_type::INTERRUPT {
                        if ep.is_in() {
                            // Found interrupt IN endpoint for status changes
                            let ep_num = ep.endpoint_number();
                            let mps = ep.max_packet_size();
                            let dci = XhciController::ep_num_to_dci(ep_num, true);

                            // Configure interrupt endpoint
                            if xhc.configure_endpoint(slot_id, dci, 7 /* Interrupt IN */,
                                mps, 3, mps as u16) {
                                // Allocate interrupt buffer
                                let intr_buf = match RingMem::alloc(64) {
                                    Some(b) => b,
                                    None => { cfg_buf.free(); return ProbeResult::Claimed; }
                                };

                                self.hubs[hub_idx].intr_ep = ep_num;
                                self.hubs[hub_idx].intr_mps = mps;
                                self.hubs[hub_idx].intr_buf = Some(intr_buf);
                            }
                            break;
                        }
                    }
                }
                if len == 0 { break; }
                i += len;
            }
        }
        cfg_buf.free();

        if self.verbose >= 1 {
            ffi::print(b"xHCI: hub initialized\0");
        }
        ProbeResult::Claimed
    }

    fn disconnect(&mut self, xhc: &mut XhciController, slot_id: u8) {
        // Free interrupt buffer via mutable access
        if let Some(hub) = self.find_hub_by_slot_mut(slot_id) {
            if let Some(b) = &mut hub.intr_buf {
                b.free();
                hub.intr_buf = None;
            }
        }
        self.remove_hub(slot_id);
    }
}
