//! # /dev/hci0 Character Device — xHCI-backed real implementation
//!
//! Implements the MINIX chardev interface for Bluetooth HCI USB transport,
//! with direct access to the xHCI controller's `BT_DRIVER` and `XHC` globals.

#![allow(dead_code)]

use core::ffi::{c_int, c_ulong, c_void};
use core::sync::atomic::Ordering;
use std::vec::Vec;

use minix_bt_hci::chardev;
use minix_bt_hci::hci;

use crate::ffi;
use crate::usb_bt::{self, BtDriver, BtAdapterState, MAX_BT_DEVICES};
use crate::xhci::XhciController;

// ============================================================================
// Constants
// ============================================================================

pub const MAX_HCI_DEVICES: usize = MAX_BT_DEVICES;

/// BT suspend ioctl — request selective USB suspend (U3).
pub const HCISUSPEND: u32 = 0x400A;
/// BT resume ioctl — exit U3 and reinitialise HCI.
pub const HCIRESUME: u32 = 0x400B;

/// Maximum idle ticks before auto-suspend.
pub const AUTO_SUSPEND_IDLE_THRESHOLD: u32 = 500;

/// Tick interval in microseconds (approximate, from alarm callback).
pub const TICK_INTERVAL_US: u32 = 10_000; // 10ms

/// Number of consecutive idle ticks before a short poll skips ACL reads.
/// Reduces polling overhead when the adapter has little activity.
pub const IDLE_POLL_INTERVAL: u32 = 10; // ~100ms before we stop checking ACLs every tick

// ============================================================================
// BT Chardev State
// ============================================================================

pub struct BtChardevState {
    pub hci: Option<chardev::HciChardev>,
    pub adapter_idx: usize,
    pub in_use: bool,
}

impl BtChardevState {
    pub fn new(minor: ffi::DevMinor, adapter_idx: usize) -> Self {
        Self {
            hci: Some(chardev::HciChardev::new(minor)),
            adapter_idx,
            in_use: true,
        }
    }

    pub const fn unused() -> Self {
        Self { hci: None, adapter_idx: 0, in_use: false }
    }

    pub fn hci_mut(&mut self) -> &mut chardev::HciChardev {
        self.hci.as_mut().expect("BtChardevState not in use")
    }
}

// ============================================================================
// BT Chardev Manager
// ============================================================================

pub struct BtChardevManager {
    pub devices: [BtChardevState; MAX_HCI_DEVICES],
}

impl BtChardevManager {
    pub fn new() -> Self {
        Self { devices: core::array::from_fn(|_| BtChardevState::unused()) }
    }

    pub fn alloc(&mut self, adapter_idx: usize) -> Option<usize> {
        for i in 0..MAX_HCI_DEVICES {
            if !self.devices[i].in_use {
                self.devices[i] = BtChardevState::new(i as ffi::DevMinor, adapter_idx);
                return Some(i);
            }
        }
        None
    }

    pub fn free(&mut self, idx: usize) {
        if idx < MAX_HCI_DEVICES {
            self.devices[idx] = BtChardevState::unused();
        }
    }

    pub fn find_by_adapter(&self, adapter_idx: usize) -> Option<usize> {
        for i in 0..MAX_HCI_DEVICES {
            if self.devices[i].in_use && self.devices[i].adapter_idx == adapter_idx {
                return Some(i);
            }
        }
        None
    }

    pub fn find_by_minor_mut(&mut self, minor: usize) -> Option<&mut BtChardevState> {
        if minor < MAX_HCI_DEVICES && self.devices[minor].in_use {
            Some(&mut self.devices[minor])
        } else {
            None
        }
    }
}

// ============================================================================
// BT Event Polling
// ============================================================================

