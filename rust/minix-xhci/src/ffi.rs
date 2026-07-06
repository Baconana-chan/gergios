//! # FFI — MINIX C system call bindings for the xHCI USB 3.0 driver
//!
//! Dual-platform: real MINIX extern blocks + host stubs for cargo test.

#![allow(dead_code, unused_imports)]

use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};

pub type CLong = isize;
pub use CLong as c_long;

// ============================================================================
// Platform selection
// ============================================================================

#[cfg(target_os = "minix")]
mod platform {
    use super::*;

    unsafe extern "C" {
        pub fn pci_init() -> c_int;
        pub fn pci_first_dev(devindp: *mut c_int, vidp: *mut u16, didp: *mut u16) -> c_int;
        pub fn pci_next_dev(devindp: *mut c_int, vidp: *mut u16, didp: *mut u16) -> c_int;
        pub fn pci_reserve(devind: c_int);
        pub fn pci_get_bar(devind: c_int, bar: c_int, base: *mut u32,
            size_: *mut u32, ioflag: *mut c_int) -> c_int;
        pub fn pci_attr_r8(devind: c_int, offset: c_int) -> u8;
        pub fn pci_attr_r16(devind: c_int, offset: c_int) -> u16;
        pub fn pci_attr_r32(devind: c_int, offset: c_int) -> u32;
        pub fn pci_attr_w16(devind: c_int, offset: c_int, val: u16);
        pub fn pci_find_cap(devind: c_int, cap_id: c_int, cap_ptr: *mut c_int) -> c_int;
        pub fn pci_msix_parse(devind: c_int, info: *mut PciMsixInfo) -> c_int;

        pub fn vm_map_phys(endpt: c_int, base: *mut c_void, size: usize) -> *mut c_void;
        pub fn vm_unmap_phys(endpt: c_int, base: *mut c_void, size: usize) -> c_int;

        pub fn alloc_contig(size: usize, flags: c_int, phys: *mut u64) -> *mut c_void;
        pub fn free_contig(addr: *mut c_void, size: usize);

        pub fn sys_irqsetpolicy(irq: c_int, policy: c_int, hook_id: *mut c_int) -> c_int;
        pub fn sys_irqenable(hook_id: *mut c_int) -> c_int;
        pub fn sys_irqrmpolicy(hook_id: *mut c_int) -> c_int;
        pub fn sys_msix_alloc(irq: *mut c_int) -> c_int;
        pub fn sys_msix_free(irq: c_int) -> c_int;
        pub fn sys_msix_setpolicy(irq: c_int, policy: c_int, hook_id: *mut c_int) -> c_int;
        pub fn sys_irqthread_priority(irq: c_int, priority: c_int) -> c_int;

        pub fn sef_setcb_init_fresh(cb: Option<SefInitFreshFn>);
        pub fn sef_setcb_signal_handler(cb: Option<SefSignalHandlerFn>);
        pub fn sef_startup();
        pub fn env_setargs(argc: c_int, argv: *mut *mut c_char);
        pub fn env_parse(name: *const c_char, fmt: *const c_char, field: c_int,
            val: *mut c_long, min: c_long, max: c_long) -> c_int;

        pub fn sys_hz() -> u64;
        pub fn micro_delay(us: c_uint);

        pub fn printf(fmt: *const c_char, arg: *const c_char) -> c_int;
        pub fn sys_safecopyto(proc: c_int, grant: c_int, offset: c_ulong,
            buf: *const c_void, bytes: c_ulong) -> c_int;
        pub fn sys_safecopyfrom(proc: c_int, grant: c_int, offset: c_ulong,
            buf: *mut c_void, bytes: c_ulong) -> c_int;

        // Chardriver framework
        pub fn cdr_task(cdp: *const Chardriver);
        pub fn cdr_announce(type_: c_int);
        pub fn cdr_terminate();

        // IPC
        pub fn sef_receive_status(src: c_int, msg: *mut u8, status: *mut c_int) -> c_int;
        pub fn send(dst: c_int, msg: *const u8) -> c_int;

        // System State Control (kernel call wrapper, in libsys)
        pub fn sys_statectl(request: c_int, address: *mut core::ffi::c_void, length: c_int) -> c_int;

        // DS (Data Store) — label retrieval + publish
        pub fn ds_retrieve_label_name(name: *mut c_char, endpoint: c_int) -> c_int;
        pub fn ds_publish_u32(name: *const c_char, val: u32, flags: c_int) -> c_int;

        // SEF — self endpoint
        pub fn sef_self() -> c_int;
    }

