//! # FFI — MINIX C system call bindings for virtio-net driver
//!
//! Dual-platform: real MINIX extern blocks + host stubs for cargo test.

#![allow(dead_code, unused_imports)]

use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};

// ============================================================================
// Platform selection
// ============================================================================

#[cfg(target_os = "minix")]
pub(crate) mod platform {
    use super::*;

    unsafe extern "C" {
        // Netdriver
        pub fn netdriver_task(ndp: *const c_void);
        pub fn netdriver_copyin(data: *mut NetdriverData, offset: usize, ptr: *const c_void, size: usize);
        pub fn netdriver_copyout(data: *mut NetdriverData, offset: usize, ptr: *const c_void, size: usize);
        pub fn netdriver_recv();
        pub fn netdriver_send();
        pub fn netdriver_link();

        // SEF
        pub fn sef_setcb_init_fresh(cb: Option<SefInitFreshFn>);
        pub fn sef_setcb_signal_handler(cb: Option<SefSignalHandlerFn>);
        pub fn sef_startup();
        pub fn env_setargs(argc: c_int, argv: *mut *mut c_char);
        pub fn env_parse(name: *const c_char, fmt: *const c_char, field: c_int,
            val: *mut c_long, min: c_long, max: c_long) -> c_int;

        // System
        pub fn sys_hz() -> u64;
        pub fn micro_delay(us: c_uint);
        pub fn printf(fmt: *const c_char) -> c_int;
    }

    pub type NetdriverData = c_void;
    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

    pub const SELF: c_int = -0x100;
    pub const OK: c_int = 0;
    pub const ENXIO: c_int = -6;
    pub const EIO: c_int = -5;
    pub const ENOMEM: c_int = -12;
    pub const EINVAL: c_int = -22;
    pub const SUSPEND: c_int = -998;

    pub const NDEV_CAP_MCAST: u32 = 0x0001;
    pub const NDEV_CAP_BCAST: u32 = 0x0002;
    pub const NDEV_CAP_HWADDR: u32 = 0x0004;

    pub type NetdriverAddr = [u8; 6];

    #[repr(C)]
    pub struct Netdriver {
        pub ndr_name: *const c_char,
        pub ndr_init: Option<unsafe extern "C" fn(c_uint, *mut NetdriverAddr, *mut u32, *mut c_uint) -> c_int>,
        pub ndr_stop: Option<unsafe extern "C" fn()>,
        pub ndr_set_mode: Option<unsafe extern "C" fn(u32, *const NetdriverAddr, c_uint)>,
        pub ndr_set_hwaddr: Option<unsafe extern "C" fn(*const NetdriverAddr)>,
        pub ndr_recv: Option<unsafe extern "C" fn(*mut NetdriverData, usize) -> isize>,
        pub ndr_send: Option<unsafe extern "C" fn(*mut NetdriverData, usize) -> c_int>,
        pub ndr_get_link: Option<unsafe extern "C" fn(*mut u32) -> c_uint>,
        pub ndr_intr: Option<unsafe extern "C" fn(c_uint)>,
        pub ndr_tick: Option<unsafe extern "C" fn()>,
    }
}

#[cfg(not(target_os = "minix"))]
pub(crate) mod platform {
    use super::*;

    pub type NetdriverData = c_void;
    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

    pub const SELF: c_int = -0x100;
    pub const OK: c_int = 0;
    pub const ENXIO: c_int = -6;
    pub const EIO: c_int = -5;
    pub const ENOMEM: c_int = -12;
    pub const EINVAL: c_int = -22;
    pub const SUSPEND: c_int = -998;

    pub const NDEV_CAP_MCAST: u32 = 0x0001;
    pub const NDEV_CAP_BCAST: u32 = 0x0002;
    pub const NDEV_CAP_HWADDR: u32 = 0x0004;

    pub type NetdriverAddr = [u8; 6];

