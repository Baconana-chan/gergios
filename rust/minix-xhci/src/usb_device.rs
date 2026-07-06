//! # USB Device Framework — Class Driver Abstraction
//!
//! Provides:
//! - `UsbDeviceType` — classification of detected USB devices
//! - `UsbClassDriver` trait — interface for USB class drivers (MSC, Hub, HID, etc.)
//! - `UsbDeviceRegistry` — registry of devices + class driver dispatch
//!
//! This decouples class-specific drivers (usb_msc, usb_hub) from the core
//! xHCI controller (xhci.rs), enabling pluggable USB class support.

use crate::registers::{
    usb_class, DeviceDescriptor, InterfaceDescriptor,
};
use crate::xhci::XhciController;

// ============================================================================
// USB Device Classification
// ============================================================================

/// Broad device type based on USB class codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UsbDeviceType {
    /// Hub (class 0x09)
    Hub,
    /// Mass Storage (class 0x08)
    MassStorage,
    /// HID (class 0x03 — keyboards, mice, etc.)
    Hid,
    /// Audio (class 0x01)
    Audio,
    /// Video (class 0x0E)
    Video,
    /// Communications (class 0x02)
    Comm,
    /// Printer (class 0x07)
    Printer,
    /// Bluetooth (class 0xE0, subclass 0x01 — Wireless Controller)
    Bluetooth,
    /// Vendor-specific or unknown
    Other,
}

impl UsbDeviceType {
    /// Classify a device from its interface class.
    pub fn from_interface_class(class: u8) -> Self {
        match class {
            usb_class::HUB => UsbDeviceType::Hub,
            usb_class::MASS_STORAGE => UsbDeviceType::MassStorage,
            usb_class::HID => UsbDeviceType::Hid,
            usb_class::AUDIO => UsbDeviceType::Audio,
            usb_class::VIDEO => UsbDeviceType::Video,
            usb_class::COMMUNICATIONS => UsbDeviceType::Comm,
            usb_class::PRINTER => UsbDeviceType::Printer,
            usb_class::WIRELESS => UsbDeviceType::Bluetooth,
            _ => UsbDeviceType::Other,
        }
    }

    /// Check if this device type has a registered class driver.
    pub fn has_driver(&self) -> bool {
        matches!(self, UsbDeviceType::MassStorage | UsbDeviceType::Hub | UsbDeviceType::Hid | UsbDeviceType::Bluetooth)
    }
}

// ============================================================================
// Registered USB Device Entry
// ============================================================================

/// Information about a detected USB device.
pub struct UsbDeviceInfo {
    /// xHCI slot ID.
    pub slot_id: u8,
    /// Root hub port or hub downstream port.
    pub port: u8,
    /// USB speed code (4=Full, 5=Low, 6=High, 7=Super).
    pub speed_code: u8,
    /// Device type (from interface class).
    pub device_type: UsbDeviceType,
    /// Vendor ID from device descriptor.
    pub vendor_id: u16,
    /// Product ID from device descriptor.
    pub product_id: u16,
    /// Whether a class driver has been attached.
    pub driver_attached: bool,
    /// Device descriptor data (cached).
    pub dev_desc: DeviceDescriptor,
    /// Class driver index (which entry in class_drivers array).
    pub driver_idx: usize,
}

impl UsbDeviceInfo {
    fn new() -> Self {
        Self {
            slot_id: 0,
            port: 0,
            speed_code: 0,
            device_type: UsbDeviceType::Other,
            vendor_id: 0,
            product_id: 0,
            driver_attached: false,
            dev_desc: unsafe { core::mem::zeroed() },
            driver_idx: 0,
        }
    }
}

// ============================================================================
// Class Driver Trait
// ============================================================================

/// Result of probing a device with a class driver.
pub enum ProbeResult {
    /// Driver claimed the device successfully.
    Claimed,
    /// Driver does not handle this device.
    NotMine,
    /// Driver would handle but initialization failed.
    Failed,
}

/// A USB class driver that can claim and manage devices.
pub trait UsbClassDriver {
    /// Return the USB interface class code this driver handles.
    fn class_code(&self) -> u8;

    /// Optionally, also match on subclass and protocol.
    fn subclass_code(&self) -> u8 { 0 }
    fn protocol_code(&self) -> u8 { 0 }

    /// Probe a newly detected device. Return Claimed if this driver
    /// successfully initialized the device.
    fn probe(&mut self, xhc: &mut XhciController, slot_id: u8, dev_info: &UsbDeviceInfo) -> ProbeResult;

    /// Called when a device is disconnected.
    fn disconnect(&mut self, xhc: &mut XhciController, slot_id: u8);

    /// Name of this driver for logging.
    fn name(&self) -> &'static [u8];
}

// ============================================================================
// Device Registry
// ============================================================================

/// Maximum number of detected USB devices to track.
pub const MAX_USB_DEVICES: usize = 16;

/// Maximum number of registered class drivers.
pub const MAX_CLASS_DRIVERS: usize = 8;

/// Global registry of USB devices and class drivers.
pub struct UsbDeviceRegistry {
    /// Detected USB devices.
    pub devices: [UsbDeviceInfo; MAX_USB_DEVICES],
    /// Number of active devices.
    pub num_devices: usize,
    /// Registered class drivers.
    pub class_drivers: [Option<&'static mut dyn UsbClassDriver>; MAX_CLASS_DRIVERS],
    /// Number of registered drivers.
    pub num_drivers: usize,
}

impl UsbDeviceRegistry {
    pub fn new() -> Self {
        Self {
            devices: core::array::from_fn(|_| UsbDeviceInfo::new()),
            num_devices: 0,
            class_drivers: [None, None, None, None, None, None, None, None],
            num_drivers: 0,
        }
    }