    #[repr(C)]
    pub struct PciMsixInfo {
        pub msix_table_size: c_int,
        pub msix_table_bir: c_int,
        pub msix_table_offset: u32,
        pub msix_pba_bir: c_int,
        pub msix_pba_offset: u32,
    }

    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

    pub type DevMinor = c_int;
    pub const SELF: c_int = -0x100;
    pub const AC_ALIGN4K: c_int = 1;
    pub const PAGE_SIZE: usize = 4096;

    // errno constants
    pub const OK: c_int = 0;
    pub const EIO: c_int = -5;
    pub const ENXIO: c_int = -6;
    pub const ENOMEM: c_int = -12;
    pub const EACCES: c_int = -13;
    pub const EBUSY: c_int = -16;
    pub const EINVAL: c_int = -22;
    pub const ENOTTY: c_int = -25;
    pub const EAGAIN: c_int = -35;
    pub const EPROTO: c_int = -71;

    // MINIX chardev message types
    pub const CDEV_OPEN: c_int = 0x200;
    pub const CDEV_CLOSE: c_int = 0x201;
    pub const CDEV_READ: c_int = 0x202;
    pub const CDEV_WRITE: c_int = 0x203;
    pub const CDEV_IOCTL: c_int = 0x204;
    pub const CDEV_SELECT: c_int = 0x205;

    // MINIX blockdev message types
    pub const BDEV_OPEN: c_int = 0x100;
    pub const BDEV_CLOSE: c_int = 0x101;
    pub const BDEV_READ: c_int = 0x102;
    pub const BDEV_WRITE: c_int = 0x103;
    pub const BDEV_GATHER: c_int = 0x104;
    pub const BDEV_SCATTER: c_int = 0x105;
    pub const BDEV_IOCTL: c_int = 0x106;

    pub const ANY: c_int = -1;

    // MINIX notification
    pub const NOTIFY_MESSAGE: c_int = -3;
    pub const HARDWARE: c_int = 0;
    pub const CLOCK: c_int = 1;

    // DS constants
    pub const DS_DRIVER_UP: u32 = 1;
    pub const DSF_OVERWRITE: c_int = 0x01000;
    pub const DS_MAX_KEYLEN: usize = 80;

    // System state control
    pub const SYS_STATE_CLEAR_IPC_REFS: c_int = 1;

    /// Chardriver callback table (MINIX chardriver.h compatible).
    #[repr(C)]
    pub struct Chardriver {
        pub cdr_type: c_int,
        pub cdr_open: Option<unsafe extern "C" fn(DevMinor, c_int, c_int) -> c_int>,
        pub cdr_close: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
        pub cdr_read: Option<unsafe extern "C" fn(
            DevMinor, u64, c_int, c_int, usize, c_int, c_int) -> isize>,
        pub cdr_write: Option<unsafe extern "C" fn(
            DevMinor, u64, c_int, c_int, usize, c_int, c_int) -> isize>,
        pub cdr_ioctl: Option<unsafe extern "C" fn(
            DevMinor, c_ulong, c_int, c_int, c_int, c_int) -> c_int>,
        pub cdr_select: Option<unsafe extern "C" fn(DevMinor, c_uint, c_int) -> c_int>,
        pub cdr_intr: Option<unsafe extern "C" fn(c_uint)>,
        pub cdr_alarm: Option<unsafe extern "C" fn(u64)>,
        pub cdr_other: Option<unsafe extern "C" fn(*const c_void, *mut c_void)>,
        pub cdr_device: Option<unsafe extern "C" fn(DevMinor, *mut c_int) -> c_int>,
        pub cdr_signal: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
    }
}

#[cfg(not(target_os = "minix"))]
mod platform {
    use super::*;
    use core::ptr;

    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

    pub type DevMinor = c_int;
    pub const SELF: c_int = -0x100;
    pub const AC_ALIGN4K: c_int = 1;
    pub const PAGE_SIZE: usize = 4096;

    pub const OK: c_int = 0;
    pub const EIO: c_int = -5;
    pub const ENXIO: c_int = -6;
    pub const ENOMEM: c_int = -12;
    pub const EACCES: c_int = -13;
    pub const EBUSY: c_int = -16;
    pub const EINVAL: c_int = -22;
    pub const ENOTTY: c_int = -25;
    pub const EAGAIN: c_int = -35;
    pub const EPROTO: c_int = -71;