pub fn poll_bt_events(
    bt_driver: &mut BtDriver, xhc: &mut XhciController, chardev_mgr: &mut BtChardevManager,
) {
    for i in 0..bt_driver.adapters.len() {
        // Check preconditions first (no borrow on adapter fields yet)
        let suspended: bool;
        let initialized: bool;
        let used: bool;
        {
            let adapter = &bt_driver.adapters[i];
            used = adapter.used;
            initialized = adapter.initialized;
            suspended = adapter.suspended;
        }
        if !used || !initialized || suspended { continue; }

        // Track whether this tick had any activity (reset idle counter)
        let mut had_activity = false;

        // Borrow transport for data transfer (separate from adapter's other fields)
        let activity_flag = &mut had_activity;
        {
            let adapter = &mut bt_driver.adapters[i];
            let transport = match adapter.transport.as_mut() {
                Some(t) => t,
                None => continue,
            };
            if transport.hci_state != hci::HciState::Up { continue; }

            let chardev_idx = match chardev_mgr.find_by_adapter(i) {
                Some(idx) => idx,
                None => continue,
            };
            let hci = chardev_mgr.devices[chardev_idx].hci_mut();

            // HCI events (small — safe on stack)
            let mut evt_buf = [0u8; 260];
            let evt_len = transport.recv_event(xhc, &mut evt_buf);
            if evt_len > 0 {
                hci.evt_ring.push(0x04, &evt_buf[..evt_len]);
                hci.stats.evt_rx += 1;
                hci.stats.byte_rx += evt_len as u32;
                *activity_flag = true;
            }

            // ACL data (64KB — heap-allocated via vec to avoid stack overflow)
            let mut acl_buf = vec![0u8; hci::HCI_MAX_ACL_SIZE];
            let acl_len = transport.recv_acl(xhc, &mut acl_buf);
            if acl_len > 0 {
                hci.acl_ring.push(0x02, &acl_buf[..acl_len]);
                hci.stats.acl_rx += 1;
                hci.stats.byte_rx += acl_len as u32;
                *activity_flag = true;
            }
        } // End borrow of adapter fields

        // Touch adapter outside the borrow scope (resolves E0499)
        if had_activity {
            let adapter = &mut bt_driver.adapters[i];
            adapter.touch();
        }
    }
}

// ============================================================================
// Raw global access helpers (avoids overlapping borrow across closures)
// ============================================================================

/// Get the BT_CHARDEV manager as a raw pointer.
unsafe fn chardev_mgr_ptr() -> *mut BtChardevManager {
    let ptr = core::ptr::addr_of_mut!(crate::BT_CHARDEV);
    unsafe { (*ptr).as_mut().expect("BT_CHARDEV not initialised") as *mut BtChardevManager }
}

/// Get the BT_DRIVER as a raw pointer.
unsafe fn bt_driver_ptr() -> *mut BtDriver {
    core::ptr::addr_of_mut!(crate::BT_DRIVER)
}

/// Get XHC as a raw pointer.
unsafe fn xhc_ptr() -> *mut XhciController {
    let ptr = core::ptr::addr_of_mut!(crate::XHC);
    unsafe { (*ptr).as_mut().expect("XHC not initialised") as *mut XhciController }
}

// ============================================================================
// C-callable chardev callbacks
// ============================================================================

unsafe extern "C" fn hci_open_c(minor: ffi::DevMinor, _flags: c_int, _endpt: ffi::endpoint_t) -> c_int {
    let mgr = unsafe { &mut *chardev_mgr_ptr() };
    let state = match mgr.find_by_minor_mut(minor as usize) {
        Some(s) => s,
        None => return ffi::ENXIO,
    };
    let bt_drv = unsafe { &mut *bt_driver_ptr() };
    if state.adapter_idx < bt_drv.adapters.len()
        && bt_drv.adapters[state.adapter_idx].used
        && bt_drv.adapters[state.adapter_idx].initialized
    {
        state.hci_mut().open()
    } else {
        ffi::ENXIO
    }
}

unsafe extern "C" fn hci_close_c(minor: ffi::DevMinor) -> c_int {
    let mgr = unsafe { &mut *chardev_mgr_ptr() };
    mgr.find_by_minor_mut(minor as usize)
        .map_or(ffi::ENXIO, |s| s.hci_mut().close())
}

