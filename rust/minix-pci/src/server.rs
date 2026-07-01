//! # PCI Server — IPC Message Handlers
//!
//! Handles BUSC_PCI messages from other drivers:
//!   - BUSC_PCI_INIT: initialize a new driver instance
//!   - BUSC_PCI_FIRST_DEV / NEXT_DEV: device enumeration
//!   - BUSC_PCI_ATTR_R8/R16/R32: config space read
//!   - BUSC_PCI_ATTR_W8/W16/W32: config space write
//!   - BUSC_PCI_GET_BAR: BAR info by register number
//!   - BUSC_PCI_RESERVE: device reservation
//!   - BUSC_PCI_FIND_DEV: find by BDF
//!   - BUSC_PCI_IDS: get vendor/device IDs
//!   - BUSC_PCI_RESCAN: rescan a bus
//!
//! NOTE: The MINIX `message` type is a C union — all field views (m1, m2, etc.)
//! overlap at the same offset after m_type + m_source (offset 8). We model this
//! with a Rust union + wrapper functions to access fields by view.

use crate::ffi;
use crate::devices::PciDeviceTable;
use crate::devices::PciDevice;

use core::ffi::{c_int, c_uint, c_ulong, c_void};

/// Helper: return a negative errno value (MINIX convention).
#[inline]
fn libc_errno(e: c_int) -> c_int { e }

// ============================================================================
// MINIX message layout (C union compatible)
// ============================================================================

/// Message header — every MINIX message starts with these.
#[repr(C)]
#[derive(Clone, Copy)]
struct MsgHdr {
    m_type: c_int,
    m_source: c_int,
}

/// m1 view — used by FIRST_DEV, NEXT_DEV, FIND_DEV, IDS, RESERVE.
#[repr(C)]
#[derive(Clone, Copy)]
struct MsgM1 {
    i1: c_int,
    i2: c_int,
    i3: c_int,
    i4: c_int,
}

/// m2 view — used by ATTR_R8/R16/R32, ATTR_W8/W16/W32, RESCAN.
#[repr(C)]
#[derive(Clone, Copy)]
struct MsgM2 {
    i1: c_int,
    i2: c_int,
    i3: c_int,
    l1: c_ulong,
    l2: c_ulong,
}

/// m7 view — used by DEV_NAME_S.
#[repr(C)]
#[derive(Clone, Copy)]
struct MsgM7 {
    i1: c_int,  // vid
    i2: c_int,  // did
    i3: c_int,  // name_len
    i4: c_int,  // name_gid (grant ID)
}

/// BAR request — for BUSC_PCI_GET_BAR.
#[repr(C)]
#[derive(Clone, Copy)]
struct BarReq {
    devind: c_int,
    port: c_int,
}

/// BAR response — for BUSC_PCI_GET_BAR reply.
#[repr(C)]
#[derive(Clone, Copy)]
struct BarResp {
    base: u32,
    size: u32,
    flags: c_int,
}

/// Full MINIX message union — models the C `message` type.
///
/// All body variants overlap at offset 8 (after header).
/// Size must be at least 56 bytes to match MINIX message size.
#[repr(C)]
union MsgBody {
    m1: MsgM1,
    m2: MsgM2,
    m7: MsgM7,
    bar_req: BarReq,
    bar_resp: BarResp,
}

/// Complete message — header + overlapping body.
#[repr(C)]
struct PciMessage {
    header: MsgHdr,
    body: MsgBody,
}

/// Access helpers — read/write message fields through the union.
///
/// These ensure safe access by reading/writing through the correct view.
impl PciMessage {
    fn m_type(&self) -> c_int { unsafe { self.header.m_type } }
    fn set_m_type(&mut self, val: c_int) { self.header.m_type = val; }
    fn m_source(&self) -> c_int { unsafe { self.header.m_source } }

    // m1 view
    fn m1_i1(&self) -> c_int { unsafe { self.body.m1.i1 } }
    fn set_m1_i1(&mut self, val: c_int) { unsafe { self.body.m1.i1 = val; } }
    fn m1_i2(&self) -> c_int { unsafe { self.body.m1.i2 } }
    fn set_m1_i2(&mut self, val: c_int) { unsafe { self.body.m1.i2 = val; } }
    fn m1_i3(&self) -> c_int { unsafe { self.body.m1.i3 } }
    fn set_m1_i3(&mut self, val: c_int) { unsafe { self.body.m1.i3 = val; } }