    pub const CDEV_OPEN: c_int = 0x200;
    pub const CDEV_CLOSE: c_int = 0x201;
    pub const CDEV_READ: c_int = 0x202;
    pub const CDEV_WRITE: c_int = 0x203;
    pub const CDEV_IOCTL: c_int = 0x204;
    pub const CDEV_SELECT: c_int = 0x205;

    pub const BDEV_OPEN: c_int = 0x100;
    pub const BDEV_CLOSE: c_int = 0x101;
    pub const BDEV_READ: c_int = 0x102;
    pub const BDEV_WRITE: c_int = 0x103;
    pub const BDEV_GATHER: c_int = 0x104;
    pub const BDEV_SCATTER: c_int = 0x105;
    pub const BDEV_IOCTL: c_int = 0x106;

    pub const ANY: c_int = -1;

    pub const NOTIFY_MESSAGE: c_int = -3;
    pub const HARDWARE: c_int = 0;
    pub const CLOCK: c_int = 1;

    // DS constants
    pub const DS_DRIVER_UP: u32 = 1;
    pub const DSF_OVERWRITE: c_int = 0x01000;
    pub const DS_MAX_KEYLEN: usize = 80;

    // System state control
    pub const SYS_STATE_CLEAR_IPC_REFS: c_int = 1;

    #[repr(C)]
    pub struct PciMsixInfo {
        pub msix_table_size: c_int,
        pub msix_table_bir: c_int,
        pub msix_table_offset: u32,
        pub msix_pba_bir: c_int,
        pub msix_pba_offset: u32,
    }

    #[repr(C)]
    pub struct Blockdriver {
        pub bdr_type: c_int,
        pub bdr_open: Option<unsafe extern "C" fn(DevMinor, c_int) -> c_int>,
        pub bdr_close: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
        pub bdr_transfer: Option<unsafe extern "C" fn(
            DevMinor, c_int, u64, c_int, *mut c_void, c_uint, c_int) -> isize>,
        pub bdr_ioctl: Option<unsafe extern "C" fn(
            DevMinor, c_ulong, c_int, c_int, c_int) -> c_int>,
        pub bdr_part: Option<unsafe extern "C" fn(DevMinor) -> *mut c_void>,
        pub bdr_intr: Option<unsafe extern "C" fn(c_uint)>,
        pub bdr_alarm: Option<unsafe extern "C" fn(u64)>,
        pub bdr_device: Option<unsafe extern "C" fn(DevMinor, *mut c_int) -> c_int>,
    }

    /// Chardriver callback table (host stub).
    #[repr(C)]
    pub struct Chardriver {
        pub cdr_type: c_int,
        pub cdr_open: Option<unsafe extern "C" fn(DevMinor, c_int, c_int) -> c_int>,
        pub cdr_close: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
        pub cdr_read: Option<unsafe extern "C" fn(
            DevMinor, u64, c_int, c_int, usize, c_int, c_int) -> isize>,
        pub cdr_write: Option<unsafe extern "C" fn(
            DevMinor, u64, c_int, c_int, usize, c_int, c_int) -> isize>,
        pub cdr_ioctl: Option<unsafe extern "C" fn(
            DevMinor, c_ulong, c_int, c_int, c_int, c_int) -> c_int>,
        pub cdr_select: Option<unsafe extern "C" fn(DevMinor, c_uint, c_int) -> c_int>,
        pub cdr_intr: Option<unsafe extern "C" fn(c_uint)>,
        pub cdr_alarm: Option<unsafe extern "C" fn(u64)>,
        pub cdr_other: Option<unsafe extern "C" fn(*const c_void, *mut c_void)>,
        pub cdr_device: Option<unsafe extern "C" fn(DevMinor, *mut c_int) -> c_int>,
        pub cdr_signal: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
    }