unsafe extern "C" fn hci_read_c(
    minor: ffi::DevMinor, _seekpos: u64, endpoint: ffi::endpoint_t,
    grant: ffi::cp_grant_id_t, size: usize, _flags: c_int, _cdev: ffi::cdev_id_t
) -> isize {
    let mgr = unsafe { &mut *chardev_mgr_ptr() };
    let state_raw = match mgr.find_by_minor_mut(minor as usize) {
        Some(s) => s as *mut BtChardevState,
        None => return ffi::ENXIO as isize,
    };
    let adapter_idx = unsafe { (*state_raw).adapter_idx };
    let hci = unsafe { (*state_raw).hci_mut() as *mut chardev::HciChardev };

    let buf_size = core::cmp::min(size, 65536);
    let mut tmp_buf = vec![0u8; buf_size];
    let written = unsafe { (*hci).read(&mut tmp_buf) };
    if written <= 0 { return written; }

    // Reset idle counter on read (user consumed data)
    {
        let bt_drv = unsafe { &mut *bt_driver_ptr() };
        if adapter_idx < bt_drv.adapters.len() {
            bt_drv.adapters[adapter_idx].touch();
        }
    }

    let r = ffi::sys_safecopyto_wrapper(
        endpoint, grant, 0,
        tmp_buf.as_ptr() as *const c_void,
        written as c_ulong
    );
    if r != ffi::OK { ffi::EIO as isize } else { written }
}

unsafe extern "C" fn hci_write_c(
    minor: ffi::DevMinor, _seekpos: u64, endpoint: ffi::endpoint_t,
    grant: ffi::cp_grant_id_t, size: usize, _flags: c_int, _cdev: ffi::cdev_id_t
) -> isize {
    // Extract adapter_idx via raw ptr to avoid overlapping borrows
    let mgr = unsafe { &mut *chardev_mgr_ptr() };
    let state = match mgr.find_by_minor_mut(minor as usize) {
        Some(s) => s as *mut BtChardevState,
        None => return ffi::ENXIO as isize,
    };
    let adapter_idx = unsafe { (*state).adapter_idx };

    let bt_drv = unsafe { &mut *bt_driver_ptr() };
    let xhc = unsafe { &mut *xhc_ptr() };

    let adapter = &mut bt_drv.adapters[adapter_idx];
    let transport = match &mut adapter.transport {
        Some(t) => t,
        None => return ffi::ENXIO as isize,
    };
    let hci = unsafe { (*state).hci_mut() };

    let buf_size = core::cmp::min(size, hci::HCI_MAX_ACL_SIZE);
    let mut buf = vec![0u8; buf_size];

    let r = ffi::sys_safecopyfrom_wrapper(
        endpoint, grant, 0,
        buf.as_mut_ptr() as *mut c_void,
        buf_size as c_ulong
    );
    if r != ffi::OK { return ffi::EIO as isize; }
    if buf.is_empty() { return ffi::EAGAIN as isize; }

    let result = match hci::HciPacketType::from_byte(buf[0]) {
        Some(hci::HciPacketType::HciCommand) => {
            if transport.send_command(xhc, &buf) {
                hci.stats.cmd_tx += 1;
                hci.stats.byte_tx += buf.len() as u32;
                // Reset idle counter on outgoing commands
                adapter.touch();
                buf.len() as isize
            } else { hci.stats.err_tx += 1; ffi::EIO as isize }
        }
        Some(hci::HciPacketType::HciAclData) => {
            if transport.send_acl(xhc, &buf) {
                hci.stats.acl_tx += 1;
                hci.stats.byte_tx += buf.len() as u32;
                // Reset idle counter on outgoing ACL data
                adapter.touch();
                buf.len() as isize
            } else { hci.stats.err_tx += 1; ffi::EIO as isize }
        }
        _ => ffi::EPROTO as isize,
    };
    result
}

