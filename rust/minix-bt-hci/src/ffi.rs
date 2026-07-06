//! # FFI — MINIX C system call bindings for the Bluetooth HCI USB driver
//!
//! Dual-platform: real MINIX extern blocks + host stubs for cargo test.

#![allow(dead_code, unused_imports)]

use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};

// c_long is not in core::ffi — define it based on pointer width.
#[cfg(target_pointer_width = "64")]
pub type c_long = i64;
#[cfg(target_pointer_width = "32")]
pub type c_long = i32;

// ============================================================================
// Platform selection
// ============================================================================

#[cfg(target_os = "minix")]
pub(crate) mod platform {
    use super::{c_char, c_int, c_uint, c_ulong, c_void, c_long};
    use core::ptr;

    pub type DevMinor = c_int;

    pub const SELF: c_int = -0x100;
    pub const AC_ALIGN4K: c_int = 1;
    pub const PAGE_SIZE: usize = 4096;

    // Error constants
    pub const OK: c_int = 0;
    pub const EIO: c_int = -5;
    pub const ENXIO: c_int = -6;
    pub const ENOMEM: c_int = -12;
    pub const EACCES: c_int = -13;
    pub const EBUSY: c_int = -16;
    pub const EINVAL: c_int = -22;
    pub const ENOTTY: c_int = -25;
    pub const EAGAIN: c_int = -35;
    pub const EWOULDBLOCK: c_int = -35;
    pub const EPROTO: c_int = -71;

    // Chardriver structure (MINIX chardriver.h)
    #[repr(C)]
    pub struct Chardriver {
        pub cdr_type: c_int,
        pub cdr_open: Option<unsafe extern "C" fn(DevMinor, c_int, endpoint_t) -> c_int>,
        pub cdr_close: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
        pub cdr_read: Option<unsafe extern "C" fn(
            DevMinor, u64, endpoint_t, cp_grant_id_t, usize, c_int, cdev_id_t) -> isize>,
        pub cdr_write: Option<unsafe extern "C" fn(
            DevMinor, u64, endpoint_t, cp_grant_id_t, usize, c_int, cdev_id_t) -> isize>,
        pub cdr_ioctl: Option<unsafe extern "C" fn(
            DevMinor, c_ulong, endpoint_t, cp_grant_id_t, c_int, endpoint_t) -> c_int>,
        pub cdr_select: Option<unsafe extern "C" fn(
            DevMinor, c_uint, endpoint_t) -> c_int>,
        pub cdr_intr: Option<unsafe extern "C" fn(c_uint)>,
        pub cdr_alarm: Option<unsafe extern "C" fn(u64)>,
        pub cdr_other: Option<unsafe extern "C" fn(*const c_void, *mut c_void)>,
        pub cdr_device: Option<unsafe extern "C" fn(DevMinor, *mut c_int) -> c_int>,
        pub cdr_signal: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
    }

    // MINIX endpoint type
    pub type endpoint_t = c_int;
    pub type cp_grant_id_t = c_int;
    pub type cdev_id_t = c_int;

    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

