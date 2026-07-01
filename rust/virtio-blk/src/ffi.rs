//! # FFI — MINIX C system call bindings for virtio-blk driver
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
pub(crate) mod platform {
    use super::*;

    unsafe extern "C" {
        // PCI
        pub fn pci_init() -> c_int;
        pub fn pci_first_dev(devindp: *mut c_int, vidp: *mut u16, didp: *mut u16) -> c_int;
        pub fn pci_next_dev(devindp: *mut c_int, vidp: *mut u16, didp: *mut u16) -> c_int;
        pub fn pci_reserve(devind: c_int);
        pub fn pci_get_bar(devind: c_int, bar: c_int, base: *mut u32,
            size_: *mut u32, ioflag: *mut c_int) -> c_int;
        pub fn pci_attr_r8(devind: c_int, offset: c_int) -> u8;
        pub fn pci_attr_r16(devind: c_int, offset: c_int) -> u16;

        // Port-mapped I/O (virtio uses I/O BAR)
        pub fn sys_inb(port: u16, value: *mut u32) -> c_int;
        pub fn sys_inw(port: u16, value: *mut u32) -> c_int;
        pub fn sys_inl(port: u16, value: *mut u32) -> c_int;
        pub fn sys_outb(port: u16, value: u8) -> c_int;
        pub fn sys_outw(port: u16, value: u16) -> c_int;
        pub fn sys_outl(port: u16, value: u32) -> c_int;

        // Physical memory
        pub fn alloc_contig(size: usize, flags: c_int, phys: *mut u64) -> *mut c_void;
        pub fn free_contig(addr: *mut c_void, size: usize);

        // VM
        pub fn vm_map_phys(endpt: c_int, base: *mut c_void, size: usize) -> *mut c_void;
        pub fn vm_unmap_phys(endpt: c_int, base: *mut c_void, size: usize) -> c_int;

        // IRQ
        pub fn sys_irqsetpolicy(irq: c_int, policy: c_int, hook_id: *mut c_int) -> c_int;
        pub fn sys_irqenable(hook_id: *mut c_int) -> c_int;
        pub fn sys_irqrmpolicy(hook_id: *mut c_int) -> c_int;

        // SEF
        pub fn sef_setcb_init_fresh(cb: Option<SefInitFreshFn>);
        pub fn sef_setcb_signal_handler(cb: Option<SefSignalHandlerFn>);
        pub fn sef_startup();
        pub fn env_setargs(argc: c_int, argv: *mut *mut c_char);
        pub fn env_parse(name: *const c_char, fmt: *const c_char, field: c_int,
            val: *mut c_long, min: c_long, max: c_long) -> c_int;

        // Blockdriver multi-threaded
        pub fn blockdriver_mt_task(bdp: *const c_void);
        pub fn blockdriver_mt_support_lu();
        pub fn blockdriver_announce(type_: c_int);
        pub fn blockdriver_mt_wakeup(tid: c_int);
        pub fn blockdriver_mt_sleep();
        pub fn blockdriver_mt_get_tid() -> c_int;
        pub fn blockdriver_mt_set_workers(device_id: c_int, nr: c_int);
        pub fn blockdriver_mt_terminate();

        // System
        pub fn sys_hz() -> u64;
        pub fn micro_delay(us: c_uint);

        // Debug
        pub fn printf(fmt: *const c_char, arg1: *const c_char) -> c_int;

        // Safe copy for ioctl
        pub fn sys_safecopyto(proc: c_int, grant: c_int, offset: c_ulong,
            buf: *const c_void, bytes: c_ulong) -> c_int;

        // sys_vumap — map caller buffers to physical addresses
        pub fn sys_vumap(
            endpt: c_int,
            vvec: *const VumapVir,
            vcount: c_int,
            offset: c_ulong,
            access: c_int,
            pvec: *mut VumapPhys,
            pcount: *mut c_int,
        ) -> c_int;

    }

    // Access flags for sys_vumap
    pub const VUA_READ: c_int = 1;
    pub const VUA_WRITE: c_int = 2;

    pub use super::VumapVir;
    pub use super::VumapPhys;
    pub use super::IoVec;

    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

    pub type DevMinor = c_int;
    pub const SELF: c_int = -0x100;
    pub const AC_ALIGN4K: c_int = 1;
    pub const PAGE_SIZE: usize = 4096;