    /// Register a class driver. Returns the driver index or None if full.
    pub fn register_driver(&mut self, driver: &'static mut dyn UsbClassDriver) -> Option<usize> {
        if self.num_drivers >= MAX_CLASS_DRIVERS {
            return None;
        }
        let idx = self.num_drivers;
        self.class_drivers[idx] = Some(driver);
        self.num_drivers += 1;
        Some(idx)
    }

    /// Find a free device slot and register a new device.
    pub fn register_device(&mut self, dev: UsbDeviceInfo) -> Option<usize> {
        if self.num_devices >= MAX_USB_DEVICES {
            return None;
        }
        self.devices[self.num_devices] = dev;
        self.num_devices += 1;
        Some(self.num_devices - 1)
    }

    /// Find a device by slot_id.
    pub fn find_by_slot(&self, slot_id: u8) -> Option<&UsbDeviceInfo> {
        self.devices[..self.num_devices].iter().find(|d| d.slot_id == slot_id)
    }

    /// Find a device by slot_id (mutable).
    pub fn find_by_slot_mut(&mut self, slot_id: u8) -> Option<&mut UsbDeviceInfo> {
        self.devices[..self.num_devices].iter_mut().find(|d| d.slot_id == slot_id)
    }

    /// Remove a device by slot_id.
    pub    fn remove_device(&mut self, slot_id: u8) {
        let mut found = None;
        for i in 0..self.num_devices {
            if self.devices[i].slot_id == slot_id {
                found = Some(i);
                break;
            }
        }
        if let Some(idx) = found {
            // Shift remaining devices down (use temp swap to avoid borrow conflict)
            for j in idx..self.num_devices - 1 {
                let tmp = core::mem::replace(&mut self.devices[j + 1], UsbDeviceInfo::new());
                self.devices[j] = tmp;
            }
            self.num_devices -= 1;
        }
    }

    /// Find a class driver index that matches the given interface class.
    fn find_driver_idx(&self, class: u8) -> Option<usize> {
        for i in 0..self.num_drivers {
            if let Some(drv) = &self.class_drivers[i] {
                if drv.class_code() == class {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Dispatch a newly detected device to the appropriate class driver.
    /// Returns true if a driver claimed the device.
    pub fn dispatch_to_driver(
        &mut self, xhc: &mut XhciController, slot_id: u8, device_type: UsbDeviceType
    ) -> bool {
        let class_code = match device_type {
            UsbDeviceType::Hub => usb_class::HUB,
            UsbDeviceType::MassStorage => usb_class::MASS_STORAGE,
            UsbDeviceType::Hid => usb_class::HID,
            _ => return false,
        };

        let driver_idx = match self.find_driver_idx(class_code) {
            Some(i) => i,
            None => return false,
        };

        // Build a temporary device info for probing
        let dummy_info = UsbDeviceInfo {
            slot_id, port: 0, speed_code: 0,
            device_type, vendor_id: 0, product_id: 0,
            driver_attached: false,
            dev_desc: unsafe { core::mem::zeroed() },
            driver_idx: 0,
        };

        match self.class_drivers[driver_idx].as_mut() {
            Some(driver) => {
                match driver.probe(xhc, slot_id, &dummy_info) {
                    ProbeResult::Claimed => true,
                    _ => false,
                }
            }
            None => false,
        }
    }

    /// Notify all drivers of a device disconnect.
    pub fn notify_disconnect(&mut self, xhc: &mut XhciController, slot_id: u8) {
        // Get device info before any mutable borrows
        let device_type = match self.find_by_slot(slot_id) {
            Some(d) => d.device_type,
            None => return,
        };
        let driver_attached = match self.find_by_slot(slot_id) {
            Some(d) => d.driver_attached,
            None => return,
        };

        if driver_attached {
            let class_code = match device_type {
                UsbDeviceType::Hub => usb_class::HUB,
                UsbDeviceType::MassStorage => usb_class::MASS_STORAGE,
                UsbDeviceType::Hid => usb_class::HID,
                _ => { self.remove_device(slot_id); return; }
            };
            let driver_idx = match self.find_driver_idx(class_code) {
                Some(i) => i,
                None => { self.remove_device(slot_id); return; }
            };
            if let Some(driver) = self.class_drivers[driver_idx].as_mut() {
                driver.disconnect(xhc, slot_id);
            }
        }
        self.remove_device(slot_id);
    }
}

// ============================================================================
// Device Enumeration Pipeline
// ============================================================================

/// Result from scanning the config descriptor to find the first
/// non-per-interface class code.
/// Returns the interface class of the first interface descriptor found.
pub fn get_device_class_from_config(config_data: &[u8]) -> Option<(u8, u8, u8)> {
    let mut i = 0;
    while i + 1 < config_data.len() {
        let len = config_data[i] as usize;
        let desc_type = config_data[i + 1];
        if len < 2 { break; }

        if desc_type == crate::registers::usb_descriptor::INTERFACE {
            if i + 9 <= config_data.len() {
                let iface = match InterfaceDescriptor::parse(&config_data[i..]) {
                    Some(iface) => iface,
                    None => break,
                };
                return Some((iface.bInterfaceClass, iface.bInterfaceSubClass, iface.bInterfaceProtocol));
            }
        }
        if len == 0 { break; }
        i += len;
    }
    None
}
