//! # USB Driver Interface
//!
//! Provides a URB-like abstraction for USB transfers, a chardev interface
//! for userspace access, and the class driver dispatch layer that routes
//! USB device events to the appropriate class driver.
//!
//! ## Architecture
//!
//! ```text
//! xhci.rs (controller)  ←→  usb_interface.rs (URB + dispatch)
//!                               ↕
//!                    ┌──────────┼──────────┐
//!                    ↓          ↓          ↓
//!              usb_msc.rs  usb_hub.rs  usb_hid.rs (future)
//! ```
//!
//! The interface is built on top of the USB Device Framework (usb_device.rs)
//! which provides the class driver trait and device registry.

use crate::ffi;
use crate::registers::{
    self, build_setup_packet, usb_req, usb_descriptor,
};
use crate::ring::RingMem;
use crate::xhci::XhciController;
use crate::usb_device;

// ============================================================================
// URB (USB Request Block) Abstraction
// ============================================================================

/// Transfer types for URBs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum UrbTransferType {
    /// Control transfer (EP0)
    Control,
    /// Bulk transfer
    Bulk,
    /// Interrupt transfer
    Interrupt,
    /// Isochronous transfer
    Isochronous,
}

/// USB Request Block — abstraction for a single USB transaction.
/// Encapsulates the endpoint, direction, data buffer, and transfer parameters.
pub struct Urb {
    /// Target device slot ID.
    pub slot_id: u8,
    /// Endpoint number (0=EP0, 1..15 for others).
    pub endpoint: u8,
    /// Direction: true = IN (device→host).
    pub direction_in: bool,
    /// Transfer type.
    pub transfer_type: UrbTransferType,
    /// Physical address of data buffer.
    pub data_phys: u64,
    /// Virtual address of data buffer.
    pub data_virt: *mut u8,
    /// Transfer length in bytes.
    pub transfer_len: u32,
    /// Actual transferred length (filled on completion).
    pub actual_len: u32,
    /// Completion status: true = success.
    pub success: bool,
    /// Interrupt on completion.
    pub ioc: bool,
}

impl Urb {
    /// Create a new URB for a bulk transfer.
    pub fn new_bulk(slot_id: u8, endpoint: u8, dir_in: bool,
        data_virt: *mut u8, data_phys: u64, len: u32, ioc: bool
    ) -> Self {
        Self {
            slot_id, endpoint, direction_in: dir_in,
            transfer_type: UrbTransferType::Bulk,
            data_phys, data_virt, transfer_len: len,
            actual_len: 0, success: false, ioc,
        }
    }

    /// Create a new URB for a control transfer.
    pub fn new_control(slot_id: u8, setup_pkt: &[u8; 8],
        dir_in: bool, data_virt: *mut u8, data_phys: u64, len: u32
    ) -> Self {
        Self {
            slot_id, endpoint: 0, direction_in: dir_in,
            transfer_type: UrbTransferType::Control,
            data_phys, data_virt, transfer_len: len,
            actual_len: 0, success: false, ioc: true,
        }
    }

    /// Submit this URB to the xHCI controller.
    /// Returns true if the transfer was submitted and completed successfully.
    pub fn submit(&mut self, xhc: &mut XhciController) -> bool {
        match self.transfer_type {
            UrbTransferType::Bulk | UrbTransferType::Interrupt => {
                self.success = xhc.queue_bulk_transfer(
                    self.slot_id, self.endpoint, self.direction_in,
                    self.data_phys, self.transfer_len, self.ioc
                );
                if self.success && self.ioc {
                    // Wait for transfer event completion
                    let timeout = if self.transfer_type == UrbTransferType::Interrupt {
                        100_000 // 100ms for interrupt
                    } else {
                        30_000_000 // 30s for bulk
                    };
                    self.success = xhc.poll_transfer_event(timeout);
                    if self.success {
                        self.actual_len = self.transfer_len;
                    }
                } else if self.success {
                    self.actual_len = self.transfer_len;
                }
                self.success
            }
            UrbTransferType::Control => {
                // Control transfers use control_transfer method
                self.success = xhc.control_transfer(
                    self.slot_id, unsafe { &*(self.data_virt as *const [u8; 8]) },
                    self.direction_in, self.data_phys, self.transfer_len
                );
                if self.success {
                    self.actual_len = self.transfer_len;
                }
                self.success
            }
            UrbTransferType::Isochronous => {
                // Not yet implemented
                false
            }
        }
    }
}