    // errno constants
    pub const OK: c_int = 0;
    pub const EPERM: c_int = -1;
    pub const ENOENT: c_int = -2;
    pub const EIO: c_int = -5;
    pub const ENXIO: c_int = -6;
    pub const ENOMEM: c_int = -12;
    pub const EACCES: c_int = -13;
    pub const EBUSY: c_int = -16;
    pub const EINVAL: c_int = -22;
    pub const ENOTTY: c_int = -25;
    pub const EOPNOTSUPP: c_int = -95;
    pub const ENOTSUP: c_int = -96;

    #[repr(C)]
    pub struct Blockdriver {
        pub bdr_type: c_int,
        pub bdr_open: Option<unsafe extern "C" fn(DevMinor, c_int) -> c_int>,
        pub bdr_close: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
        pub bdr_transfer: Option<unsafe extern "C" fn(
            DevMinor, c_int, u64, c_int, *mut c_void, c_uint, c_int) -> isize>,
        pub bdr_ioctl: Option<unsafe extern "C" fn(
            DevMinor, c_ulong, c_int, c_int, c_int) -> c_int>,
        pub bdr_cleanup: Option<unsafe extern "C" fn()>,
        pub bdr_part: Option<unsafe extern "C" fn(DevMinor) -> *mut c_void>,
        pub bdr_geometry: Option<unsafe extern "C" fn(DevMinor, *mut c_void)>,
        pub bdr_intr: Option<unsafe extern "C" fn(c_uint)>,
        pub bdr_alarm: Option<unsafe extern "C" fn(u64)>,
        pub bdr_other: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
        pub bdr_device: Option<unsafe extern "C" fn(DevMinor, *mut c_int) -> c_int>,
    }
}

#[cfg(not(target_os = "minix"))]
pub(crate) mod platform {
    use super::*;

    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

    pub type DevMinor = c_int;
    pub const SELF: c_int = -0x100;
    pub const AC_ALIGN4K: c_int = 1;
    pub const PAGE_SIZE: usize = 4096;

    pub const OK: c_int = 0;
    pub const EPERM: c_int = -1;
    pub const ENOENT: c_int = -2;
    pub const EIO: c_int = -5;
    pub const ENXIO: c_int = -6;
    pub const ENOMEM: c_int = -12;
    pub const EACCES: c_int = -13;
    pub const EBUSY: c_int = -16;
    pub const EINVAL: c_int = -22;
    pub const ENOTTY: c_int = -25;
    pub const EOPNOTSUPP: c_int = -95;
    pub const ENOTSUP: c_int = -96;

    #[repr(C)]
    pub struct Blockdriver {
        pub bdr_type: c_int,
        pub bdr_open: Option<unsafe extern "C" fn(DevMinor, c_int) -> c_int>,
        pub bdr_close: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
        pub bdr_transfer: Option<unsafe extern "C" fn(
            DevMinor, c_int, u64, c_int, *mut c_void, c_uint, c_int) -> isize>,
        pub bdr_ioctl: Option<unsafe extern "C" fn(
            DevMinor, c_ulong, c_int, c_int, c_int) -> c_int>,
        pub bdr_cleanup: Option<unsafe extern "C" fn()>,
        pub bdr_part: Option<unsafe extern "C" fn(DevMinor) -> *mut c_void>,
        pub bdr_geometry: Option<unsafe extern "C" fn(DevMinor, *mut c_void)>,
        pub bdr_intr: Option<unsafe extern "C" fn(c_uint)>,
        pub bdr_alarm: Option<unsafe extern "C" fn(u64)>,
        pub bdr_other: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
        pub bdr_device: Option<unsafe extern "C" fn(DevMinor, *mut c_int) -> c_int>,
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

    pub unsafe fn sys_inb(_port: u16, _value: *mut u32) -> c_int {
        unsafe { *(_value as *mut u32) = 0xFF; 0 }
    }
    pub unsafe fn sys_inw(_port: u16, _value: *mut u32) -> c_int {
        unsafe { *(_value as *mut u32) = 0xFFFF; 0 }
    }
    pub unsafe fn sys_inl(_port: u16, _value: *mut u32) -> c_int {
        unsafe { *(_value as *mut u32) = 0xFFFFFFFF; 0 }
    }
    pub unsafe fn sys_outb(_port: u16, _value: u8) -> c_int { 0 }
    pub unsafe fn sys_outw(_port: u16, _value: u16) -> c_int { 0 }
    pub unsafe fn sys_outl(_port: u16, _value: u32) -> c_int { 0 }

