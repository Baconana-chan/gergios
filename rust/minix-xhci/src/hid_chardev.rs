//! # HID Character Device — /dev/kbd0, /dev/mouse0
//!
//! Implements MINIX chardev interface for USB HID devices (keyboard, mouse).
//! Data is polled from `xhci_alarm()` and pushed to ring buffers.
//!
//! ## Minor Number Layout
//!
//! | Minor | Device      | Description                     |
//! |-------|-------------|---------------------------------|
//! | 4     | `/dev/kbd0`   | First USB HID keyboard         |
//! | 5     | `/dev/kbd1`   | Second USB HID keyboard        |
//! | 6     | `/dev/mouse0` | First USB HID mouse            |
//! | 7     | `/dev/mouse1` | Second USB HID mouse           |
//! | 8     | `/dev/gamepad0` | First USB HID gamepad        |
//! | 9     | `/dev/gamepad1` | Second USB HID gamepad       |
//!
//! ## Report Format (read returns these bytes)
//!
//! **Keyboard** (8 bytes): Boot protocol format
//! - byte 0: modifiers (bit 0=LCtrl, 1=LShift, 2=LAlt, 3=LGui, 4=RCtrl, 5=RShift, 6=RAlt, 7=RGui)
//! - byte 1: reserved (0x00)
//! - bytes 2-7: key codes (up to 6 simultaneous, 0 = no key)
//!
//! **Mouse** (8 bytes): Extended boot protocol format
//! - byte 0: buttons (bit 0=Left, 1=Right, 2=Middle, 3-7 reserved)
//! - byte 1: X movement (signed, relative)
//! - byte 2: Y movement (signed, relative)
//! - byte 3: wheel movement (signed, relative, positive=scroll down)
//! - bytes 4-7: reserved (0x00)
//!
//! **Gamepad** (16 bytes): Extended gamepad state
//! - bytes 0-3: buttons (u32 LE, bit 0 = Button 1)
//! - byte 4-5: X axis (i16 LE)
//! - byte 6-7: Y axis (i16 LE)
//! - byte 8-9: Z axis (i16 LE)
//! - byte 10-11: Rx axis (i16 LE, rotation X)
//! - byte 12-13: Ry axis (i16 LE, rotation Y)
//! - byte 14: HAT switch (i8, 0-7 directions, 0x0F = centered)
//! - byte 15: reserved
//!
//! Returns -1 (EAGAIN) on read when no new data is available.

#![allow(dead_code)]

use core::ffi::{c_int, c_ulong, c_void};

use crate::ffi;
use crate::usb_hid;

// ============================================================================
// Constants
// ============================================================================

/// Minor number base for HID devices (starts at 4, after BT minors 0-3).
pub const HID_KBD0_MINOR: ffi::DevMinor = 4;
pub const HID_KBD1_MINOR: ffi::DevMinor = 5;
pub const HID_MS0_MINOR: ffi::DevMinor = 6;
pub const HID_MS1_MINOR: ffi::DevMinor = 7;
pub const HID_GP0_MINOR: ffi::DevMinor = 8;
pub const HID_GP1_MINOR: ffi::DevMinor = 9;
pub const HID_NUM_MINORS: usize = 10;

/// Maximum number of keyboard/mouse/gamepad pairs.
const HID_MAX_PAIRS: usize = 2;

/// Report size: 16 bytes (large enough for keyboard, mouse, and gamepad reports).
const REPORT_SIZE: usize = 16;

// ============================================================================
// HID Chardev State
// ============================================================================

/// Per-minor chardev state.
pub struct HidChardevState {
    /// Whether this slot is in use (allocated).
    pub in_use: bool,
    /// Whether this is a keyboard or mouse.
    pub kind: usb_hid::HidDeviceKind,
    /// Index into HID_DRIVER.devices[].
    pub device_idx: usize,
    /// True when new data has arrived since last read.
    pub has_data: bool,
    /// Latest report data (8 bytes, zero-padded).
    pub report: [u8; REPORT_SIZE],
}

impl HidChardevState {
    const fn unused() -> Self {
        Self {
            in_use: false,
            kind: usb_hid::HidDeviceKind::Other,
            device_idx: 0,
            has_data: false,
            report: [0u8; REPORT_SIZE],
        }
    }