    // External C functions
    unsafe extern "C" {
        // SEF lifecycle
        pub fn sef_setcb_init_fresh(cb: Option<SefInitFreshFn>);
        pub fn sef_setcb_signal_handler(cb: Option<SefSignalHandlerFn>);
        pub fn sef_startup();

        // Chardriver
        pub fn cdr_task(cdp: *const Chardriver);
        pub fn cdr_announce(type_: c_int);
        pub fn cdr_terminate();

        // Environment
        pub fn env_setargs(argc: c_int, argv: *mut *mut c_char);
        pub fn env_parse(name: *const c_char, fmt: *const c_char,
            field: c_int, val: *mut c_long, min: c_long, max: c_long) -> c_int;

        // PCI
        pub fn pci_init() -> c_int;
        pub fn pci_first_dev(devindp: *mut c_int, vidp: *mut u16, didp: *mut u16) -> c_int;
        pub fn pci_next_dev(devindp: *mut c_int, vidp: *mut u16, didp: *mut u16) -> c_int;
        pub fn pci_reserve(devind: c_int);
        pub fn pci_get_bar(devind: c_int, bar: c_int,
            base: *mut u32, size_: *mut u32, ioflag: *mut c_int) -> c_int;
        pub fn pci_attr_r8(devind: c_int, offset: c_int) -> u8;
        pub fn pci_attr_r16(devind: c_int, offset: c_int) -> u16;
        pub fn pci_attr_r32(devind: c_int, offset: c_int) -> u32;
        pub fn pci_find_cap(devind: c_int, cap_id: c_int, cap_ptr: *mut c_int) -> c_int;

        // Memory
        pub fn vm_map_phys(endpt: c_int, base: *mut c_void, size: usize) -> *mut c_void;
        pub fn vm_unmap_phys(endpt: c_int, base: *mut c_void, size: usize) -> c_int;
        pub fn alloc_contig(size: usize, flags: c_int, phys: *mut u64) -> *mut c_void;
        pub fn free_contig(addr: *mut c_void, size: usize);

        // IRQ
        pub fn sys_irqsetpolicy(irq: c_int, policy: c_int, hook_id: *mut c_int) -> c_int;
        pub fn sys_irqenable(hook_id: *mut c_int) -> c_int;
        pub fn sys_irqrmpolicy(hook_id: *mut c_int) -> c_int;
        pub fn sys_msix_alloc(irq: *mut c_int) -> c_int;
        pub fn sys_msix_free(irq: c_int) -> c_int;
        pub fn sys_msix_setpolicy(irq: c_int, policy: c_int, hook_id: *mut c_int) -> c_int;
        pub fn sys_irqthread_priority(irq: c_int, priority: c_int) -> c_int;

        // Safecopy
        pub fn sys_safecopyto(proc_nr: c_int, grant: cp_grant_id_t, offset: c_ulong,
            buf: *const c_void, bytes: c_ulong) -> c_int;
        pub fn sys_safecopyfrom(proc_nr: c_int, grant: cp_grant_id_t, offset: c_ulong,
            buf: *mut c_void, bytes: c_ulong) -> c_int;

        // Timers
        pub fn sys_hz() -> u64;
        pub fn micro_delay(us: c_uint);

        // Console/log
        pub fn printf(fmt: *const c_char, ...) -> c_int;

        // Non-variadic wrapper for internal use
        pub fn kputchar(c: c_int) -> c_int;

        // Interrupt hook management
        pub fn sys_irqctl(hook_id: *mut c_int, request: c_int, irq: c_int,
            policy: c_int) -> c_int;
    }
}

#[cfg(not(target_os = "minix"))]
pub(crate) mod platform {
    use super::{c_char, c_int, c_uint, c_ulong, c_void, c_long};
    use core::ptr;