    pub unsafe fn alloc_contig(_size: usize, _flags: c_int, _phys: *mut u64) -> *mut c_void {
        core::ptr::null_mut()
    }
    pub unsafe fn free_contig(_addr: *mut c_void, _size: usize) {}
    pub unsafe fn vm_map_phys(_endpt: c_int, _base: *mut c_void, _size: usize) -> *mut c_void {
        core::ptr::null_mut()
    }
    pub unsafe fn vm_unmap_phys(_endpt: c_int, _base: *mut c_void, _size: usize) -> c_int { -1 }

    pub unsafe fn sys_irqsetpolicy(_irq: c_int, _policy: c_int, _hook_id: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_irqenable(_hook_id: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_irqrmpolicy(_hook_id: *mut c_int) -> c_int { -1 }

    pub unsafe fn sef_setcb_init_fresh(_cb: *mut c_void) {}
    pub unsafe fn sef_setcb_signal_handler(_cb: *mut c_void) {}
    pub unsafe fn sef_startup() {}
    pub unsafe fn env_setargs(_argc: c_int, _argv: *mut *mut c_char) {}
    pub unsafe fn env_parse(_name: *const c_char, _fmt: *const c_char, _field: c_int,
        _val: *mut c_long, _min: c_long, _max: c_long) -> c_int { 0 }

    pub unsafe fn blockdriver_mt_task(_bdp: *const c_void) {}
    pub unsafe fn blockdriver_mt_support_lu() {}
    pub unsafe fn blockdriver_announce(_type_: c_int) {}
    pub unsafe fn blockdriver_mt_wakeup(_tid: c_int) {}
    pub unsafe fn blockdriver_mt_sleep() {}
    pub unsafe fn blockdriver_mt_get_tid() -> c_int { 0 }
    pub unsafe fn blockdriver_mt_set_workers(_device_id: c_int, _nr: c_int) {}
    pub unsafe fn blockdriver_mt_terminate() {}

    pub unsafe fn sys_hz() -> u64 { 100 }
    pub unsafe fn micro_delay(_us: c_uint) {}

    pub unsafe fn printf(_fmt: *const c_char, _arg: *const c_char) -> c_int { 0 }
    pub unsafe fn sys_safecopyto(_proc: c_int, _grant: c_int, _offset: c_ulong,
        _buf: *const c_void, _bytes: c_ulong) -> c_int { -1 }

    pub const VUA_READ: c_int = 1;
    pub const VUA_WRITE: c_int = 2;

    pub unsafe fn sys_vumap(
        _endpt: c_int,
        _vvec: *const VumapVir,
        _vcount: c_int,
        _offset: c_ulong,
        _access: c_int,
        _pvec: *mut VumapPhys,
        _pcount: *mut c_int,
    ) -> c_int { -1 }
}

/// Virtual memory vector for sys_vumap (input)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VumapVir {
    pub vv_addr: u64,
    pub vv_size: u64,
}

/// Physical memory vector for sys_vumap (output)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VumapPhys {
    pub vp_addr: u64,
    pub vp_size: u64,
}

/// I/O vector from blockdriver framework (iovec_s_t)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IoVec {
    pub iov_grant: c_int,
    pub iov_size: usize,
}

// ============================================================================
// Public API wrappers
// ============================================================================

pub type DevMinor = platform::DevMinor;

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

// Port I/O helpers for virtio
pub unsafe fn port_inb(port: u16) -> u8 {
    let mut val: u32 = 0;
    let _ = platform::sys_inb(port, &mut val);
    val as u8
}

pub unsafe fn port_inw(port: u16) -> u16 {
    let mut val: u32 = 0;
    let _ = platform::sys_inw(port, &mut val);
    val as u16
}

pub unsafe fn port_inl(port: u16) -> u32 {
    let mut val: u32 = 0;
    let _ = platform::sys_inl(port, &mut val);
    val
}

pub unsafe fn port_outb(port: u16, val: u8) {
    let _ = platform::sys_outb(port, val);
}

pub unsafe fn port_outw(port: u16, val: u16) {
    let _ = platform::sys_outw(port, val);
}

pub unsafe fn port_outl(port: u16, val: u32) {
    let _ = platform::sys_outl(port, val);
}

// Physical memory
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

// IRQ
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

pub fn irq_reenable(hook_id: &c_int) -> c_int {
    unsafe { platform::sys_irqenable(hook_id as *const c_int as *mut c_int) }
}

pub fn irq_remove(hook_id: &mut c_int) -> c_int {
    unsafe { platform::sys_irqrmpolicy(hook_id) }
}

// SEF
pub fn sef_set_init_fresh(cb: platform::SefInitFreshFn) {
    #[cfg(target_os = "minix")]
    unsafe { platform::sef_setcb_init_fresh(Some(cb)); }
    #[cfg(not(target_os = "minix"))]
    unsafe { platform::sef_setcb_init_fresh(cb as *mut c_void); }
}

