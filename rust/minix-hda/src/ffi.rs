//! # FFI — MINIX C system call bindings for the Intel HDA Audio driver
//!
//! Dual-platform: real MINIX extern blocks + host stubs for cargo test.

#![allow(dead_code, unused_imports)]

use core::ffi::{c_char, c_int, c_uint, c_void};

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

        pub fn sef_setcb_init_fresh(cb: Option<SefInitFreshFn>);
        pub fn sef_setcb_signal_handler(cb: Option<SefSignalHandlerFn>);
        pub fn sef_startup();
        pub fn env_setargs(argc: c_int, argv: *mut *mut c_char);
        pub fn env_parse(name: *const c_char, fmt: *const c_char, field: c_int,
            val: *mut c_long, min: c_long, max: c_long) -> c_int;

        // Character device framework
        pub fn cdr_task(bdp: *const c_void);
        pub fn cdr_announce(type_: c_int);
        pub fn cdr_terminate();

        pub fn sys_hz() -> u64;
        pub fn micro_delay(us: c_uint);

        pub fn printf(fmt: *const c_char, arg: *const c_char) -> c_int;
        pub fn sys_safecopyto(proc: c_int, grant: c_int, offset: c_ulong,
            buf: *const c_void, bytes: c_ulong) -> c_int;
        pub fn sys_safecopyfrom(proc: c_int, grant: c_int, offset: c_ulong,
            buf: *mut c_void, bytes: c_ulong) -> c_int;
    }

    /// MSI-X capability info.
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
    pub const EINVAL: c_int = -22;
    pub const ENOTTY: c_int = -25;
    pub const EBUSY: c_int = -16;

    // Character driver types
    pub const CDEV_MAJOR: c_int = 44;  // Audio standard major number

    #[repr(C)]
    pub struct Chardriver {
        pub cdr_type: c_int,
        pub cdr_open: Option<unsafe extern "C" fn(DevMinor, c_int, endpoint_t) -> c_int>,
        pub cdr_close: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
        pub cdr_read: Option<unsafe extern "C" fn(
            DevMinor, u64, endpoint_t, cp_grant_id_t, size_t, c_int, cdev_id_t) -> isize>,
        pub cdr_write: Option<unsafe extern "C" fn(
            DevMinor, u64, endpoint_t, cp_grant_id_t, size_t, c_int, cdev_id_t) -> isize>,
        pub cdr_ioctl: Option<unsafe extern "C" fn(
            DevMinor, c_ulong, endpoint_t, cp_grant_id_t, c_int, c_ulong, cdev_id_t) -> c_int>,
    }

    pub type endpoint_t = c_int;
    pub type cp_grant_id_t = c_int;
    pub type cdev_id_t = c_int;
    pub type size_t = usize;
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
    pub const EINVAL: c_int = -22;
    pub const ENOTTY: c_int = -25;
    pub const EBUSY: c_int = -16;

    pub const CDEV_MAJOR: c_int = 44;
    pub type endpoint_t = c_int;
    pub type cp_grant_id_t = c_int;
    pub type cdev_id_t = c_int;
    pub type size_t = usize;

    #[repr(C)]
    pub struct PciMsixInfo {
        pub msix_table_size: c_int,
        pub msix_table_bir: c_int,
        pub msix_table_offset: u32,
        pub msix_pba_bir: c_int,
        pub msix_pba_offset: u32,
    }

    #[repr(C)]
    pub struct Chardriver {
        pub cdr_type: c_int,
        pub cdr_open: Option<unsafe extern "C" fn(DevMinor, c_int, endpoint_t) -> c_int>,
        pub cdr_close: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
        pub cdr_read: Option<unsafe extern "C" fn(
            DevMinor, u64, endpoint_t, cp_grant_id_t, size_t, c_int, cdev_id_t) -> isize>,
        pub cdr_write: Option<unsafe extern "C" fn(
            DevMinor, u64, endpoint_t, cp_grant_id_t, size_t, c_int, cdev_id_t) -> isize>,
        pub cdr_ioctl: Option<unsafe extern "C" fn(
            DevMinor, c_ulong, endpoint_t, cp_grant_id_t, c_int, c_ulong, cdev_id_t) -> c_int>,
    }

    // Stubs for host-side testing
    pub unsafe fn pci_init() -> c_int { 0 }
    pub unsafe fn pci_first_dev(_: *mut c_int, _: *mut u16, _: *mut u16) -> c_int { -1 }
    pub unsafe fn pci_next_dev(_: *mut c_int, _: *mut u16, _: *mut u16) -> c_int { -1 }
    pub unsafe fn pci_reserve(_: c_int) {}
    pub unsafe fn pci_get_bar(_: c_int, _: c_int, _: *mut u32, _: *mut u32, _: *mut c_int) -> c_int { -1 }
    pub unsafe fn pci_attr_r8(_: c_int, _: c_int) -> u8 { 0 }
    pub unsafe fn pci_attr_r16(_: c_int, _: c_int) -> u16 { 0 }
    pub unsafe fn pci_attr_r32(_: c_int, _: c_int) -> u32 { 0 }
    pub unsafe fn pci_attr_w16(_: c_int, _: c_int, _: u16) {}
    pub unsafe fn pci_find_cap(_: c_int, _: c_int, _: *mut c_int) -> c_int { 0 }
    pub unsafe fn pci_msix_parse(_: c_int, _: *mut PciMsixInfo) -> c_int { 0 }
    pub unsafe fn vm_map_phys(_: c_int, _: *mut c_void, _: usize) -> *mut c_void { ptr::null_mut() }
    pub unsafe fn vm_unmap_phys(_: c_int, _: *mut c_void, _: usize) -> c_int { -1 }
    pub unsafe fn alloc_contig(_: usize, _: c_int, _: *mut u64) -> *mut c_void { ptr::null_mut() }
    pub unsafe fn free_contig(_: *mut c_void, _: usize) {}
    pub unsafe fn sys_irqsetpolicy(_: c_int, _: c_int, _: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_irqenable(_: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_irqrmpolicy(_: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_msix_alloc(_: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_msix_free(_: c_int) -> c_int { -1 }
    pub unsafe fn sys_msix_setpolicy(_: c_int, _: c_int, _: *mut c_int) -> c_int { -1 }
    pub unsafe fn sef_setcb_init_fresh(_: *mut c_void) {}
    pub unsafe fn sef_setcb_signal_handler(_: *mut c_void) {}
    pub unsafe fn sef_startup() {}
    pub unsafe fn env_setargs(_: c_int, _: *mut *mut c_char) {}
    pub unsafe fn env_parse(_: *const c_char, _: *const c_char, _: c_int,
        _: *mut c_long, _: c_long, _: c_long) -> c_int { 0 }
    pub unsafe fn cdr_task(_: *const c_void) {}
    pub unsafe fn cdr_announce(_: c_int) {}
    pub unsafe fn cdr_terminate() {}
    pub unsafe fn sys_hz() -> u64 { 100 }
    pub unsafe fn micro_delay(_: c_uint) {}
    pub unsafe fn printf(_: *const c_char, _: *const c_char) -> c_int { 0 }
    pub unsafe fn sys_safecopyto(_: c_int, _: c_int, _: c_ulong,
        _: *const c_void, _: c_ulong) -> c_int { -1 }
    pub unsafe fn sys_safecopyfrom(_: c_int, _: c_int, _: c_ulong,
        _: *mut c_void, _: c_ulong) -> c_int { -1 }
}

// ============================================================================
// Public API wrappers
// ============================================================================

pub type DevMinor = platform::DevMinor;
pub type endpoint_t = platform::endpoint_t;
pub type cp_grant_id_t = platform::cp_grant_id_t;
pub type cdev_id_t = platform::cdev_id_t;
pub type size_t = platform::size_t;

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

pub fn msix_setup(irq: c_int) -> Option<c_int> {
    unsafe {
        let mut hook_id: c_int = 0;
        let r = platform::sys_msix_setpolicy(irq, 0, &mut hook_id);
        if r != 0 { None } else { Some(hook_id) }
    }
}

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

pub use platform::Chardriver;

pub fn chardriver_task(cdp: &Chardriver) {
    unsafe { platform::cdr_task(cdp as *const Chardriver as *const c_void) }
}

pub fn chardriver_announce(type_: c_int) {
    unsafe { platform::cdr_announce(type_) }
}

pub fn chardriver_terminate() { unsafe { platform::cdr_terminate() } }

pub fn get_sys_hz() -> u64 { unsafe { platform::sys_hz() } }
pub fn udelay(us: u32) { unsafe { platform::micro_delay(us) } }

pub fn millis_to_ticks(ms: u64) -> u64 {
    let hz = get_sys_hz();
    (ms * hz + 999) / 1000
}

#[inline]
pub unsafe fn read32_raw(addr: usize) -> u32 {
    core::ptr::read_volatile(addr as *const u32)
}

#[inline]
pub unsafe fn write32_raw(addr: usize, val: u32) {
    core::ptr::write_volatile(addr as *mut u32, val)
}

#[inline]
pub unsafe fn read16_raw(addr: usize) -> u16 {
    core::ptr::read_volatile(addr as *const u16)
}

#[inline]
pub unsafe fn write16_raw(addr: usize, val: u16) {
    core::ptr::write_volatile(addr as *mut u16, val)
}

#[inline]
pub unsafe fn read64_raw(addr: usize) -> u64 {
    core::ptr::read_volatile(addr as *const u64)
}

pub fn print(msg: &[u8]) {
    unsafe {
        let fmt = b"%s\n\0".as_ptr() as *const c_char;
        platform::printf(fmt, msg.as_ptr() as *const c_char);
    }
}

pub fn driver_panic(msg: &[u8]) -> ! {
    print(msg);
    loop {}
}

pub fn sys_safecopyto_ffi(proc: c_int, grant: c_int, offset: c_ulong,
    buf: *const c_void, bytes: c_ulong) -> c_int
{
    unsafe { platform::sys_safecopyto(proc, grant, offset, buf, bytes) }
}

pub fn sys_safecopyfrom_ffi(proc: c_int, grant: c_int, offset: c_ulong,
    buf: *mut c_void, bytes: c_ulong) -> c_int
{
    unsafe { platform::sys_safecopyfrom(proc, grant, offset, buf, bytes) }
}

pub use platform::{OK, EIO, ENXIO, ENOMEM, EINVAL, ENOTTY, EBUSY};
pub use platform::{CDEV_MAJOR, PciMsixInfo};
pub use core::ffi::c_ulong;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errno_constants() {
        assert_eq!(EIO, -5);
        assert_eq!(ENXIO, -6);
        assert_eq!(ENOMEM, -12);
        assert_eq!(EINVAL, -22);
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

    #[test]
    fn raw_read_write_16() {
        let mut val: u16 = 0;
        unsafe {
            write16_raw(&mut val as *mut u16 as usize, 0xABCD);
            assert_eq!(read16_raw(&mut val as *mut u16 as usize), 0xABCD);
        }
    }
}
