//! # xHCI (USB 3.0) Driver for MINIX — Native Rust Implementation
//!
//! Implements: PCI probe, xHCI controller init, device enumeration,
//! TRB ring management, USB control/bulk/interrupt transfers.
//!
//! ## Architecture
//!
//! ```text
//! lib.rs (main entry, SEF lifecycle, blockdriver integration)
//!   ├── ffi.rs (MINIX C FFI bindings)
//!   ├── registers.rs (xHCI register set, TRB structs, Device Context)
//!   ├── xhci.rs (controller init/stop, port mgmt, command ring ops)
//!   ├── ring.rs (TRB ring management — cmd ring, event ring, transfer rings)
//!   └── devmgr.rs (device slot mgmt, address device, configure endpoint)
//! ```

#![allow(dead_code)]

mod bt_chardev;
mod ffi;
mod hid_chardev;
mod registers;
mod ring;
mod usb_bt;
mod usb_device;
mod usb_hid;
mod usb_hub;
mod usb_interface;
mod usb_msc;
mod xhci;

use core::ffi::{c_int, c_uint, c_ulong};

use usb_bt::BtDriver;
use usb_device::UsbClassDriver;
use usb_hid::HidDriver;
use usb_hub::UsbHubDriver;
use usb_msc::MscDevice;
use xhci::XhciController;

/// Global xHCI controller instance (static for C callback access).
static mut XHC: Option<XhciController> = None;

/// Global HUB driver instance.
pub static mut HUB_DRIVER: UsbHubDriver = UsbHubDriver::new_static();

/// Global HID driver instance.
pub static mut HID_DRIVER: HidDriver = HidDriver::new_static();

/// Global Bluetooth driver instance.
pub static mut BT_DRIVER: BtDriver = BtDriver::new_static();

/// Global Bluetooth chardev manager.
pub static mut BT_CHARDEV: Option<bt_chardev::BtChardevManager> = None;

/// Global HID chardev manager.
pub static mut HID_CHARDEV: Option<hid_chardev::HidChardevManager> = None;

/// Verbosity level (from env).
static mut VERBOSE: u8 = 0;

// ============================================================================
// Blockdriver Callbacks
// ============================================================================

unsafe extern "C" fn xhci_open(minor: ffi::DevMinor, _access: c_int) -> c_int {
    // Check if minor corresponds to an MSC device
    if minor >= 0 && (minor as usize) < usb_msc::MAX_MSC_DEVICES {
        if let Some(xhc) = unsafe { &mut *core::ptr::addr_of_mut!(XHC) } {
            if (minor as usize) < usb_msc::MAX_MSC_DEVICES && xhc.msc_devices[minor as usize].ready {
                return ffi::OK;
            }
        }
    }
    if minor == 0 {
        ffi::OK
    } else {
        ffi::ENXIO
    }
}

unsafe extern "C" fn xhci_close(_minor: ffi::DevMinor) -> c_int {
    ffi::OK
}