pub fn sef_set_signal_handler(cb: platform::SefSignalHandlerFn) {
    #[cfg(target_os = "minix")]
    unsafe { platform::sef_setcb_signal_handler(Some(cb)); }
    #[cfg(not(target_os = "minix"))]
    unsafe { platform::sef_setcb_signal_handler(cb as *mut c_void); }
}

pub fn sef_startup_ffi() { unsafe { platform::sef_startup() } }
pub fn env_setargs_ffi(argc: c_int, argv: *mut *mut c_char) {
    unsafe { platform::env_setargs(argc, argv) }
}

pub fn env_parse_long(name: &[u8], default: c_long, min: c_long, max: c_long) -> c_long {
    unsafe {
        let mut val: c_long = default;
        let cname = name.as_ptr() as *const c_char;
        platform::env_parse(cname, b"d\0".as_ptr() as *const c_char, 0, &mut val, min, max);
        val
    }
}

pub use platform::Blockdriver;

// Blockdriver MT wrappers
pub fn blockdriver_task(bdp: &Blockdriver) {
    unsafe { platform::blockdriver_mt_task(bdp as *const Blockdriver as *const c_void) }
}

pub fn blockdriver_support_lu() {
    unsafe { platform::blockdriver_mt_support_lu() }
}

pub fn blockdriver_announce_ffi(type_: c_int) {
    unsafe { platform::blockdriver_announce(type_) }
}

pub fn blockdriver_wakeup(tid: c_int) { unsafe { platform::blockdriver_mt_wakeup(tid) } }
pub fn blockdriver_sleep() { unsafe { platform::blockdriver_mt_sleep() } }
pub fn blockdriver_get_tid() -> c_int { unsafe { platform::blockdriver_mt_get_tid() } }
pub fn blockdriver_set_workers(device_id: c_int, nr: c_int) {
    unsafe { platform::blockdriver_mt_set_workers(device_id, nr) }
}
pub fn blockdriver_terminate() { unsafe { platform::blockdriver_mt_terminate() } }

pub fn get_sys_hz() -> u64 { unsafe { platform::sys_hz() } }
pub fn udelay(us: u32) { unsafe { platform::micro_delay(us) } }

pub fn print(msg: &[u8]) {
    unsafe {
        let fmt = b"%s\n\0".as_ptr() as *const c_char;
        platform::printf(fmt, msg.as_ptr() as *const c_char);
    }
}

pub fn sys_safecopyto_ffi(proc: c_int, grant: c_int, offset: c_ulong,
    buf: *const c_void, bytes: c_ulong) -> c_int
{
    unsafe { platform::sys_safecopyto(proc, grant, offset, buf, bytes) }
}

pub fn sys_vumap_ffi(
    endpt: c_int,
    vvec: *const VumapVir,
    vcount: c_int,
    offset: c_ulong,
    access: c_int,
    pvec: *mut VumapPhys,
    pcount: *mut c_int,
) -> c_int {
    unsafe { platform::sys_vumap(endpt, vvec, vcount, offset, access, pvec, pcount) }
}

/// part_geom struct (MINIX partition geometry)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PartGeom {
    pub base: u64,
    pub size: u64,
    pub cylinders: c_uint,
    pub heads: c_uint,
    pub sectors: c_uint,
}

pub use platform::{VUA_READ, VUA_WRITE};

pub fn millis_to_ticks(ms: u64) -> u64 {
    let hz = get_sys_hz();
    (ms * hz + 999) / 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errno_constants() {
        assert_eq!(platform::EIO, -5);
        assert_eq!(platform::ENXIO, -6);
        assert_eq!(platform::ENOMEM, -12);
        assert_eq!(platform::EINVAL, -22);
    }

    #[test]
    fn pci_stubs_work() {
        assert_eq!(pci_init_ffi(), 0);
        assert!(pci_first_dev_ffi().is_none());
        assert!(pci_next_dev_ffi().is_none());
        pci_reserve_ffi(0);
    }

    #[test]
    fn port_io_stubs_work() {
        unsafe {
            assert_eq!(port_inb(0), 0xFF);
            assert_eq!(port_inw(0), 0xFFFF);
            assert_eq!(port_inl(0), 0xFFFFFFFF);
        }
    }

    #[test]
    fn blockdriver_stubs_work() {
        blockdriver_set_workers(0, 1);
        blockdriver_terminate();
        assert_eq!(blockdriver_get_tid(), 0);
    }
}