    // Stubs for host-side testing
    pub unsafe fn pci_init() -> c_int { 0 }
    pub unsafe fn pci_first_dev(_devindp: *mut c_int, _vidp: *mut u16, _didp: *mut u16) -> c_int { -1 }
    pub unsafe fn pci_next_dev(_devindp: *mut c_int, _vidp: *mut u16, _didp: *mut u16) -> c_int { -1 }
    pub unsafe fn pci_reserve(_devind: c_int) {}
    pub unsafe fn pci_get_bar(_devind: c_int, _bar: c_int, _base: *mut u32,
        _size_: *mut u32, _ioflag: *mut c_int) -> c_int { -1 }
    pub unsafe fn pci_attr_r8(_devind: c_int, _offset: c_int) -> u8 { 0 }
    pub unsafe fn pci_attr_r16(_devind: c_int, _offset: c_int) -> u16 { 0 }
    pub unsafe fn pci_attr_r32(_devind: c_int, _offset: c_int) -> u32 { 0 }
    pub unsafe fn pci_attr_w16(_devind: c_int, _offset: c_int, _val: u16) {}
    pub unsafe fn pci_find_cap(_devind: c_int, _cap_id: c_int, _cap_ptr: *mut c_int) -> c_int { 0 }
    pub unsafe fn pci_msix_parse(_devind: c_int, _info: *mut PciMsixInfo) -> c_int { 0 }

    pub unsafe fn vm_map_phys(_endpt: c_int, _base: *mut c_void, _size: usize) -> *mut c_void {
        ptr::null_mut()
    }
    pub unsafe fn vm_unmap_phys(_endpt: c_int, _base: *mut c_void, _size: usize) -> c_int { -1 }

    pub unsafe fn alloc_contig(_size: usize, _flags: c_int, _phys: *mut u64) -> *mut c_void {
        ptr::null_mut()
    }
    pub unsafe fn free_contig(_addr: *mut c_void, _size: usize) {}

