//! # FFI — MINIX C system call bindings for the AHCI driver
//!
//! All extern blocks use `unsafe extern "C"` (Rust 2024 edition).
//! Non-MINIX targets get stub implementations for host-side testing.

#![allow(dead_code, unused_imports)]

use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};

pub type CLong = isize;
pub use CLong as c_long;

// ============================================================================
// Platform selection: MINIX (real FFI) vs host (linkable stubs)
// ============================================================================

#[cfg(target_os = "minix")]
mod platform {
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

        pub fn vm_map_phys(endpt: c_int, base: *mut c_void, size: usize) -> *mut c_void;
        pub fn vm_unmap_phys(endpt: c_int, base: *mut c_void, size: usize) -> c_int;

        pub fn alloc_contig(size: usize, flags: c_int, phys: *mut u64) -> *mut c_void;
        pub fn free_contig(addr: *mut c_void, size: usize);

        pub fn sys_irqsetpolicy(irq: c_int, policy: c_int, hook_id: *mut c_int) -> c_int;
        pub fn sys_irqenable(hook_id: *mut c_int) -> c_int;
        pub fn sys_irqrmpolicy(hook_id: *mut c_int) -> c_int;

        pub fn sef_setcb_init_fresh(cb: Option<SefInitFreshFn>);
        pub fn sef_setcb_signal_handler(cb: Option<SefSignalHandlerFn>);
        pub fn sef_startup();
        pub fn env_setargs(argc: c_int, argv: *mut *mut c_char);
        pub fn env_parse(name: *const c_char, fmt: *const c_char, field: c_int,
            val: *mut c_long, min: c_long, max: c_long) -> c_int;

        pub fn blockdriver_mt_task(bdp: *const c_void);
        pub fn blockdriver_announce(type_: c_int);
        pub fn blockdriver_mt_wakeup(tid: c_int);
        pub fn blockdriver_mt_sleep();
        pub fn blockdriver_mt_get_tid() -> c_int;
        pub fn blockdriver_mt_set_workers(device_id: c_int, nr: c_int);
        pub fn blockdriver_mt_terminate();

        pub fn sys_hz() -> u64;
        pub fn micro_delay(us: c_uint);

        pub fn printf(fmt: *const c_char, arg1: *const c_char) -> c_int;
    }

    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

    pub type DevMinor = c_int;
    pub const SELF: c_int = -0x100;
    pub const AC_ALIGN4K: c_int = 1;

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
}

#[cfg(not(target_os = "minix"))]
mod platform {
    use super::*;
    use core::ptr;

    /// Stub implementations for host-side compilation. These provide real
    /// function bodies so the linker can resolve them during `cargo test`.

    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

    pub type DevMinor = c_int;
    pub const SELF: c_int = -0x100;
    pub const AC_ALIGN4K: c_int = 1;

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

    // SAFETY: stubs only used on host (non-MINIX) for cargo test; never called in production
    pub unsafe fn pci_init() -> c_int { 0 }
    pub unsafe fn pci_first_dev(_devindp: *mut c_int, _vidp: *mut u16, _didp: *mut u16) -> c_int { -1 }
    pub unsafe fn pci_next_dev(_devindp: *mut c_int, _vidp: *mut u16, _didp: *mut u16) -> c_int { -1 }
    pub unsafe fn pci_reserve(_devind: c_int) {}
    pub unsafe fn pci_get_bar(_devind: c_int, _bar: c_int, _base: *mut u32,
        _size_: *mut u32, _ioflag: *mut c_int) -> c_int { -1 }
    pub unsafe fn pci_attr_r8(_devind: c_int, _offset: c_int) -> u8 { 0 }
    pub unsafe fn pci_attr_r16(_devind: c_int, _offset: c_int) -> u16 { 0 }

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

    pub unsafe fn sef_setcb_init_fresh(_cb: *mut c_void) {}
    pub unsafe fn sef_setcb_signal_handler(_cb: *mut c_void) {}
    pub unsafe fn sef_startup() {}
    pub unsafe fn env_setargs(_argc: c_int, _argv: *mut *mut c_char) {}
    pub unsafe fn env_parse(_name: *const c_char, _fmt: *const c_char, _field: c_int,
        _val: *mut c_long, _min: c_long, _max: c_long) -> c_int { 0 }

    pub unsafe fn blockdriver_mt_task(_bdp: *const c_void) {}
    pub unsafe fn blockdriver_announce(_type_: c_int) {}
    pub unsafe fn blockdriver_mt_wakeup(_tid: c_int) {}
    pub unsafe fn blockdriver_mt_sleep() {}
    pub unsafe fn blockdriver_mt_get_tid() -> c_int { 0 }
    pub unsafe fn blockdriver_mt_set_workers(_device_id: c_int, _nr: c_int) {}
    pub unsafe fn blockdriver_mt_terminate() {}

    pub unsafe fn sys_hz() -> u64 { 100 }
    pub unsafe fn micro_delay(_us: c_uint) {}

    // Printf stub — no-op on host (tests don't need console output).
    // Avoids unstable c_variadic feature.
    pub unsafe fn printf(_fmt: *const c_char, _arg: *const c_char) -> c_int { 0 }
}

// ============================================================================
// Public API wrappers — delegate to platform module
// ============================================================================

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

pub fn vm_map_phys_ffi(phys_base: *mut c_void, size: usize) -> *mut c_void {
    unsafe { platform::vm_map_phys(platform::SELF, phys_base, size) }
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

pub fn irq_reenable(hook_id: &c_int) -> c_int {
    unsafe { platform::sys_irqenable(hook_id as *const c_int as *mut c_int) }
}

pub fn irq_remove(hook_id: &mut c_int) -> c_int {
    unsafe { platform::sys_irqrmpolicy(hook_id) }
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

pub use platform::{Blockdriver, DevMinor};

pub fn blockdriver_task(bdp: &Blockdriver) {
    unsafe { platform::blockdriver_mt_task(bdp as *const Blockdriver as *const c_void) }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read32_raw_works() {
        let val: u32 = 0xDEAD_BEEF;
        let r = unsafe { read32_raw(&val as *const u32 as usize) };
        assert_eq!(r, 0xDEAD_BEEF);
    }

    #[test]
    fn millis_to_ticks_works() {
        let ticks = millis_to_ticks(1000);
        assert!(ticks > 0);
    }

    #[test]
    fn pci_stubs_work() {
        assert_eq!(pci_init_ffi(), 0);
        assert!(pci_first_dev_ffi().is_none());
        assert!(pci_next_dev_ffi().is_none());
        pci_reserve_ffi(0);
    }

    #[test]
    fn irq_stubs_work() {
        assert!(irq_setup(0).is_none());
    }

    #[test]
    fn env_parse_stub_works() {
        let val = env_parse_long(b"test_param\0", 42, 0, 255);
        assert_eq!(val, 42);
    }
}