    // m2 view
    fn m2_i1(&self) -> c_int { unsafe { self.body.m2.i1 } }
    fn set_m2_i1(&mut self, val: c_int) { unsafe { self.body.m2.i1 = val; } }
    fn m2_i2(&self) -> c_int { unsafe { self.body.m2.i2 } }
    fn set_m2_i2(&mut self, val: c_int) { unsafe { self.body.m2.i2 = val; } }
    fn m2_l1(&self) -> c_ulong { unsafe { self.body.m2.l1 } }
    fn set_m2_l1(&mut self, val: c_ulong) { unsafe { self.body.m2.l1 = val; } }

    // m7 view (DEV_NAME_S)
    fn m7_i1(&self) -> c_int { unsafe { self.body.m7.i1 } }
    fn m7_i2(&self) -> c_int { unsafe { self.body.m7.i2 } }
    fn m7_i3(&self) -> c_int { unsafe { self.body.m7.i3 } }
    fn m7_i4(&self) -> c_int { unsafe { self.body.m7.i4 } }

    // BAR request/response view
    fn bar_devind(&self) -> c_int { unsafe { self.body.bar_req.devind } }
    fn bar_port(&self) -> c_int { unsafe { self.body.bar_req.port } }
    fn set_bar_base(&mut self, val: u32) { unsafe { self.body.bar_resp.base = val; } }
    fn set_bar_size(&mut self, val: u32) { unsafe { self.body.bar_resp.size = val; } }
    fn set_bar_flags(&mut self, val: c_int) { unsafe { self.body.bar_resp.flags = val; } }
}

// ============================================================================
// ACL table — controls which drivers can see which PCI devices
// ============================================================================

const NR_DRIVERS: usize = 64;

static mut ACL_INUSE: [c_int; NR_DRIVERS] = [0; NR_DRIVERS];
static mut ACL_ENDPOINT: [c_int; NR_DRIVERS] = [0; NR_DRIVERS];

/// Find ACL entry by endpoint (returns index or None).
fn find_acl(endpoint: c_int) -> Option<usize> {
    unsafe {
        for i in 0..NR_DRIVERS {
            if ACL_INUSE[i] != 0 && ACL_ENDPOINT[i] == endpoint {
                return Some(i);
            }
        }
    }
    None
}

/// Check whether `endpoint` is allowed to see a given device.
///
/// Matches the C `visible()` function from `pci.c`:
///   - If no ACL entry exists for `endpoint` → device is visible (all drivers
///     without explicit ACL can see all devices).
///   - If ACL exists → filtered by device/class rules (TODO: implement full
///     rs_pci device/class matching when the grant data is fully stored).
fn visible_to(endpoint: c_int, _devind: usize, _dev: &PciDevice) -> bool {
    match find_acl(endpoint) {
        None => true,   // C: `if (!aclp) return TRUE;`
        Some(_) => true, // TODO: full ACL device/class matching
    }
}

// ============================================================================
// Chardriver callbacks
// ============================================================================

unsafe extern "C" fn pci_c_open(
    _minor: ffi::DevMinor, _access: c_int, _user_endpt: c_int,
) -> c_int {
    0 // OK
}

unsafe extern "C" fn pci_c_close(_minor: ffi::DevMinor) -> c_int {
    0 // OK
}

unsafe extern "C" fn pci_c_ioctl(
    _minor: ffi::DevMinor, _request: c_ulong,
    _endpt: c_int, _grant: c_int, _flags: c_int, _user_endpt: c_int,
    _id: c_uint,
) -> c_int {
    libc_errno(-25) /* ENOTTY */
}