    pub unsafe fn sys_irqsetpolicy(_irq: c_int, _policy: c_int, _hook_id: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_irqenable(_hook_id: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_irqrmpolicy(_hook_id: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_msix_alloc(_irq: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_msix_free(_irq: c_int) -> c_int { -1 }
    pub unsafe fn sys_msix_setpolicy(_irq: c_int, _policy: c_int, _hook_id: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_irqthread_priority(_irq: c_int, _priority: c_int) -> c_int { -1 }

    pub unsafe fn sef_setcb_init_fresh(_cb: *mut c_void) {}
    pub unsafe fn sef_setcb_signal_handler(_cb: *mut c_void) {}
    pub unsafe fn sef_startup() {}
    pub unsafe fn env_setargs(_argc: c_int, _argv: *mut *mut c_char) {}
    pub unsafe fn env_parse(_name: *const c_char, _fmt: *const c_char, _field: c_int,
        _val: *mut c_long, _min: c_long, _max: c_long) -> c_int { 0 }

    pub unsafe fn sys_hz() -> u64 { 100 }
    pub unsafe fn micro_delay(_us: c_uint) {}

    pub unsafe fn printf(_fmt: *const c_char, _arg: *const c_char) -> c_int { 0 }
    pub unsafe fn sys_safecopyto(_proc: c_int, _grant: c_int, _offset: c_ulong,
        _buf: *const c_void, _bytes: c_ulong) -> c_int { -1 }
    pub unsafe fn sys_safecopyfrom(_proc: c_int, _grant: c_int, _offset: c_ulong,
        _buf: *mut c_void, _bytes: c_ulong) -> c_int { -1 }

    pub unsafe fn cdr_task(_cdp: *const Chardriver) {}
    pub unsafe fn cdr_announce(_type_: c_int) {}
    pub unsafe fn cdr_terminate() {}

    pub unsafe fn sef_receive_status(
        _src: c_int, _msg: *mut u8, _status: *mut c_int
    ) -> c_int { -1 }
    pub unsafe fn send(_dst: c_int, _msg: *const u8) -> c_int { -1 }

    pub unsafe fn sys_statectl(
        _request: c_int, _address: *mut core::ffi::c_void, _length: c_int
    ) -> c_int { OK }
    pub unsafe fn ds_retrieve_label_name(
        _name: *mut c_char, _endpoint: c_int
    ) -> c_int { ENXIO }
    pub unsafe fn ds_publish_u32(
        _name: *const c_char, _val: u32, _flags: c_int
    ) -> c_int { OK }
    pub unsafe fn sef_self() -> c_int { SELF }
}

// ============================================================================
// MINIX IPC Message Struct
// ============================================================================

/// MINIX IPC message — opaque storage matching sizeof(message) on the target.
/// Layout (LP64 / x86_64 MINIX):
///   offset 0: m_source (c_int, 4 bytes)
///   offset 4: m_type   (c_int, 4 bytes)
///   offset 8: union payload (up to 48 bytes on 64-bit)
///
/// Sub-structs used by different message types:
///   mess_1 (CDEV_OPEN/CLOSE):  i1@8, i2@12, i3@16    — all c_int
///   mess_4 (CDEV_READ/WRITE/IOCTL): l1@8..l5@40       — 5× c_long on LP64
///   mess_2 (BDEV_OPEN/CLOSE/READ/WRITE): i1@8, i2@12, l1@16, i3@24, i4@28, l2@32, i5@40, i6@44
#[repr(C)]
pub struct Message {
    raw: [u8; 56],
}

impl Message {
    /// Create a zero-initialised message.
    pub const fn zero() -> Self {
        Self { raw: [0u8; 56] }
    }

    pub fn m_source(&self) -> c_int {
        c_int::from_ne_bytes([self.raw[0], self.raw[1], self.raw[2], self.raw[3]])
    }
    pub fn m_type(&self) -> c_int {
        c_int::from_ne_bytes([self.raw[4], self.raw[5], self.raw[6], self.raw[7]])
    }
    pub fn set_m_source(&mut self, v: c_int) {
        self.raw[0..4].copy_from_slice(&v.to_ne_bytes());
    }
    pub fn set_m_type(&mut self, v: c_int) {
        self.raw[4..8].copy_from_slice(&v.to_ne_bytes());
    }

    // ── mess_1 — CDEV_OPEN / CDEV_CLOSE ──────────────────────────────────
    pub fn m1_i1(&self) -> c_int {
        c_int::from_ne_bytes([self.raw[8], self.raw[9], self.raw[10], self.raw[11]])
    }
    pub fn m1_i2(&self) -> c_int {
        c_int::from_ne_bytes([self.raw[12], self.raw[13], self.raw[14], self.raw[15]])
    }
    pub fn m1_i3(&self) -> c_int {
        c_int::from_ne_bytes([self.raw[16], self.raw[17], self.raw[18], self.raw[19]])
    }

    // ── mess_4 — CDEV_READ / CDEV_WRITE / CDEV_IOCTL ─────────────────────
    pub fn m4_l1(&self) -> c_long {
        read_c_long(&self.raw[8..])
    }
    pub fn m4_l2(&self) -> c_long {
        read_c_long(&self.raw[16..])
    }
    pub fn m4_l3(&self) -> c_long {
        read_c_long(&self.raw[24..])
    }
    pub fn m4_l4(&self) -> c_long {
        read_c_long(&self.raw[32..])
    }
    pub fn m4_l5(&self) -> c_long {
        read_c_long(&self.raw[40..])
    }

    // ── mess_2 — BDEV_OPEN / BDEV_CLOSE / BDEV_READ / BDEV_WRITE ────────
    pub fn m2_i1(&self) -> c_int {
        c_int::from_ne_bytes([self.raw[8], self.raw[9], self.raw[10], self.raw[11]])
    }
    pub fn m2_i2(&self) -> c_int {
        c_int::from_ne_bytes([self.raw[12], self.raw[13], self.raw[14], self.raw[15]])
    }
    pub fn m2_l1(&self) -> c_long {
        read_c_long(&self.raw[16..])
    }
    pub fn m2_i3(&self) -> c_int {
        c_int::from_ne_bytes([self.raw[24], self.raw[25], self.raw[26], self.raw[27]])
    }
    pub fn m2_i4(&self) -> c_int {
        c_int::from_ne_bytes([self.raw[28], self.raw[29], self.raw[30], self.raw[31]])
    }
    pub fn m2_l2(&self) -> c_long {
        read_c_long(&self.raw[32..])
    }
    pub fn m2_i5(&self) -> c_int {
        c_int::from_ne_bytes([self.raw[40], self.raw[41], self.raw[42], self.raw[43]])
    }

    /// Set m_type to result code for reply.
    pub fn set_result(&mut self, val: c_int) {
        self.set_m_type(val);
    }
}

/// Read a `c_long` from the first `size_of::<c_long>()` bytes of `slice`.
fn read_c_long(slice: &[u8]) -> c_long {
    let sz = core::mem::size_of::<c_long>();
    let mut buf = [0u8; 8];
    buf[..sz].copy_from_slice(&slice[..sz]);
    c_long::from_ne_bytes(buf)
}

// ============================================================================
// Public API wrappers (same pattern as minix-nvme)
// ============================================================================

pub type DevMinor = platform::DevMinor;
pub type endpoint_t = c_int;
pub type cp_grant_id_t = c_int;
pub type cdev_id_t = c_int;

/// Safe wrapper: copy data FROM kernel TO userspace via safecopy.
pub fn sys_safecopyto_wrapper(
    proc_nr: c_int, grant: cp_grant_id_t, offset: c_ulong,
    buf: *const c_void, bytes: c_ulong
) -> c_int {
    unsafe { platform::sys_safecopyto(proc_nr, grant, offset, buf, bytes) }
}

/// Safe wrapper: copy data FROM userspace TO kernel via safecopy.
pub fn sys_safecopyfrom_wrapper(
    proc_nr: c_int, grant: cp_grant_id_t, offset: c_ulong,
    buf: *mut c_void, bytes: c_ulong
) -> c_int {
    unsafe { platform::sys_safecopyfrom(proc_nr, grant, offset, buf, bytes) }
}

pub fn pci_init_ffi() -> c_int { unsafe { platform::pci_init() } }

pub fn pci_first_dev_ffi() -> Option<(c_int, u16, u16)> {
    unsafe {
        let mut devind: c_int = 0;
        let mut vid: u16 = 0;
        let mut did: u16 = 0;
        let r = platform::pci_first_dev(&mut devind, &mut vid, &mut did);
        if r <= 0 { None } else { Some((devind, vid, did)) }
    }
}

pub fn pci_next_dev_ffi() -> Option<(c_int, u16, u16)> {
    unsafe {
        let mut devind: c_int = 0;
        let mut vid: u16 = 0;
        let mut did: u16 = 0;
        let r = platform::pci_next_dev(&mut devind, &mut vid, &mut did);
        if r <= 0 { None } else { Some((devind, vid, did)) }
    }
}

pub fn pci_reserve_ffi(devind: c_int) { unsafe { platform::pci_reserve(devind) } }

pub fn pci_get_bar_ffi(devind: c_int, bar: c_int) -> Option<(u32, u32, bool)> {
    unsafe {
        let mut base: u32 = 0;
        let mut size_: u32 = 0;
        let mut ioflag: c_int = 0;
        let r = platform::pci_get_bar(devind, bar, &mut base, &mut size_, &mut ioflag);
        if r != 0 { None } else { Some((base, size_, ioflag != 0)) }
    }
}

pub fn pci_attr_r8_ffi(devind: c_int, offset: c_int) -> u8 {
    unsafe { platform::pci_attr_r8(devind, offset) }
}

pub fn pci_attr_r16_ffi(devind: c_int, offset: c_int) -> u16 {
    unsafe { platform::pci_attr_r16(devind, offset) }
}

pub fn pci_attr_r32_ffi(devind: c_int, offset: c_int) -> u32 {
    unsafe { platform::pci_attr_r32(devind, offset) }
}

pub fn pci_find_cap_ffi(devind: c_int, cap_id: c_int) -> Option<c_int> {
    unsafe {
        let mut cap_ptr: c_int = 0;
        let r = platform::pci_find_cap(devind, cap_id, &mut cap_ptr);
        if r == 0 { None } else { Some(cap_ptr) }
    }
}

pub fn pci_msix_parse_ffi(devind: c_int) -> Option<platform::PciMsixInfo> {
    unsafe {
        let mut info = platform::PciMsixInfo {
            msix_table_size: 0,
            msix_table_bir: 0,
            msix_table_offset: 0,
            msix_pba_bir: 0,
            msix_pba_offset: 0,
        };
        let r = platform::pci_msix_parse(devind, &mut info);
        if r == 0 { None } else { Some(info) }
    }
}

pub fn vm_map_phys_ffi(base: *mut c_void, size: usize) -> *mut c_void {
    unsafe { platform::vm_map_phys(platform::SELF, base, size) }
}

pub fn vm_unmap_phys_ffi(base: *mut c_void, size: usize) -> c_int {
    unsafe { platform::vm_unmap_phys(platform::SELF, base, size) }
}

pub fn alloc_contig_ffi(size: usize) -> Option<(*mut c_void, u64)> {
    unsafe {
        let mut phys: u64 = 0;
        let ptr = platform::alloc_contig(size, platform::AC_ALIGN4K, &mut phys);
        if ptr.is_null() { None } else { Some((ptr, phys)) }
    }
}

pub fn free_contig_ffi(addr: *mut c_void, size: usize) {
    unsafe { platform::free_contig(addr, size) }
}

pub fn irq_setup(irq: c_int) -> Option<c_int> {
    unsafe {
        let mut hook_id: c_int = 0;
        let r = platform::sys_irqsetpolicy(irq, 0, &mut hook_id);
        if r != 0 { return None; }
        let r = platform::sys_irqenable(&mut hook_id);
        if r != 0 { return None; }
        Some(hook_id)
    }
}

pub fn irq_remove(hook_id: &mut c_int) -> c_int {
    unsafe { platform::sys_irqrmpolicy(hook_id) }
}

pub fn msix_alloc_irq() -> Option<c_int> {
    unsafe {
        let mut irq: c_int = 0;
        let r = platform::sys_msix_alloc(&mut irq);
        if r != 0 { None } else { Some(irq) }
    }
}

pub fn msix_free_irq(irq: c_int) -> c_int {
    unsafe { platform::sys_msix_free(irq) }
}

pub fn irq_set_priority(irq: c_int, priority: c_int) -> c_int {
    unsafe { platform::sys_irqthread_priority(irq, priority) }
}

pub fn msix_setup(irq: c_int) -> Option<c_int> {
    unsafe {
        let mut hook_id: c_int = 0;
        let r = platform::sys_msix_setpolicy(irq, 0, &mut hook_id);
        if r != 0 { None } else { Some(hook_id) }
    }
}

pub fn udelay(us: u32) { unsafe { platform::micro_delay(us) } }

#[inline]
pub unsafe fn read32_raw(addr: usize) -> u32 {
    core::ptr::read_volatile(addr as *const u32)
}

#[inline]
pub unsafe fn write32_raw(addr: usize, val: u32) {
    core::ptr::write_volatile(addr as *mut u32, val)
}

#[inline]
pub unsafe fn read64_raw(addr: usize) -> u64 {
    core::ptr::read_volatile(addr as *const u64)
}

#[inline]
pub unsafe fn write64_raw(addr: usize, val: u64) {
    core::ptr::write_volatile(addr as *mut u64, val)
}

pub fn print(msg: &[u8]) {
    unsafe {
        let fmt = b"%s\n\0".as_ptr() as *const c_char;
        platform::printf(fmt, msg.as_ptr() as *const c_char);
    }
}

pub use platform::{
    OK, EIO, ENXIO, ENOMEM, EACCES, EBUSY, EINVAL, ENOTTY,
    CDEV_OPEN, CDEV_CLOSE, CDEV_READ, CDEV_WRITE, CDEV_IOCTL, CDEV_SELECT,
    BDEV_OPEN, BDEV_CLOSE, BDEV_READ, BDEV_WRITE, BDEV_GATHER, BDEV_SCATTER, BDEV_IOCTL,
    NOTIFY_MESSAGE, HARDWARE, CLOCK,
    EAGAIN, EPROTO, ANY,
};

pub use platform::PciMsixInfo;
pub use platform::Blockdriver;
pub use platform::Chardriver;
pub use platform::SefInitFreshFn;
pub use platform::SefSignalHandlerFn;

pub fn driver_panic(msg: &[u8]) -> ! {
    print(msg);
    loop {}
}

pub fn env_parse_long(name: &[u8], default: c_long, min: c_long, max: c_long) -> c_long {
    unsafe {
        let mut val: c_long = default;
        let cname = name.as_ptr() as *const c_char;
        let fmt = b"d\0".as_ptr() as *const c_char;
        platform::env_parse(cname, fmt, 0, &mut val, min, max);
        val
    }
}

pub fn sef_set_init_fresh(cb: SefInitFreshFn) {
    #[cfg(target_os = "minix")]
    unsafe { platform::sef_setcb_init_fresh(Some(cb)); }
    #[cfg(not(target_os = "minix"))]
    unsafe { platform::sef_setcb_init_fresh(cb as *mut c_void); }
}

pub fn sef_set_signal_handler(cb: SefSignalHandlerFn) {
    #[cfg(target_os = "minix")]
    unsafe { platform::sef_setcb_signal_handler(Some(cb)); }
    #[cfg(not(target_os = "minix"))]
    unsafe { platform::sef_setcb_signal_handler(cb as *mut c_void); }
}

pub fn sef_startup_ffi() { unsafe { platform::sef_startup() } }
pub fn env_setargs_ffi(argc: c_int, argv: *mut *mut c_char) {
    unsafe { platform::env_setargs(argc, argv) }
}

pub fn blockdriver_support_lu() {
    // Stub — no MINIX multi-threaded blockdriver in xHCI yet
}

/// Real `blockdriver_announce()` — publishes DS_DRIVER_UP event so VFS/RS
/// knows this driver is ready to receive BDEV messages.
///
/// Equivalent to:
///   blockdriver_announce(int type) { /* libblockdriver/driver.c */
///       if (type == SEF_INIT_RESTART)
///           sys_statectl(SYS_STATE_CLEAR_IPC_REFS);
///       ds_retrieve_label_name(label, sef_self());
///       snprintf(key, "drv.blk.%s", label);
///       ds_publish_u32(key, DS_DRIVER_UP, DSF_OVERWRITE);
///   }
pub fn blockdriver_announce_ffi(type_: c_int) {
    #[cfg(not(target_os = "minix"))]
    {
        let _ = type_;
        return; // stub on host
    }

    #[cfg(target_os = "minix")]
    {
        // 1. On restart: clear stale IPC references so blocked callers
        //    don't wait forever on a dead driver instance.
        if type_ == 2 /* SEF_INIT_RESTART */ {
            unsafe {
                platform::sys_statectl(
                    platform::SYS_STATE_CLEAR_IPC_REFS,
                    core::ptr::null_mut(),
                    0,
                );
            }
        }

        // 2. Retrieve own label from DS
        let mut label = [0i8; 80];
        let r = unsafe {
            platform::ds_retrieve_label_name(
                label.as_mut_ptr(),
                platform::sef_self(),
            )
        };
        if r != 0 {
            return;
        }

        // 3. Find null terminator for label length
        let label_len = label.iter().position(|&c| c == 0).unwrap_or(label.len());
        if label_len == 0 {
            return;
        }

        // 4. Build key "drv.blk.<label>"
        let prefix = b"drv.blk.";
        let mut key = [0i8; 96];
        let prefix_len = prefix.len();
        key[..prefix_len].copy_from_slice(&prefix.map(|b| b as i8));
        let copy_len = core::cmp::min(label_len, 96usize.saturating_sub(prefix_len));
        key[prefix_len..prefix_len + copy_len]
            .copy_from_slice(&label[..copy_len]);
        key[prefix_len + copy_len] = 0; // null-terminate

        // 5. Publish DS_DRIVER_UP event
        unsafe {
            platform::ds_publish_u32(
                key.as_ptr(),
                platform::DS_DRIVER_UP,
                platform::DSF_OVERWRITE,
            );
        }
    }
}

pub fn blockdriver_terminate() {
    // Stub — no special cleanup needed yet
}

pub fn blockdriver_task(_bdp: &Blockdriver) {
    // Stub — main loop is handled by MINIX SEF/blockdriver
}

pub fn chardriver_task(cdp: &Chardriver) {
    unsafe { platform::cdr_task(cdp as *const Chardriver) }
}

pub fn chardriver_announce(type_: c_int) {
    unsafe { platform::cdr_announce(type_) }
}

pub fn chardriver_terminate() {
    unsafe { platform::cdr_terminate() }
}

pub fn receive_status() -> (Message, c_int) {
    let mut msg = Message::zero();
    let mut status: c_int = 0;
    let r = unsafe {
        platform::sef_receive_status(
            platform::ANY,
            &mut msg as *mut Message as *mut u8,
            &mut status,
        )
    };
    if r != 0 {
        status = r;
    }
    (msg, status)
}

pub fn reply(dest: c_int, msg: &Message) -> c_int {
    unsafe {
        platform::send(dest, msg as *const Message as *const u8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errno_constants() {
        assert_eq!(EIO, -5);
        assert_eq!(ENXIO, -6);
        assert_eq!(ENOMEM, -12);
    }

    #[test]
    fn pci_stubs_work() {
        assert_eq!(pci_init_ffi(), 0);
        assert!(pci_first_dev_ffi().is_none());
        pci_reserve_ffi(0);
    }

    #[test]
    fn raw_read_write_32() {
        let mut val: u32 = 0;
        unsafe {
            write32_raw(&mut val as *mut u32 as usize, 0xDEAD_BEEF);
            assert_eq!(read32_raw(&mut val as *mut u32 as usize), 0xDEAD_BEEF);
        }
    }
}