    pub type DevMinor = c_int;
    pub type endpoint_t = c_int;
    pub type cp_grant_id_t = c_int;
    pub type cdev_id_t = c_int;

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
    pub const EWOULDBLOCK: c_int = -35;
    pub const EPROTO: c_int = -71;

    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

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
    pub unsafe fn sef_setcb_init_fresh(_cb: *mut c_void) {}
    pub unsafe fn sef_setcb_signal_handler(_cb: *mut c_void) {}
    pub unsafe fn sef_startup() {}
    pub unsafe fn cdr_task(_cdp: *const Chardriver) {}
    pub unsafe fn cdr_announce(_type_: c_int) {}
    pub unsafe fn cdr_terminate() {}
    pub unsafe fn env_setargs(_argc: c_int, _argv: *mut *mut c_char) {}
    pub unsafe fn env_parse(_name: *const c_char, _fmt: *const c_char,
        _field: c_int, _val: *mut c_long, _min: c_long, _max: c_long) -> c_int { 0 }
    pub unsafe fn pci_init() -> c_int { 0 }
    pub unsafe fn pci_first_dev(_d: *mut c_int, _v: *mut u16, _p: *mut u16) -> c_int { -1 }
    pub unsafe fn pci_next_dev(_d: *mut c_int, _v: *mut u16, _p: *mut u16) -> c_int { -1 }
    pub unsafe fn pci_reserve(_d: c_int) {}
    pub unsafe fn pci_get_bar(_d: c_int, _b: c_int, _base: *mut u32,
        _s: *mut u32, _f: *mut c_int) -> c_int { -1 }
    pub unsafe fn pci_attr_r8(_d: c_int, _o: c_int) -> u8 { 0 }
    pub unsafe fn pci_attr_r16(_d: c_int, _o: c_int) -> u16 { 0 }
    pub unsafe fn pci_attr_r32(_d: c_int, _o: c_int) -> u32 { 0 }
    pub unsafe fn pci_find_cap(_d: c_int, _c: c_int, _p: *mut c_int) -> c_int { 0 }
    pub unsafe fn vm_map_phys(_e: c_int, _b: *mut c_void, _sz: usize) -> *mut c_void {
        ptr::null_mut()
    }
    pub unsafe fn vm_unmap_phys(_e: c_int, _b: *mut c_void, _sz: usize) -> c_int { -1 }
    pub unsafe fn alloc_contig(_sz: usize, _fl: c_int, _p: *mut u64) -> *mut c_void {
        ptr::null_mut()
    }
    pub unsafe fn free_contig(_a: *mut c_void, _sz: usize) {}
    pub unsafe fn sys_irqsetpolicy(_i: c_int, _p: c_int, _h: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_irqenable(_h: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_irqrmpolicy(_h: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_msix_alloc(_i: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_msix_free(_i: c_int) -> c_int { -1 }
    pub unsafe fn sys_msix_setpolicy(_i: c_int, _p: c_int, _h: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_irqthread_priority(_i: c_int, _p: c_int) -> c_int { -1 }
    pub unsafe fn sys_safecopyto(_p: c_int, _g: c_int, _o: c_ulong,
        _b: *const c_void, _by: c_ulong) -> c_int { -1 }
    pub unsafe fn sys_safecopyfrom(_p: c_int, _g: c_int, _o: c_ulong,
        _b: *mut c_void, _by: c_ulong) -> c_int { -1 }
    pub unsafe fn sys_hz() -> u64 { 100 }
    pub unsafe fn micro_delay(_us: c_uint) {}
    pub unsafe fn printf(_fmt: *const c_char, _arg: *const c_char) -> c_int { 0 }
    pub unsafe fn kputchar(_c: c_int) -> c_int { 0 }
    pub unsafe fn sys_irqctl(_h: *mut c_int, _rq: c_int, _irq: c_int, _po: c_int) -> c_int { -1 }
}

// ============================================================================
// Public API wrappers
// ============================================================================

pub use platform::{
    OK, EIO, ENXIO, ENOMEM, EACCES, EBUSY, EINVAL, ENOTTY, EAGAIN, EWOULDBLOCK, EPROTO,
    DevMinor, endpoint_t, cp_grant_id_t, cdev_id_t,
    Chardriver, SefInitFreshFn, SefSignalHandlerFn,
};

pub type CLong = isize;

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

pub fn vm_map_phys_ffi(base: *mut c_void, size: usize) -> *mut c_void {
    unsafe { platform::vm_map_phys(platform::SELF, base, size) }
}

pub fn vm_unmap_phys_ffi(base: *mut c_void, size: usize) -> c_int {
    unsafe { platform::vm_unmap_phys(platform::SELF, base, size) }
}

pub fn udelay(us: u32) { unsafe { platform::micro_delay(us) } }

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

pub fn set_irq_priority(irq: c_int, priority: c_int) -> c_int {
    unsafe { platform::sys_irqthread_priority(irq, priority) }
}

pub fn print(msg: &[u8]) {
    unsafe {
        // Pass the message as a C string to printf via %s format
        let fmt = b"%s\n\0".as_ptr() as *const c_char;
        let cstr = msg.as_ptr() as *const c_char;
        platform::printf(fmt, cstr);
    }
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

pub fn chardriver_task(cdp: &Chardriver) {
    unsafe { platform::cdr_task(cdp as *const Chardriver) }
}

pub fn chardriver_announce(type_: c_int) {
    unsafe { platform::cdr_announce(type_) }
}

pub fn chardriver_terminate() {
    unsafe { platform::cdr_terminate() }
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

/// Safe wrapper for sys_safecopyto — copies data FROM the kernel TO userspace.
pub fn sys_safecopyto_wrapper(
    proc_nr: c_int,
    grant: cp_grant_id_t,
    offset: c_ulong,
    buf: *const c_void,
    bytes: c_ulong,
) -> c_int {
    unsafe { platform::sys_safecopyto(proc_nr, grant, offset, buf, bytes) }
}

/// Safe wrapper for sys_safecopyfrom — copies data FROM userspace TO the kernel.
pub fn sys_safecopyfrom_wrapper(
    proc_nr: c_int,
    grant: cp_grant_id_t,
    offset: c_ulong,
    buf: *mut c_void,
    bytes: c_ulong,
) -> c_int {
    unsafe { platform::sys_safecopyfrom(proc_nr, grant, offset, buf, bytes) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errno_constants() {
        assert_eq!(OK, 0);
        assert_eq!(EIO, -5);
        assert_eq!(ENXIO, -6);
        assert_eq!(EAGAIN, -35);
    }

    #[test]
    fn pci_stubs_work() {
        assert_eq!(pci_init_ffi(), 0);
        assert!(pci_first_dev_ffi().is_none());
        pci_reserve_ffi(0);
    }

    #[test]
    fn parse_long_default() {
        // Test with stubs
        let val = env_parse_long(b"test\0", 42, 0, 100);
        assert_eq!(val, 42);
    }
}