    #[repr(C)]
    pub struct Netdriver {
        pub ndr_name: *const c_char,
        pub ndr_init: Option<unsafe extern "C" fn(c_uint, *mut NetdriverAddr, *mut u32, *mut c_uint) -> c_int>,
        pub ndr_stop: Option<unsafe extern "C" fn()>,
        pub ndr_set_mode: Option<unsafe extern "C" fn(u32, *const NetdriverAddr, c_uint)>,
        pub ndr_set_hwaddr: Option<unsafe extern "C" fn(*const NetdriverAddr)>,
        pub ndr_recv: Option<unsafe extern "C" fn(*mut NetdriverData, usize) -> isize>,
        pub ndr_send: Option<unsafe extern "C" fn(*mut NetdriverData, usize) -> c_int>,
        pub ndr_get_link: Option<unsafe extern "C" fn(*mut u32) -> c_uint>,
        pub ndr_intr: Option<unsafe extern "C" fn(c_uint)>,
        pub ndr_tick: Option<unsafe extern "C" fn()>,
    }

    // Stubs for host-side testing
    pub unsafe fn netdriver_task(_: *const c_void) {}
    pub unsafe fn netdriver_copyin(_: *mut NetdriverData, _: usize, _: *const c_void, _: usize) {}
    pub unsafe fn netdriver_copyout(_: *mut NetdriverData, _: usize, _: *const c_void, _: usize) {}
    pub unsafe fn netdriver_recv() {}
    pub unsafe fn netdriver_send() {}
    pub unsafe fn netdriver_link() {}
    pub unsafe fn sef_setcb_init_fresh(_: Option<SefInitFreshFn>) {}
    pub unsafe fn sef_setcb_signal_handler(_: Option<SefSignalHandlerFn>) {}
    pub unsafe fn sef_startup() {}
    pub unsafe fn env_setargs(_: c_int, _: *mut *mut c_char) {}
    pub unsafe fn env_parse(_: *const c_char, _: *const c_char, _: c_int,
        _: *mut c_long, _: c_long, _: c_long) -> c_int { 0 }
    pub unsafe fn sys_hz() -> u64 { 100 }
    pub unsafe fn micro_delay(_: c_uint) {}
    pub unsafe fn printf(_: *const c_char, _: *const c_char) -> c_int { 0 }
}

// ============================================================================
// Public API wrappers
// ============================================================================

pub use platform::{
    Netdriver, NetdriverData, NetdriverAddr,
    NDEV_CAP_MCAST, NDEV_CAP_BCAST, NDEV_CAP_HWADDR,
    SUSPEND, OK, ENXIO, ENOMEM, EINVAL,
};

pub fn netdriver_task(ndp: &platform::Netdriver) {
    unsafe { platform::netdriver_task(ndp as *const platform::Netdriver as *const c_void) }
}

pub fn netdriver_copyin_ffi(data: *mut NetdriverData, offset: usize, ptr: *const c_void, size: usize) {
    unsafe { platform::netdriver_copyin(data, offset, ptr, size) }
}

pub fn netdriver_copyout_ffi(data: *mut NetdriverData, offset: usize, ptr: *const c_void, size: usize) {
    unsafe { platform::netdriver_copyout(data, offset, ptr, size) }
}

pub fn netdriver_recv_ffi() { unsafe { platform::netdriver_recv() } }
pub fn netdriver_send_ffi() { unsafe { platform::netdriver_send() } }
pub fn netdriver_link_ffi() { unsafe { platform::netdriver_link() } }

// Environment / SEF
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

pub fn get_sys_hz() -> u64 { unsafe { platform::sys_hz() } }
pub fn udelay(us: u32) { unsafe { platform::micro_delay(us) } }

pub type c_long = isize;

pub fn print(msg: &[u8]) {
    unsafe {
        let fmt = b"%s\n\0".as_ptr() as *const c_char;
        platform::printf(fmt, msg.as_ptr() as *const c_char);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errno_constants() {
        assert_eq!(platform::ENXIO, -6);
        assert_eq!(platform::ENOMEM, -12);
        assert_eq!(platform::EINVAL, -22);
    }

    #[test]
    fn netdriver_size() {
        // 1 name ptr + 9 callbacks = 10 pointer-sized fields
        let expected = 10 * core::mem::size_of::<usize>();
        assert_eq!(core::mem::size_of::<platform::Netdriver>(), expected);
    }
}