/// Main IPC message handler — dispatches BUSC_PCI messages.
unsafe extern "C" fn pci_c_other(m_ptr: *mut c_void, _ipc_status: c_int) {
    // SAFETY: m_ptr is a valid MINIX message from the chardriver framework
    let msg = unsafe { &*(m_ptr as *const PciMessage) };
    match msg.m_type() {
        ffi::platform::BUSC_PCI_INIT       => do_init(m_ptr),
        ffi::platform::BUSC_PCI_FIRST_DEV  => do_first_dev(m_ptr),
        ffi::platform::BUSC_PCI_NEXT_DEV   => do_next_dev(m_ptr),
        ffi::platform::BUSC_PCI_FIND_DEV   => do_find_dev(m_ptr),
        ffi::platform::BUSC_PCI_IDS        => do_ids(m_ptr),
        ffi::platform::BUSC_PCI_RESERVE    => do_reserve(m_ptr),
        ffi::platform::BUSC_PCI_ATTR_R8   => do_attr_read::<u8>(m_ptr),
        ffi::platform::BUSC_PCI_ATTR_R16  => do_attr_read::<u16>(m_ptr),
        ffi::platform::BUSC_PCI_ATTR_R32  => do_attr_read::<u32>(m_ptr),
        ffi::platform::BUSC_PCI_ATTR_W8   => do_attr_write::<u8>(m_ptr),
        ffi::platform::BUSC_PCI_ATTR_W16  => do_attr_write::<u16>(m_ptr),
        ffi::platform::BUSC_PCI_ATTR_W32  => do_attr_write::<u32>(m_ptr),
        ffi::platform::BUSC_PCI_GET_BAR    => do_get_bar(m_ptr),
        ffi::platform::BUSC_PCI_RESCAN     => do_rescan_bus(m_ptr),
        // --- NEW: Phase 5 handlers ---
        ffi::platform::BUSC_PCI_DEV_NAME_S  => do_dev_name(m_ptr),
        ffi::platform::BUSC_PCI_SLOT_NAME_S => do_slot_name(m_ptr),
        ffi::platform::BUSC_PCI_SET_ACL     => do_set_acl(m_ptr),
        ffi::platform::BUSC_PCI_DEL_ACL     => do_del_acl(m_ptr),
        _ => {
            // SAFETY: m_ptr is a valid mutable MINIX message
            let msg = unsafe { &mut *(m_ptr as *mut PciMessage) };
            let src = msg.m_source();
            msg.set_m_type(libc_errno(-25)); // ENOTTY
            let _ = ffi::platform::ipc_send(src, m_ptr);
        }
    }
}

// ============================================================================
// PciServer
// ============================================================================

pub struct PciServer;

impl PciServer {
    pub fn new() -> Self { PciServer }

    pub fn as_chardriver(&self) -> ffi::Chardriver {
        ffi::Chardriver {
            cdr_open: Some(pci_c_open),
            cdr_close: Some(pci_c_close),
            cdr_ioctl: Some(pci_c_ioctl),
            cdr_other: Some(pci_c_other),
        }
    }
}

// ============================================================================
// Generic message handler (INIT)
// ============================================================================

fn do_init(m_ptr: *mut c_void) {
    // SAFETY: m_ptr is a valid MINIX message from the chardriver framework
    let src = unsafe { (*(m_ptr as *const PciMessage)).m_source() };
    let msg = unsafe { &mut *(m_ptr as *mut PciMessage) };
    msg.set_m_type(0); // OK
    let _ = unsafe { ffi::platform::ipc_send(src, m_ptr) };
}

// ============================================================================
// Device enumeration handlers
// ============================================================================