// ============================================================================
// Character Device Interface
// ============================================================================

/// A character device interface through which userspace can interact with
/// a USB device. Provides standard read/write/ioctl operations.
pub struct UsbCharacterDevice {
    /// Minor device number.
    pub minor: u8,
    /// Slot ID this chardev is attached to.
    pub slot_id: u8,
    /// Whether this device is open.
    pub open: bool,
    /// Default IN endpoint for read operations.
    pub ep_in: u8,
    /// Default OUT endpoint for write operations.
    pub ep_out: u8,
    /// DMA buffer for data transfer.
    pub dma_buf: Option<RingMem>,
}

impl UsbCharacterDevice {
    pub fn new(minor: u8, slot_id: u8, ep_in: u8, ep_out: u8,
        buf_size: usize
    ) -> Option<Self> {
        let dma_buf = RingMem::alloc(buf_size / core::mem::size_of::<crate::registers::Trb>());
        Some(Self {
            minor, slot_id, open: false, ep_in, ep_out, dma_buf,
        })
    }

    pub fn open(&mut self) -> bool {
        if self.open { return false; }
        self.open = true;
        true
    }

    pub fn close(&mut self) {
        self.open = false;
    }

    /// Send a control transfer via this chardev.
    pub fn control(&mut self, xhc: &mut XhciController,
        bm_request_type: u8, b_request: u8,
        w_value: u16, w_index: u16, w_length: u16,
        dir_in: bool
    ) -> bool {
        let pkt = build_setup_packet(bm_request_type, b_request, w_value, w_index, w_length);
        match &self.dma_buf {
            Some(buf) => xhc.control_transfer(self.slot_id, &pkt, dir_in, buf.phys, w_length as u32),
            None => false,
        }
    }
}

// ============================================================================
// Enumeration Endpoint — runs after basic address phase
// ============================================================================

/// Perform full enumeration of a device after address_device has succeeded.
/// This reads the device descriptor, config descriptor, classifies the device,
/// registers it in the device registry, and dispatches to a class driver.
///
/// Returns true if a class driver claimed the device.
pub fn enumerate_device_full(
    xhc: &mut XhciController, slot_id: u8,
    buf_virt: *mut u8, buf_phys: u64, buf_size: usize
) -> bool {
    if buf_size < 512 { return false; }

    // Step 1: Read device descriptor
    let dev_desc = match xhc.get_device_descriptor(slot_id, buf_virt, buf_phys) {
        Some(d) => d,
        None => return false,
    };

    // Step 2: Setup EP0 transfer ring for control transfers
    if !xhc.setup_ep0_transfer_ring(slot_id) {
        return false;
    }

    // Step 3: Read config descriptor (header first, then full)
    if !xhc.get_config_descriptor(slot_id, 0, buf_virt, buf_phys, buf_size) {
        return false;
    }

    // Step 4: Classify device from config descriptor
    let config_data = unsafe { core::slice::from_raw_parts(buf_virt as *const u8, buf_size) };
    let (iface_class, _, _) = match usb_device::get_device_class_from_config(config_data) {
        Some(c) => c,
        None => return false,
    };

    let device_type = usb_device::UsbDeviceType::from_interface_class(iface_class);

    // Step 5: Register device in the registry
    let dev_info = usb_device::UsbDeviceInfo {
        slot_id,
        port: xhc.slots[slot_id as usize].port,
        speed_code: xhc.slots[slot_id as usize].speed,
        device_type,
        vendor_id: dev_desc.vendor_id(),
        product_id: dev_desc.product_id(),
        driver_attached: false,
        dev_desc,
        driver_idx: 0,
    };

    xhc.device_registry.register_device(dev_info);

    // Step 6: Dispatch to class driver (use raw ptr to avoid double-borrow on xhc)
    if device_type.has_driver() {
        let reg = unsafe { &mut *(&mut xhc.device_registry as *mut crate::usb_device::UsbDeviceRegistry) };
        reg.dispatch_to_driver(xhc, slot_id, device_type)
    } else {
        false
    }
}