unsafe extern "C" fn xhci_transfer(
    minor: ffi::DevMinor,
    write: c_int,
    pos: u64,
    proc_endpt: c_int,
    grant_ptr: *mut core::ffi::c_void,
    bytes: c_uint,
    _flags: c_int,
) -> isize {
    // Route to USB Mass Storage if minor is an MSC device
    if minor >= 0 && (minor as usize) < usb_msc::MAX_MSC_DEVICES {
        if let Some(xhc) = unsafe { &mut *core::ptr::addr_of_mut!(XHC) } {
            let dev_idx = minor as usize;
            if dev_idx < usb_msc::MAX_MSC_DEVICES && xhc.msc_devices[dev_idx].ready {
                let block_size = xhc.msc_devices[dev_idx].block_size;
                let count = (bytes as usize + block_size as usize - 1) / block_size as usize;
                let lba = pos / block_size as u64;
                let transfer_len = (count as u32) * block_size;
                let grant = grant_ptr as ffi::cp_grant_id_t;

                // Bounds check: pre-allocated DMA buffer is 256KB
                if transfer_len > 262144 {
                    return ffi::EINVAL as isize;
                }

                // Use pre-allocated DMA bounce buffer per device
                let (dma_virt, dma_phys) = match &xhc.msc_devices[dev_idx].data_buf {
                    Some(buf) => (buf.virt, buf.phys),
                    None => return ffi::ENOMEM as isize,
                };

                if write != 0 {
                    // WRITE: copy data FROM userspace TO DMA buffer via safecopy
                    let r = ffi::sys_safecopyfrom_wrapper(
                        proc_endpt, grant, 0,
                        dma_virt as *mut core::ffi::c_void,
                        transfer_len as core::ffi::c_ulong,
                    );
                    if r != ffi::OK {
                        return ffi::EIO as isize;
                    }

                    // Issue SCSI WRITE10 via BOT
                    let n = usb_msc::msc_write(xhc, dev_idx, lba, count, dma_phys);
                    if n < 0 { ffi::EIO as isize } else { n }
                } else {
                    // READ: issue SCSI READ10 via BOT first
                    let n = usb_msc::msc_read(xhc, dev_idx, lba, count, dma_phys);
                    if n < 0 { return ffi::EIO as isize; }

                    // Copy data FROM DMA buffer TO userspace via safecopy
                    let r = ffi::sys_safecopyto_wrapper(
                        proc_endpt, grant, 0,
                        dma_virt as *const core::ffi::c_void,
                        transfer_len as core::ffi::c_ulong,
                    );
                    if r != ffi::OK {
                        return ffi::EIO as isize;
                    }

                    n
                }
            } else {
                ffi::ENXIO as isize
            }
        } else {
            ffi::ENXIO as isize
        }
    } else {
        ffi::ENXIO as isize
    }
}

unsafe extern "C" fn xhci_ioctl(
    minor: ffi::DevMinor,
    request: core::ffi::c_ulong,
    _endpt: c_int,
    _grant: c_int,
    _endpt2: c_int,
) -> c_int {
    match (minor, request) {
        (0, ..) => ffi::OK,
        _ => ffi::ENOTTY,
    }
}

unsafe extern "C" fn xhci_part(_minor: ffi::DevMinor) -> *mut core::ffi::c_void {
    core::ptr::null_mut()
}

unsafe extern "C" fn xhci_intr(_mask: c_uint) {
    // Handle interrupts from xHC — interrupt-driven event dispatch
    if let Some(xhc) = unsafe { &mut *core::ptr::addr_of_mut!(XHC) } {
        // Process all available events
        while let Some(event) = xhc.event_ring.next_event() {
            match event.trb_type() {
                Some(crate::registers::TrbType::TransferEvent) => {
                    // Transfer completed — extract completion code and signal waiter
                    let cc_val = (event.status[0] as u16) | ((event.status[1] as u16) << 8);
                    let cc = (cc_val & 0xFF) as u8;
                    xhc.tx_completed = true;
                    xhc.tx_cc = cc;
                }
                Some(crate::registers::TrbType::CommandCompletionEvent) => {
                    // Command completed — extract completion code
                    let cc_val = (event.status[0] as u16) | ((event.status[1] as u16) << 8);
                    let cc = (cc_val & 0xFF) as u8;
                    xhc.cmd_completed = true;
                    xhc.cmd_cc = cc;
                }
                Some(crate::registers::TrbType::PortStatusChangeEvent) => {
                    // Port status change — acknowledge
                    let port_id = event.flags[3];
                    if port_id > 0 && (port_id as u8) <= xhc.max_ports {
                        let sc = xhc.portsc(port_id as u8);
                        xhc.set_portsc(port_id as u8, sc | crate::registers::op::portsc::CSC);
                    }
                }
                _ => {
                    // Unknown event type — ignore
                }
            }
        }

        // Update ERDP once to acknowledge all processed events
        let erdp = xhc.event_ring.dequeue_phys() | crate::registers::rt::erdp::EHB;
        xhc.rt_w64(0, crate::registers::rt::ERDP, erdp);

        // Note: no IRQ re-enable needed — IRQ thread framework
        // handles edge-triggered or level-triggered IRQ re-enable
        // automatically via sys_irqenable during IRQ thread setup.
    }
}