fn do_first_dev(m_ptr: *mut c_void) {
    unsafe {
        let msg = &*(m_ptr as *const PciMessage);
        let src = msg.m_source();

        // ACL-aware first device: skip devices invisible to caller
        let result = crate::pci_state()
            .first_dev_where(|devind, dev| visible_to(src, devind, dev));

        let msg = &mut *(m_ptr as *mut PciMessage);
        match result {
            Some((devind, vid, did)) => {
                msg.set_m1_i1(devind as c_int);
                msg.set_m1_i2(vid as c_int);
                msg.set_m1_i3(did as c_int);
                msg.set_m_type(1);
            }
            None => { msg.set_m_type(0); }
        }
        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}

fn do_next_dev(m_ptr: *mut c_void) {
    unsafe {
        let msg = &*(m_ptr as *const PciMessage);
        let src = msg.m_source();
        let devind = msg.m1_i1() as usize;

        // ACL-aware next device: skip devices invisible to caller
        let result = crate::pci_state()
            .next_dev_where(devind, |devind, dev| visible_to(src, devind, dev));

        let msg = &mut *(m_ptr as *mut PciMessage);
        match result {
            Some((new_devind, vid, did)) => {
                msg.set_m1_i1(new_devind as c_int);
                msg.set_m1_i2(vid as c_int);
                msg.set_m1_i3(did as c_int);
                msg.set_m_type(1);
            }
            None => { msg.set_m_type(0); }
        }
        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}

fn do_find_dev(m_ptr: *mut c_void) {
    unsafe {
        let msg = &*(m_ptr as *const PciMessage);
        let src = msg.m_source();
        let bus = msg.m1_i1() as u8;
        let dev = msg.m1_i2() as u8;
        let func = msg.m1_i3() as u8;

        let found = crate::pci_state()
            .find_by_bdf(bus, dev, func)
            .map(|(devind, _)| devind);

        let msg = &mut *(m_ptr as *mut PciMessage);
        match found {
            Some(devind) => {
                msg.set_m1_i1(devind as c_int);
                msg.set_m_type(1);
            }
            None => { msg.set_m_type(0); }
        }
        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}

fn do_ids(m_ptr: *mut c_void) {
    unsafe {
        let msg = &*(m_ptr as *const PciMessage);
        let src = msg.m_source();
        let devind = msg.m1_i1() as usize;

        let msg = &mut *(m_ptr as *mut PciMessage);
        match crate::pci_state().get(devind) {
            Some(dev) => {
                msg.set_m1_i1(dev.vendor_id as c_int);
                msg.set_m1_i2(dev.device_id as c_int);
                msg.set_m_type(0);
            }
            None => { msg.set_m_type(libc_errno(-22)); }
        }
        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}

fn do_reserve(m_ptr: *mut c_void) {
    unsafe {
        let msg = &*(m_ptr as *const PciMessage);
        let src = msg.m_source();
        let devind = msg.m1_i1() as usize;

        let msg = &mut *(m_ptr as *mut PciMessage);

        // C semantics: first check bounds (EINVAL), then visibility (EPERM), then reserve
        match crate::pci_state().get(devind) {
            None => {
                msg.set_m_type(libc_errno(-22)); // EINVAL: bad devind
            }
            Some(d) if !visible_to(src, devind, d) => {
                msg.set_m_type(libc_errno(-1)); // EPERM: not allowed to see device
            }
            _ => {
                match crate::pci_state().reserve(devind, src) {
                    Ok(()) => msg.set_m_type(0),
                    Err(e) => msg.set_m_type(e),
                }
            }
        }
        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}

// ============================================================================
// Config space read/write handlers (generic over access width)
// ============================================================================

fn do_attr_read<T: ConfigRead>(m_ptr: *mut c_void) {
    unsafe {
        let msg = &*(m_ptr as *const PciMessage);
        let src = msg.m_source();
        let devind = msg.m2_i1() as usize;
        let port = msg.m2_i2() as u8;

        let msg = &mut *(m_ptr as *mut PciMessage);
        match crate::pci_state().get(devind) {
            Some(dev) => {
                let val = T::read(dev.bus, dev.dev, dev.func, port);
                msg.set_m2_l1(val as c_ulong);
                msg.set_m_type(0);
            }
            None => { msg.set_m_type(libc_errno(-22)); }
        }
        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}

fn do_attr_write<T: ConfigWrite>(m_ptr: *mut c_void) {
    unsafe {
        let msg = &*(m_ptr as *const PciMessage);
        let src = msg.m_source();
        let devind = msg.m2_i1() as usize;
        let port = msg.m2_i2() as u8;
        let val = msg.m2_l1();

        let msg = &mut *(m_ptr as *mut PciMessage);
        match crate::pci_state().get(devind) {
            Some(dev) => {
                T::write(dev.bus, dev.dev, dev.func, port, val);
                msg.set_m_type(0);
            }
            None => { msg.set_m_type(libc_errno(-22)); }
        }
        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}

/// Trait for config space read operations.
trait ConfigRead {
    fn read(bus: u8, dev: u8, func: u8, port: u8) -> c_ulong;
}

impl ConfigRead for u8 {
    fn read(bus: u8, dev: u8, func: u8, port: u8) -> c_ulong {
        unsafe { ffi::pci_read_config8(bus, dev, func, port) as c_ulong }
    }
}

impl ConfigRead for u16 {
    fn read(bus: u8, dev: u8, func: u8, port: u8) -> c_ulong {
        unsafe { ffi::pci_read_config16(bus, dev, func, port) as c_ulong }
    }
}

impl ConfigRead for u32 {
    fn read(bus: u8, dev: u8, func: u8, port: u8) -> c_ulong {
        unsafe { ffi::pci_read_config32(bus, dev, func, port) as c_ulong }
    }
}

/// Trait for config space write operations.
trait ConfigWrite {
    fn write(bus: u8, dev: u8, func: u8, port: u8, val: c_ulong);
}

impl ConfigWrite for u8 {
    fn write(bus: u8, dev: u8, func: u8, port: u8, val: c_ulong) {
        unsafe { ffi::pci_write_config8(bus, dev, func, port, val as u8) }
    }
}

impl ConfigWrite for u16 {
    fn write(bus: u8, dev: u8, func: u8, port: u8, val: c_ulong) {
        unsafe { ffi::pci_write_config16(bus, dev, func, port, val as u16) }
    }
}

impl ConfigWrite for u32 {
    fn write(bus: u8, dev: u8, func: u8, port: u8, val: c_ulong) {
        unsafe { ffi::pci_write_config32(bus, dev, func, port, val as u32) }
    }
}

// ============================================================================
// BAR query
// ============================================================================

fn do_get_bar(m_ptr: *mut c_void) {
    unsafe {
        let msg = &*(m_ptr as *const PciMessage);
        let src = msg.m_source();
        let devind = msg.bar_devind() as usize;
        let port_reg = msg.bar_port() as u8;

        let msg = &mut *(m_ptr as *mut PciMessage);
        match crate::pci_state().get(devind) {
            Some(dev) => {
                let mut found = false;
                let bar0_offset = crate::devices::PCI_BAR_0 as u8;
                for (i, bar) in dev.bars.iter().enumerate() {
                    if let Some(b) = bar {
                        let bar_reg = bar0_offset + (i as u8) * 4;
                        if bar_reg == port_reg {
                            msg.set_bar_base(b.base);
                            msg.set_bar_size(b.size);
                            msg.set_bar_flags(if b.is_io { 1 } else { 0 });
                            found = true;
                            break;
                        }
                    }
                }
                if found { msg.set_m_type(0); }
                else { msg.set_m_type(libc_errno(-22)); }
            }
            None => { msg.set_m_type(libc_errno(-22)); }
        }
        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}

// ============================================================================
// Bus rescan
// ============================================================================

fn do_rescan_bus(m_ptr: *mut c_void) {
    unsafe {
        let src = (*(m_ptr as *const PciMessage)).m_source();

        let mut table = PciDeviceTable::new();
        table.probe_all();
        crate::PCI_STATE = Some(table);

        let msg = &mut *(m_ptr as *mut PciMessage);
        msg.set_m_type(0);
        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}

// ============================================================================
// Device name lookup (DEV_NAME_S)
// ============================================================================

fn do_dev_name(m_ptr: *mut c_void) {
    unsafe {
        let msg = &*(m_ptr as *const PciMessage);
        let src = msg.m_source();
        let vid = msg.m7_i1() as u16;
        let _did = msg.m7_i2() as u16;
        let name_len = msg.m7_i3() as usize;
        let name_gid = msg.m7_i4();

        let msg = &mut *(m_ptr as *mut PciMessage);

        // Look up device name via PCI ID database (NetBSD `pci_findproduct`).
        // For the Rust pilot, we don't have access to the C library's PCI ID DB,
        // so return ENOENT. This matches the C code path when _pci_dev_name is NULL.
        //
        // In the future, we can embed a PCI ID database or call the C function via FFI.
        let _ = (vid, _did, name_len, name_gid);
        msg.set_m_type(libc_errno(-2)); // ENOENT

        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}

// ============================================================================
// Slot name formatting (SLOT_NAME_S)
// ============================================================================

/// Format an unsigned 8-bit value into a buffer (decimal).
/// Returns the new position after the formatted digits.
fn write_dec_u8(buf: &mut [u8], mut pos: usize, val: u8) -> usize {
    if val >= 100 {
        buf[pos] = b'0' + val / 100; pos += 1;
        buf[pos] = b'0' + (val / 10) % 10; pos += 1;
        buf[pos] = b'0' + val % 10; pos += 1;
    } else if val >= 10 {
        buf[pos] = b'0' + val / 10; pos += 1;
        buf[pos] = b'0' + val % 10; pos += 1;
    } else {
        buf[pos] = b'0' + val; pos += 1;
    }
    pos
}

/// Format "domain.bus.dev.func" slot name and copy to caller via safecopyto.
/// Matches the C `_pci_slot_name` format.
fn do_slot_name(m_ptr: *mut c_void) {
    unsafe {
        let msg = &*(m_ptr as *const PciMessage);
        let src = msg.m_source();
        let devind = msg.m1_i1() as usize;
        let name_len = msg.m1_i2() as usize;
        let gid = msg.m1_i3();

        let msg = &mut *(m_ptr as *mut PciMessage);

        match crate::pci_state().get(devind) {
            Some(dev) => {
                // Format "0.bus.dev.func" into a stack buffer
                // Max: "0.255.31.7" = 12 chars + NUL = 13 bytes
                let mut buf: [u8; 20] = [0; 20];
                let mut pos = 0;
                buf[pos] = b'0'; pos += 1;
                buf[pos] = b'.'; pos += 1;
                pos = write_dec_u8(&mut buf, pos, dev.bus);
                buf[pos] = b'.'; pos += 1;
                pos = write_dec_u8(&mut buf, pos, dev.dev);
                buf[pos] = b'.'; pos += 1;
                pos = write_dec_u8(&mut buf, pos, dev.func);
                buf[pos] = b'\0';
                let len = if (pos + 1) <= name_len { pos + 1 } else { name_len };

                let r = ffi::platform::sys_safecopyto(
                    src, gid, 0, buf.as_ptr() as *const c_void, len as c_ulong);
                if r != 0 {
                    msg.set_m_type(r);
                } else {
                    msg.set_m_type(0); // OK
                }
            }
            None => {
                msg.set_m_type(libc_errno(-22)); // EINVAL
            }
        }
        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}

// ============================================================================
// ACL management (SET_ACL / DEL_ACL)
// ============================================================================

/// Set an ACL entry for a driver (called by RS only).
///
/// m1_i1 = grant ID containing `struct rs_pci` with the ACL data.
/// Only RS_PROC_NR (-5) is allowed to call this.
fn do_set_acl(m_ptr: *mut c_void) {
    unsafe {
        let msg = &*(m_ptr as *const PciMessage);
        let src = msg.m_source();

        let msg = &mut *(m_ptr as *mut PciMessage);

        // Only RS is allowed to set ACLs
        if src != ffi::platform::RS_PROC_NR {
            msg.set_m_type(libc_errno(-1)); // EPERM
            let _ = ffi::platform::ipc_send(src, m_ptr);
            return;
        }

        let gid = msg.m1_i1();

        // Find a free ACL slot
        let mut slot = None;
        for i in 0..NR_DRIVERS {
            if ACL_INUSE[i] == 0 {
                slot = Some(i);
                break;
            }
        }

        let i = match slot {
            Some(i) => i,
            None => {
                msg.set_m_type(libc_errno(-12)); // ENOMEM
                let _ = ffi::platform::ipc_send(src, m_ptr);
                return;
            }
        };

        // Read the grant data (rs_pci struct) into a buffer.
        // The struct contains: endpoint, label, device/class filters.
        // We only extract the endpoint for now; full visibility filtering
        // via ACL can be added later.
        let mut buf: [u8; 256] = [0; 256];
        let r = ffi::platform::sys_safecopyfrom(
            src, gid, 0, buf.as_mut_ptr() as *mut c_void, 256);
        if r != 0 {
            msg.set_m_type(r);
            let _ = ffi::platform::ipc_send(src, m_ptr);
            return;
        }

        // First field of rs_pci is rsp_endpoint (c_int)
        let endpoint = core::ptr::read_unaligned(buf.as_ptr() as *const c_int);
        ACL_INUSE[i] = 1;
        ACL_ENDPOINT[i] = endpoint;

        msg.set_m_type(0); // OK
        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}

/// Delete an ACL entry (called by RS only).
///
/// m1_i1 = endpoint number to remove.
/// Also releases all devices held by that endpoint.
fn do_del_acl(m_ptr: *mut c_void) {
    unsafe {
        let msg = &*(m_ptr as *const PciMessage);
        let src = msg.m_source();

        let msg = &mut *(m_ptr as *mut PciMessage);

        // Only RS is allowed to delete ACLs
        if src != ffi::platform::RS_PROC_NR {
            msg.set_m_type(libc_errno(-1)); // EPERM
            let _ = ffi::platform::ipc_send(src, m_ptr);
            return;
        }

        let proc_nr = msg.m1_i1();

        // Find ACL entry for this endpoint
        let mut found = false;
        for i in 0..NR_DRIVERS {
            if ACL_INUSE[i] != 0 && ACL_ENDPOINT[i] == proc_nr {
                ACL_INUSE[i] = 0;
                ACL_ENDPOINT[i] = 0;
                found = true;
                break;
            }
        }

        if !found {
            msg.set_m_type(libc_errno(-22)); // EINVAL
            let _ = ffi::platform::ipc_send(src, m_ptr);
            return;
        }

        // Release all devices held by this process (like C _pci_release)
        crate::pci_state().release_by_endpoint(proc_nr);

        msg.set_m_type(0); // OK
        let _ = ffi::platform::ipc_send(src, m_ptr);
    }
}