    fn alloc(minor: ffi::DevMinor, kind: usb_hid::HidDeviceKind, device_idx: usize) -> Self {
        Self {
            in_use: true,
            kind,
            device_idx,
            has_data: false,
            report: [0u8; REPORT_SIZE],
        }
    }
}

// ============================================================================
// HID Chardev Manager
// ============================================================================

/// Manager for all HID chardev slots.
pub struct HidChardevManager {
    pub devices: [HidChardevState; HID_NUM_MINORS],
}

impl HidChardevManager {
    pub fn new() -> Self {
        Self { devices: core::array::from_fn(|_| HidChardevState::unused()) }
    }

    /// Allocate a chardev slot for a HID device.
    /// Returns the minor number, or None if no free slot.
    pub fn alloc(&mut self, kind: usb_hid::HidDeviceKind, device_idx: usize) -> Option<ffi::DevMinor> {
        let base = match kind {
            usb_hid::HidDeviceKind::Keyboard => HID_KBD0_MINOR,
            usb_hid::HidDeviceKind::Mouse => HID_MS0_MINOR,
            usb_hid::HidDeviceKind::Gamepad => HID_GP0_MINOR,
            _ => return None,
        } as usize;

        for offset in 0..HID_MAX_PAIRS {
            let idx = base + offset;
            if idx < HID_NUM_MINORS && !self.devices[idx].in_use {
                self.devices[idx] = HidChardevState::alloc(idx as ffi::DevMinor, kind, device_idx);
                return Some(idx as ffi::DevMinor);
            }
        }
        None
    }

    /// Free a chardev slot by minor.
    pub fn free(&mut self, minor: ffi::DevMinor) {
        let idx = minor as usize;
        if idx < HID_NUM_MINORS {
            self.devices[idx] = HidChardevState::unused();
        }
    }

    /// Find a chardev slot by device index.
    pub fn find_by_device(&self, device_idx: usize, kind: usb_hid::HidDeviceKind) -> Option<ffi::DevMinor> {
        for i in 0..HID_NUM_MINORS {
            if self.devices[i].in_use
                && self.devices[i].device_idx == device_idx
                && self.devices[i].kind == kind
            {
                return Some(i as ffi::DevMinor);
            }
        }
        None
    }

    /// Get mutable reference to a device by minor.
    pub fn get_mut(&mut self, minor: ffi::DevMinor) -> Option<&mut HidChardevState> {
        let idx = minor as usize;
        if idx < HID_NUM_MINORS && self.devices[idx].in_use {
            Some(&mut self.devices[idx])
        } else {
            None
        }
    }

    /// Push keyboard state into the chardev report buffer.
    /// Called from xhci_alarm().
    pub fn push_keyboard(&mut self, device_idx: usize, kbd: &usb_hid::KeyboardState) {
        let minor = match self.find_by_device(device_idx, usb_hid::HidDeviceKind::Keyboard) {
            Some(m) => m,
            None => return,
        };
        let dev = &mut self.devices[minor as usize];
        dev.report[0] = kbd.modifiers;
        dev.report[1] = 0; // reserved
        for i in 0..usb_hid::MAX_KEYS {
            dev.report[2 + i] = if i < kbd.key_count as usize { kbd.keys[i] } else { 0 };
        }
        dev.has_data = true;
    }

    /// Push mouse state into the chardev report buffer.
    /// Called from xhci_alarm().
    pub fn push_mouse(&mut self, device_idx: usize, mouse: &usb_hid::MouseState) {
        let minor = match self.find_by_device(device_idx, usb_hid::HidDeviceKind::Mouse) {
            Some(m) => m,
            None => return,
        };
        let dev = &mut self.devices[minor as usize];
        dev.report[0] = mouse.buttons;
        dev.report[1] = mouse.x as u8;
        dev.report[2] = mouse.y as u8;
        dev.report[3] = mouse.wheel as u8;
        dev.has_data = true;
    }