unsafe extern "C" fn xhci_alarm(_stamp: u64) {
    // Periodic tick — scan for port changes
    if let Some(xhc) = unsafe { &mut *core::ptr::addr_of_mut!(XHC) } {
        for port in 1..=xhc.max_ports {
            if xhc.port_connected(port) {
                let speed = xhc.port_speed(port);
                if speed > 0 && xhc.port_link_state(port) == 0 {
                    let sc = xhc.portsc(port);
                    if (sc & crate::registers::op::portsc::CSC) != 0 {
                        xhc.set_portsc(port, sc | crate::registers::op::portsc::CSC);

                        // Allocate a buffer for full enumeration
                        let mut enum_buf = match crate::ring::RingMem::alloc(512) {
                            Some(b) => b,
                            None => continue,
                        };

                        // Full enumeration with descriptor reading + class dispatch
                        xhc.enumerate_port_full(port, speed,
                            enum_buf.virt, enum_buf.phys, enum_buf.size);

                        // Fallback: Try legacy MSC probe for devices already
                        // enumerated but not claimed by a class driver
                        let slot_id = xhc.slots.iter().position(|s| {
                            s.assigned && s.port == port && !xhc.msc_devices.iter().any(|d| d.slot_id == s.id)
                        }).map(|i| i as u8);

                        if let Some(sid) = slot_id {
                            if sid > 0 {
                                // Check if already handled by device framework
                                if xhc.device_registry.find_by_slot(sid).is_none() {
                                    let result = usb_msc::probe_msc_device(
                                        xhc, sid, enum_buf.virt, enum_buf.phys, 512
                                    );
                                    if result.is_some() {
                                        ffi::print(b"xHCI: USB Mass Storage detected\0");
                                    }
                                }
                            }
                        }

                        enum_buf.free();
                    }
                }
            }
        }

        // Poll BT adapters for incoming HCI events (every tick)
        {
            let bt_drv = unsafe { &mut *core::ptr::addr_of_mut!(crate::BT_DRIVER) };
            let chardev_mgr = unsafe {
                let ptr = core::ptr::addr_of_mut!(crate::BT_CHARDEV);
                match (*ptr).as_mut() {
                    Some(m) => m,
                    None => return,
                }
            };

            // Auto-allocate chardev slots for BT adapters that don't have one
            for i in 0..bt_drv.adapters.len() {
                if bt_drv.adapters[i].used && bt_drv.adapters[i].initialized {
                    if chardev_mgr.find_by_adapter(i).is_none() {
                        chardev_mgr.alloc(i);
                        ffi::print(b"bt-hci: chardev registered\0");
                    }
                }
            }

            // Remote wakeup check: if a suspended adapter's port returned to U0,
            // it means the device woke itself up — reinitialise it
            for i in 0..bt_drv.adapters.len() {
                let adapter = &mut bt_drv.adapters[i];
                if !adapter.used || !adapter.initialized || !adapter.suspended { continue; }
                let port = match &adapter.transport {
                    Some(t) => xhc.slot_port(t.slot_id),
                    None => continue,
                };
                if port > 0 && xhc.port_link_state(port) == crate::registers::op::portsc::PLS_U0 {
                    // Device woke itself up (remote wakeup) — reinit
                    if xhc.verbose >= 1 {
                        crate::ffi::print(b"xHCI-BT: remote wakeup\0");
                    }
                    adapter.suspended = false;
                    if let Some(ref mut t) = adapter.transport {
                        let _ = t.resume(xhc);
                    }
                }
            }

            bt_chardev::poll_bt_events(bt_drv, xhc, chardev_mgr);

            // Auto-suspend idle BT adapters (no activity for N ticks)
            for i in 0..bt_drv.adapters.len() {
                let adapter = &mut bt_drv.adapters[i];
                if !adapter.used || !adapter.initialized || adapter.suspended { continue; }
                adapter.idle_counter += 1;
                if adapter.idle_counter >= adapter.suspend_idle_threshold {
                    let _ = crate::usb_bt::suspend_adapter(xhc, i);
                }
            }
        }

        // Poll HID devices (keyboards, mice) for input reports
        {
            let hid_drv = unsafe { &mut *core::ptr::addr_of_mut!(crate::HID_DRIVER) };
            let hid_chardev_mgr = unsafe {
                let ptr = core::ptr::addr_of_mut!(crate::HID_CHARDEV);
                match (*ptr).as_mut() {
                    Some(m) => m,
                    None => return,
                }
            };

            // Auto-allocate chardev slots for HID devices that don't have one
            for i in 0..hid_drv.num_devices {
                if hid_chardev_mgr.find_by_device(i, hid_drv.devices[i].kind).is_none() {
                    hid_chardev_mgr.alloc(hid_drv.devices[i].kind, i);
                    if xhc.verbose >= 1 {
                        crate::ffi::print(b"xHCI: HID chardev registered\0");
                    }
                }
            }

            let n = hid_drv.num_devices;
            for i in 0..n {
                let dev = &mut hid_drv.devices[i];
                let old_mods = dev.keyboard.modifiers;
                let old_keys = dev.keyboard.keys;
                let old_key_count = dev.keyboard.key_count;
                let old_mouse = dev.mouse;

                // Poll interrupt endpoint for new report
                let received = crate::usb_hid::HidDriver::poll_interrupt(dev, xhc);

                if !received {
                    continue;
                }

                match dev.kind {
                    crate::usb_hid::HidDeviceKind::Keyboard => {
                        let changed = old_mods != dev.keyboard.modifiers
                            || old_key_count != dev.keyboard.key_count
                            || old_keys != dev.keyboard.keys;
                        if changed {
                            hid_chardev_mgr.push_keyboard(i, &dev.keyboard);
                            #[cfg(target_os = "minix")]
                            if xhc.verbose >= 2 {
                                crate::ffi::print(b"xHCI: kbd state\0");
                            }
                        }
                    }
                    crate::usb_hid::HidDeviceKind::Mouse => {
                        let changed = old_mouse.buttons != dev.mouse.buttons
                            || dev.mouse.has_moved;
                        if changed {
                            hid_chardev_mgr.push_mouse(i, &dev.mouse);
                            #[cfg(target_os = "minix")]
                            if xhc.verbose >= 2 {
                                crate::ffi::print(b"xHCI: ms state\0");
                            }
                        }
                    }
                    crate::usb_hid::HidDeviceKind::Gamepad => {
                        if dev.gamepad.has_changed {
                            hid_chardev_mgr.push_gamepad(i, &dev.gamepad);
                            #[cfg(target_os = "minix")]
                            if xhc.verbose >= 2 {
                                // Log compact gamepad state
                                let r0 = dev.gamepad.buttons as u8;
                                let r1 = (dev.gamepad.buttons >> 8) as u8;
                                crate::ffi::print(b"xHCI: gp\0");
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn hex_char(v: u8) -> u8 {
        let nib = v & 0x0F;
        if nib < 10 { b'0' + nib } else { b'A' + nib - 10 }
    }
}

unsafe extern "C" fn xhci_device(minor: ffi::DevMinor, dev_id: *mut c_int) -> c_int {
    if minor == 0 {
        *dev_id = 0;
        ffi::OK
    } else {
        ffi::ENXIO
    }
}

// ============================================================================
// Combined BDEV + CDEV Message Loop
// ============================================================================

/// Combined driver message loop — dispatches BDEV (MSC), CDEV (BT),
/// and NOTIFY (alarm/interrupt) messages from a single loop.
///
/// Message dispatch (x86_64 LP64 offsets; other archs via cfg):
///
/// | Message Type       | Struct | Fields extracted                                      |
/// |--------------------|--------|------------------------------------------------------|
/// | BDEV_OPEN   (0x100)| mess_2 | minor=m2_i1@8(int), access=m2_i2@12(int)             |
/// | BDEV_CLOSE  (0x101)| mess_2 | minor=m2_i1@8(int)                                   |
/// | BDEV_READ   (0x102)| mess_2 | minor@8, pos=m2_l1@16(long), endpt@24, grant@28,     |
/// | BDEV_WRITE  (0x103)| mess_2 |   size=m2_l2@32(long), flags=m2_i5@40(int)           |
/// | BDEV_IOCTL  (0x106)| mess_2 | minor@8, request=m2_l1@16(long), endpt@24, grant@28  |
/// | CDEV_OPEN   (0x200)| mess_1 | minor=m1_i1@8(int), flags=m1_i2@12(int),             |
/// |                  |         |   endpt=m1_i3@16(int)                                |
/// | CDEV_CLOSE  (0x201)| mess_1 | minor=m1_i1@8(int)                                   |
/// | CDEV_READ   (0x202)| mess_4 | minor=m4_l1@8(long), seekpos=m4_l2@16(long),         |
/// | CDEV_WRITE  (0x203)| mess_4 |   endpt=m4_l3@24(long), grant=m4_l4@32(long),        |
/// | CDEV_IOCTL  (0x204)| mess_4 |   size=m4_l5@40(long) (READ/WRITE)                   |
/// |                  |         | minor@8, request=m4_l2@16(long), endpt@24, grant@32  |
/// | NOTIFY_MESSAGE    | status  | HARDWARE(0)→xhci_intr, CLOCK(1)→xhci_alarm           |
fn combined_driver_task(bd: &ffi::Blockdriver, cd: &ffi::Chardriver) {
    // On host stubs (non-MINIX), return immediately
    #[cfg(not(target_os = "minix"))]
    {
        let _ = bd;
        let _ = cd;
        return;
    }

    #[cfg(target_os = "minix")]
    loop {
        let (mut msg, status) = ffi::receive_status();
        let mtype = msg.m_type();
        let source = msg.m_source();

        let result = match mtype {
            // ── Block device messages ────────────────────────────────────────
            ffi::BDEV_OPEN => {
                let minor = msg.m2_i1();
                let access = msg.m2_i2();
                match bd.bdr_open {
                    Some(cb) => unsafe { cb(minor, access) },
                    None => ffi::ENXIO,
                }
            }
            ffi::BDEV_CLOSE => {
                let minor = msg.m2_i1();
                match bd.bdr_close {
                    Some(cb) => unsafe { cb(minor) },
                    None => ffi::ENXIO,
                }
            }
            ffi::BDEV_READ | ffi::BDEV_WRITE => {
                let minor = msg.m2_i1();
                let write = if mtype == ffi::BDEV_WRITE { 1 } else { 0 };
                let pos = msg.m2_l1() as u64;
                let endpoint = msg.m2_i3();
                let grant = msg.m2_i4();
                let size = msg.m2_l2() as u64;
                let flags = msg.m2_i5();
                match bd.bdr_transfer {
                    Some(cb) => unsafe {
                        cb(minor, write, pos, endpoint,
                            grant as *mut core::ffi::c_void,
                            size as c_uint, flags) as c_int
                    },
                    None => ffi::ENXIO,
                }
            }
            ffi::BDEV_IOCTL => {
                let minor = msg.m2_i1();
                let request = msg.m2_l1() as core::ffi::c_ulong;
                let endpoint = msg.m2_i3();
                let grant = msg.m2_i4();
                match bd.bdr_ioctl {
                    Some(cb) => unsafe { cb(minor, request, endpoint, grant, 0) },
                    None => ffi::ENXIO,
                }
            }

            // ── Character device messages ───────────────────────────────────
            ffi::CDEV_OPEN => {
                let minor = msg.m1_i1();
                let flags = msg.m1_i2();
                let endpoint = msg.m1_i3();
                match cd.cdr_open {
                    Some(cb) => unsafe { cb(minor, flags, endpoint) },
                    None => ffi::ENXIO,
                }
            }
            ffi::CDEV_CLOSE => {
                let minor = msg.m1_i1();
                match cd.cdr_close {
                    Some(cb) => unsafe { cb(minor) },
                    None => ffi::ENXIO,
                }
            }
            ffi::CDEV_READ => {
                let minor = msg.m4_l1() as ffi::DevMinor;
                let seekpos = msg.m4_l2() as u64;
                let endpoint = msg.m4_l3() as ffi::endpoint_t;
                let grant = msg.m4_l4() as ffi::cp_grant_id_t;
                let size = msg.m4_l5() as usize;
                match cd.cdr_read {
                    Some(cb) => unsafe {
                        cb(minor, seekpos, endpoint, grant, size, 0, 0) as c_int
                    },
                    None => ffi::ENXIO,
                }
            }
            ffi::CDEV_WRITE => {
                let minor = msg.m4_l1() as ffi::DevMinor;
                let seekpos = msg.m4_l2() as u64;
                let endpoint = msg.m4_l3() as ffi::endpoint_t;
                let grant = msg.m4_l4() as ffi::cp_grant_id_t;
                let size = msg.m4_l5() as usize;
                match cd.cdr_write {
                    Some(cb) => unsafe {
                        cb(minor, seekpos, endpoint, grant, size, 0, 0) as c_int
                    },
                    None => ffi::ENXIO,
                }
            }
            ffi::CDEV_IOCTL => {
                let minor = msg.m4_l1() as ffi::DevMinor;
                let request = msg.m4_l2() as core::ffi::c_ulong;
                let endpoint = msg.m4_l3() as ffi::endpoint_t;
                let grant = msg.m4_l4() as ffi::cp_grant_id_t;
                match cd.cdr_ioctl {
                    Some(cb) => unsafe { cb(minor, request, endpoint, grant, 0, 0) },
                    None => ffi::ENXIO,
                }
            }

            // ── Notifications (alarm / interrupt) ───────────────────────────
            ffi::NOTIFY_MESSAGE => {
                match status {
                    ffi::HARDWARE => {
                        if let Some(cb) = bd.bdr_intr {
                            unsafe { cb(0) };
                        }
                        ffi::OK
                    }
                    ffi::CLOCK => {
                        if let Some(cb) = bd.bdr_alarm {
                            unsafe { cb(0) };
                        }
                        ffi::OK
                    }
                    _ => ffi::ENXIO,
                }
            }

            _ => ffi::ENXIO,
        };

        // Send reply (set m_type to result, send to caller)
        // NOTIFY_MESSAGE (alarm/intr) has no caller to reply to
        if mtype != ffi::NOTIFY_MESSAGE {
            msg.set_result(result);
            let _ = ffi::reply(source, &msg);
        }
    }
}

// ============================================================================
// SEF Lifecycle
// ============================================================================

unsafe extern "C" fn sef_init_fresh(_type: c_int, _info: *const core::ffi::c_void) -> c_int {
    ffi::print(b"xHCI: initializing...\0");

    let skip = ffi::env_parse_long(b"instance\0", 0, 0, 10);
    let verbose = ffi::env_parse_long(b"verbose\0", 0, 0, 3) as u8;
    unsafe { VERBOSE = verbose; }

    // Parse device instance from env
    let devind = match XhciController::probe(skip as c_int) {
        Some(d) => d,
        None => {
            ffi::print(b"xHCI: no controller found\0");
            return ffi::EIO;
        }
    };

    let mut xhc = match XhciController::init(devind, verbose) {
        Some(x) => x,
        None => {
            ffi::print(b"xHCI: init failed\0");
            return ffi::EIO;
        }
    };

    // Initialize ports
    xhc.init_ports();

    // Register class drivers
    // The Hub driver for downstream port management
    {
        let xhc = &mut xhc;
        let hub_verbose = verbose;
        let hub_drv: &'static mut crate::usb_hub::UsbHubDriver =
            unsafe { &mut *core::ptr::addr_of_mut!(crate::HUB_DRIVER) };
        if hub_drv.class_code() == crate::registers::usb_class::HUB {
            xhc.device_registry.register_driver(hub_drv);
        }
    }
    // The HID driver for keyboards and mice
    {
        let xhc = &mut xhc;
        let hid_drv: &'static mut crate::usb_hid::HidDriver =
            unsafe { &mut *core::ptr::addr_of_mut!(crate::HID_DRIVER) };
        if hid_drv.class_code() == crate::registers::usb_class::HID {
            xhc.device_registry.register_driver(hid_drv);
        }
    }

    // The Bluetooth HCI driver
    {
        let xhc = &mut xhc;
        let bt_drv: &'static mut crate::usb_bt::BtDriver =
            unsafe { &mut *core::ptr::addr_of_mut!(crate::BT_DRIVER) };
        // Set verbosity from env
        bt_drv.verbose = verbose;
        xhc.device_registry.register_driver(bt_drv);
    }

    // Initialise BT chardev manager
    {
        unsafe {
            crate::BT_CHARDEV = Some(bt_chardev::BtChardevManager::new());
        }
    }

    // Initialise HID chardev manager
    {
        unsafe {
            crate::HID_CHARDEV = Some(hid_chardev::HidChardevManager::new());
        }
    }

    // Store as global
    unsafe { XHC = Some(xhc); }

    // Announce to blockdriver framework
    ffi::blockdriver_support_lu();
    ffi::blockdriver_announce_ffi(0);

    ffi::print(b"xHCI: ready\0");
    ffi::OK
}

unsafe extern "C" fn sef_signal_handler(signo: c_int) {
    if signo == 0x1001 {
        ffi::print(b"xHCI: shutting down...\0");
        if let Some(xhc) = unsafe { &mut *core::ptr::addr_of_mut!(XHC) } {
            xhc.stop();
        }
        ffi::blockdriver_terminate();
    }
}

// ============================================================================
// Entry Point
// ============================================================================

/// xHCI driver entry point (called from C main).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xhci_rust_main(argc: c_int, argv: *mut *mut core::ffi::c_char) {
    ffi::sef_set_init_fresh(sef_init_fresh);
    ffi::sef_set_signal_handler(sef_signal_handler);
    ffi::sef_startup_ffi();

    // Blockdriver table — MSC storage devices
    let bd = ffi::Blockdriver {
        bdr_type: 0,
        bdr_open: Some(xhci_open),
        bdr_close: Some(xhci_close),
        bdr_transfer: Some(xhci_transfer),
        bdr_ioctl: Some(xhci_ioctl),
        bdr_part: Some(xhci_part),
        bdr_intr: Some(xhci_intr),
        bdr_alarm: Some(xhci_alarm),
        bdr_device: Some(xhci_device),
    };

    // Build merged chardriver: BT (minor 0-3) + HID (minor 4+)
    // We need a merged table because VFS only accepts one chardev per driver
    let cd = merged_chardriver();

    // Announce the chardriver to VFS
    ffi::chardriver_announce(-1);

    // Combined message loop: handles both BDEV (MSC) and CDEV (BT/HID) messages
    combined_driver_task(&bd, &cd);
}

// ============================================================================
// Merged Chardriver — dispatches to BT (minor 0-3) or HID (minor 4+) by minor
// ============================================================================

const BT_MINOR_END: ffi::DevMinor = 4; // BT uses minors 0..3

unsafe extern "C" fn merged_open_c(minor: ffi::DevMinor, flags: c_int, endpt: ffi::endpoint_t) -> c_int {
    if minor < BT_MINOR_END {
        let bt = bt_chardev::as_chardriver();
        match bt.cdr_open {
            Some(cb) => cb(minor, flags, endpt),
            None => ffi::ENXIO,
        }
    } else {
        let hid = hid_chardev::as_chardriver();
        match hid.cdr_open {
            Some(cb) => cb(minor, flags, endpt),
            None => ffi::ENXIO,
        }
    }
}

unsafe extern "C" fn merged_close_c(minor: ffi::DevMinor) -> c_int {
    if minor < BT_MINOR_END {
        let bt = bt_chardev::as_chardriver();
        match bt.cdr_close {
            Some(cb) => cb(minor),
            None => ffi::ENXIO,
        }
    } else {
        let hid = hid_chardev::as_chardriver();
        match hid.cdr_close {
            Some(cb) => cb(minor),
            None => ffi::ENXIO,
        }
    }
}

unsafe extern "C" fn merged_read_c(
    minor: ffi::DevMinor, seekpos: u64, endpoint: ffi::endpoint_t,
    grant: ffi::cp_grant_id_t, size: usize, flags: c_int, cdev: ffi::cdev_id_t
) -> isize {
    if minor < BT_MINOR_END {
        let bt = bt_chardev::as_chardriver();
        match bt.cdr_read {
            Some(cb) => cb(minor, seekpos, endpoint, grant, size, flags, cdev),
            None => ffi::ENXIO as isize,
        }
    } else {
        let hid = hid_chardev::as_chardriver();
        match hid.cdr_read {
            Some(cb) => cb(minor, seekpos, endpoint, grant, size, flags, cdev),
            None => ffi::ENXIO as isize,
        }
    }
}

unsafe extern "C" fn merged_write_c(
    minor: ffi::DevMinor, seekpos: u64, endpoint: ffi::endpoint_t,
    grant: ffi::cp_grant_id_t, size: usize, flags: c_int, cdev: ffi::cdev_id_t
) -> isize {
    if minor < BT_MINOR_END {
        let bt = bt_chardev::as_chardriver();
        match bt.cdr_write {
            Some(cb) => cb(minor, seekpos, endpoint, grant, size, flags, cdev),
            None => ffi::ENXIO as isize,
        }
    } else {
        let hid = hid_chardev::as_chardriver();
        match hid.cdr_write {
            Some(cb) => cb(minor, seekpos, endpoint, grant, size, flags, cdev),
            None => ffi::ENXIO as isize,
        }
    }
}

unsafe extern "C" fn merged_ioctl_c(
    minor: ffi::DevMinor, request: c_ulong, endpoint: ffi::endpoint_t,
    grant: ffi::cp_grant_id_t, flags: c_int, user_endpt: ffi::endpoint_t
) -> c_int {
    if minor < BT_MINOR_END {
        let bt = bt_chardev::as_chardriver();
        match bt.cdr_ioctl {
            Some(cb) => cb(minor, request, endpoint, grant, flags, user_endpt),
            None => ffi::ENXIO,
        }
    } else {
        let hid = hid_chardev::as_chardriver();
        match hid.cdr_ioctl {
            Some(cb) => cb(minor, request, endpoint, grant, flags, user_endpt),
            None => ffi::ENXIO,
        }
    }
}

fn merged_chardriver() -> ffi::Chardriver {
    ffi::Chardriver {
        cdr_type: -1,
        cdr_open: Some(merged_open_c),
        cdr_close: Some(merged_close_c),
        cdr_read: Some(merged_read_c),
        cdr_write: Some(merged_write_c),
        cdr_ioctl: Some(merged_ioctl_c),
        cdr_select: None,
        cdr_intr: None,
        cdr_alarm: None,
        cdr_other: None,
        cdr_device: None,
        cdr_signal: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xhci_crate_exists() {
        // Basic compile check
        assert_eq!(core::mem::size_of::<ffi::Blockdriver>(), 8 * core::mem::size_of::<usize>());
    }
}