unsafe extern "C" fn hci_ioctl_c(
    minor: ffi::DevMinor, request: c_ulong, endpoint: ffi::endpoint_t,
    grant: ffi::cp_grant_id_t, _flags: c_int, _user_endpt: ffi::endpoint_t
) -> c_int {
    let mgr = unsafe { &mut *chardev_mgr_ptr() };
    let state = match mgr.find_by_minor_mut(minor as usize) {
        Some(s) => s as *mut BtChardevState,
        None => return ffi::ENXIO,
    };
    let adapter_idx = unsafe { (*state).adapter_idx };

    let bt_drv = unsafe { &mut *bt_driver_ptr() };
    let xhc = unsafe { &mut *xhc_ptr() };

    let adapter = &mut bt_drv.adapters[adapter_idx];
    let transport = match &mut adapter.transport {
        Some(t) => t,
        None => return ffi::ENXIO,
    };
    let hci = unsafe { (*state).hci_mut() };

    match request {
        chardev::HCIDEVUP => {
            if hci.device_up.load(Ordering::Acquire) { return ffi::OK; }
            if transport.init_sequence(xhc) {
                hci.device_up.store(true, Ordering::Release);
                ffi::print(b"bt-hci: device UP\n");
                ffi::OK
            } else { ffi::EIO }
        }
        chardev::HCIDEVDOWN => {
            transport.hci_state = hci::HciState::Down;
            hci.device_up.store(false, Ordering::Release);
            ffi::print(b"bt-hci: device DOWN\n");
            ffi::OK
        }
        chardev::HCIGETDEVINFO => {
            let info = chardev::HciDevInfo {
                dev_id: minor as u16,
                name: { let mut n = [0u8; 8]; n[..4].copy_from_slice(b"hci0"); n },
                bdaddr: transport.bd_addr.0,
                flags: if hci.device_up.load(Ordering::Acquire) { chardev::HCI_UP } else { 0 },
                dev_type: chardev::HCI_BREDR_LE as u8,
                features: { let mut f = [0u8; 8]; f[0]=0xFF;f[1]=0xFF;f[2]=0xFF;f[3]=0xFE; f },
                pkt_type: 0,
                link_policy: 0, link_mode: 0,
                acl_mtu: 1024, sco_mtu: 64,
                acl_pkts: 16, sco_pkts: 8,
                dev_stats: hci.stats.clone(),
            };
            let info_bytes = core::slice::from_raw_parts(
                &info as *const chardev::HciDevInfo as *const u8,
                core::mem::size_of::<chardev::HciDevInfo>()
            );
            ffi::sys_safecopyto_wrapper(
                endpoint, grant, 0,
                info_bytes.as_ptr() as *const c_void,
                info_bytes.len() as c_ulong
            )
        }
        chardev::HCIGETDEVLIST | chardev::HCIINQUIRY => ffi::OK,
        HCISUSPEND => {
            if adapter.suspended {
                return ffi::OK; // Already suspended
            }
            if crate::usb_bt::suspend_adapter(xhc, adapter_idx) {
                hci.device_up.store(false, Ordering::Release);
                ffi::print(b"bt-hci: device suspended\n");
                ffi::OK
            } else {
                ffi::EIO
            }
        }
        HCIRESUME => {
            if !adapter.suspended {
                return ffi::OK; // Already running
            }
            // Clear the power-down ops before trying to resume
            adapter.suspended = false;
            if crate::usb_bt::resume_adapter(xhc, adapter_idx) {
                hci.device_up.store(true, Ordering::Release);
                ffi::print(b"bt-hci: device resumed\n");
                ffi::OK
            } else {
                adapter.suspended = true; // Resume failed — keep suspended
                ffi::EIO
            }
        }
        _ => ffi::ENOTTY,
    }
}

// ============================================================================
// Public API
// ============================================================================

pub fn as_chardriver() -> ffi::Chardriver {
    ffi::Chardriver {
        cdr_type: -1,
        cdr_open: Some(hci_open_c),
        cdr_close: Some(hci_close_c),
        cdr_read: Some(hci_read_c),
        cdr_write: Some(hci_write_c),
        cdr_ioctl: Some(hci_ioctl_c),
        cdr_select: None,
        cdr_intr: None,
        cdr_alarm: None,
        cdr_other: None,
        cdr_device: None,
        cdr_signal: None,
    }
}