    /// Push gamepad state into the chardev report buffer.
    /// Called from xhci_alarm().
    /// Report format: 16 bytes — buttons(u32 LE) + X(i16) + Y(i16) + Z(i16) + Rx(i16) + Ry(i16) + HAT(i8) + reserved
    pub fn push_gamepad(&mut self, device_idx: usize, gp: &usb_hid::GamepadState) {
        let minor = match self.find_by_device(device_idx, usb_hid::HidDeviceKind::Gamepad) {
            Some(m) => m,
            None => return,
        };
        let dev = &mut self.devices[minor as usize];
        let report = &mut dev.report;
        // Bytes 0-3: buttons (u32 LE)
        report[0..4].copy_from_slice(&gp.buttons.to_le_bytes());
        // Bytes 4-5: X axis (i16 LE)
        report[4..6].copy_from_slice(&gp.x.to_le_bytes());
        // Bytes 6-7: Y axis (i16 LE)
        report[6..8].copy_from_slice(&gp.y.to_le_bytes());
        // Bytes 8-9: Z axis (i16 LE)
        report[8..10].copy_from_slice(&gp.z.to_le_bytes());
        // Bytes 10-11: Rx axis (i16 LE)
        report[10..12].copy_from_slice(&gp.rx.to_le_bytes());
        // Bytes 12-13: Ry axis (i16 LE)
        report[12..14].copy_from_slice(&gp.ry.to_le_bytes());
        // Byte 14: HAT switch
        report[14] = gp.hat as u8;
        // Byte 15: reserved
        report[15] = 0;
        dev.has_data = true;
    }
}

// ============================================================================
// Global access helpers
// ============================================================================

unsafe fn hid_chardev_mgr_ptr() -> *mut HidChardevManager {
    let ptr = core::ptr::addr_of_mut!(crate::HID_CHARDEV);
    unsafe { (*ptr).as_mut().expect("HID_CHARDEV not initialized") as *mut HidChardevManager }
}

// ============================================================================
// C-callable Chardev Callbacks
// ============================================================================

unsafe extern "C" fn hid_open_c(minor: ffi::DevMinor, _flags: c_int, _endpt: ffi::endpoint_t) -> c_int {
    let mgr = unsafe { &mut *hid_chardev_mgr_ptr() };
    match mgr.get_mut(minor) {
        Some(s) if s.in_use => ffi::OK,
        _ => ffi::ENXIO,
    }
}

unsafe extern "C" fn hid_close_c(_minor: ffi::DevMinor) -> c_int {
    ffi::OK
}

unsafe extern "C" fn hid_read_c(
    minor: ffi::DevMinor, _seekpos: u64, endpoint: ffi::endpoint_t,
    grant: ffi::cp_grant_id_t, size: usize, _flags: c_int, _cdev: ffi::cdev_id_t
) -> isize {
    let mgr = unsafe { &mut *hid_chardev_mgr_ptr() };
    let dev = match mgr.get_mut(minor) {
        Some(s) => s as *mut HidChardevState,
        None => return ffi::ENXIO as isize,
    };

    // Check if data is available
    let available = unsafe { (*dev).has_data };
    if !available {
        return ffi::EAGAIN as isize;
    }

    // Read report
    let report: [u8; REPORT_SIZE] = unsafe { (*dev).report };
    unsafe { (*dev).has_data = false; }

    let copy_len = core::cmp::min(size, REPORT_SIZE);
    let r = ffi::sys_safecopyto_wrapper(
        endpoint, grant, 0,
        report.as_ptr() as *const c_void,
        copy_len as c_ulong,
    );
    if r != ffi::OK { ffi::EIO as isize } else { copy_len as isize }
}

unsafe extern "C" fn hid_write_c(
    _minor: ffi::DevMinor, _seekpos: u64, _endpoint: ffi::endpoint_t,
    _grant: ffi::cp_grant_id_t, _size: usize, _flags: c_int, _cdev: ffi::cdev_id_t
) -> isize {
    // HID devices are read-only
    ffi::ENOTTY as isize
}

unsafe extern "C" fn hid_ioctl_c(
    _minor: ffi::DevMinor, _request: c_ulong, _endpoint: ffi::endpoint_t,
    _grant: ffi::cp_grant_id_t, _flags: c_int, _user_endpt: ffi::endpoint_t
) -> c_int {
    // No ioctls for HID yet
    ffi::ENOTTY
}

// ============================================================================
// Public API
// ============================================================================

/// Build the HID chardriver callback table.
pub fn as_chardriver() -> ffi::Chardriver {
    ffi::Chardriver {
        cdr_type: -1,
        cdr_open: Some(hid_open_c),
        cdr_close: Some(hid_close_c),
        cdr_read: Some(hid_read_c),
        cdr_write: Some(hid_write_c),
        cdr_ioctl: Some(hid_ioctl_c),
        cdr_select: None,
        cdr_intr: None,
        cdr_alarm: None,
        cdr_other: None,
        cdr_device: None,
        cdr_signal: None,
    }
}
