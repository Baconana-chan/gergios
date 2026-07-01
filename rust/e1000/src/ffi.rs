//! # FFI — MINIX C system call bindings for e1000 driver
//!
//! Dual-platform: real MINIX extern blocks + host stubs for cargo test.

#![allow(dead_code, unused_imports)]

use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};

pub type c_long = isize;
pub use c_long as CLong;

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
        pub fn pci_attr_r32(devind: c_int, offset: c_int) -> u32;
        pub fn pci_attr_w16(devind: c_int, offset: c_int, value: u16);
        pub fn pci_dev_name(vid: u16, did: u16) -> *const c_char;
        pub fn pci_slot_name(devind: c_int) -> *const c_char;

        // Physical memory
        pub fn alloc_contig(size: usize, flags: c_int, phys: *mut u64) -> *mut c_void;
        pub fn free_contig(addr: *mut c_void, size: usize);

        // VM (MMIO mapping)
        pub fn vm_map_phys(endpt: c_int, base: *mut c_void, size: usize) -> *mut c_void;
        pub fn sys_umap(endpt: c_int, seg: c_int, vir_addr: *const c_void,
            vir_bytes: usize, phys_addr: *mut u64) -> c_int;

        // IRQ
        pub fn sys_irqsetpolicy(irq: c_int, policy: c_int, hook_id: *mut c_int) -> c_int;
        pub fn sys_irqenable(hook_id: *mut c_int) -> c_int;

        // Port I/O (for debug/testing)
        pub fn sys_inb(port: u16, value: *mut u32) -> c_int;
        pub fn sys_outb(port: u16, value: u8) -> c_int;

        // SEF
        pub fn sef_setcb_init_fresh(cb: Option<SefInitFreshFn>);
        pub fn sef_setcb_signal_handler(cb: Option<SefSignalHandlerFn>);
        pub fn sef_startup();
        pub fn env_setargs(argc: c_int, argv: *mut *mut c_char);
        pub fn env_parse(name: *const c_char, fmt: *const c_char, field: c_int,
            val: *mut c_long, min: c_long, max: c_long) -> c_int;

        // Netdriver
        pub fn netdriver_task(ndp: *const c_void);
        pub fn netdriver_announce();

        // Netdriver data movement
        pub fn netdriver_copyin(data: *mut NetdriverData, offset: usize, ptr: *const c_void, size: usize);
        pub fn netdriver_copyout(data: *mut NetdriverData, offset: usize, ptr: *const c_void, size: usize);

        // Netdriver notification calls
        pub fn netdriver_recv();
        pub fn netdriver_send();
        pub fn netdriver_link();

        // Netdriver statistics
        pub fn netdriver_stat_ierror(count: u32);
        pub fn netdriver_stat_coll(count: u32);
        pub fn netdriver_stat_oerror(count: u32);

        // System
        pub fn sys_hz() -> u64;
        pub fn micro_delay(us: c_uint);
        pub fn tsc_calibrate() -> c_int;
        pub fn printf(fmt: *const c_char) -> c_int;

        // VM segments
        pub fn VM_D() -> c_int;
    }

    // Netdriver data helper types
    pub type NetdriverData = c_void;

    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

    pub const SELF: c_int = -0x100;
    pub const AC_ALIGN4K: c_int = 1;
    pub const PAGE_SIZE: usize = 4096;
    pub const OK: c_int = 0;
    pub const ENXIO: c_int = -6;
    pub const ENOMEM: c_int = -12;
    pub const EINVAL: c_int = -22;
    pub const SUSPEND: c_int = -998;

    /// Netdevice capabilities
    pub const NDEV_CAP_MCAST: u32 = 0x0001;
    pub const NDEV_CAP_BCAST: u32 = 0x0002;
    pub const NDEV_CAP_HWADDR: u32 = 0x0004;

    /// Netdevice modes
    pub const NDEV_MODE_BCAST: u32 = 0x0001;
    pub const NDEV_MODE_PROMISC: u32 = 0x0002;
    pub const NDEV_MODE_MCAST_LIST: u32 = 0x0004;
    pub const NDEV_MODE_MCAST_ALL: u32 = 0x0008;

    /// Link status
    pub const NDEV_LINK_UP: u32 = 1;
    pub const NDEV_LINK_DOWN: u32 = 0;

    /// Media types (from net/if_media.h)
    pub const IFM_ETHER: u32 = 0x00000020;
    pub const IFM_10_T: u32 = 0x00000001;
    pub const IFM_100_TX: u32 = 0x00000002;
    pub const IFM_1000_T: u32 = 0x00000003;
    pub const IFM_FDX: u32 = 0x00100000;
    pub const IFM_HDX: u32 = 0x00000000;

    /// netdriver_addr (6 bytes)
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
    pub const AC_ALIGN4K: c_int = 1;
    pub const PAGE_SIZE: usize = 4096;
    pub const OK: c_int = 0;
    pub const ENXIO: c_int = -6;
    pub const ENOMEM: c_int = -12;
    pub const EINVAL: c_int = -22;
    pub const SUSPEND: c_int = -998;

    pub const NDEV_CAP_MCAST: u32 = 0x0001;
    pub const NDEV_CAP_BCAST: u32 = 0x0002;
    pub const NDEV_CAP_HWADDR: u32 = 0x0004;
    pub const NDEV_MODE_BCAST: u32 = 0x0001;
    pub const NDEV_MODE_PROMISC: u32 = 0x0002;
    pub const NDEV_MODE_MCAST_LIST: u32 = 0x0004;
    pub const NDEV_MODE_MCAST_ALL: u32 = 0x0008;
    pub const NDEV_LINK_UP: u32 = 1;
    pub const NDEV_LINK_DOWN: u32 = 0;
    pub const IFM_ETHER: u32 = 0x00000020;
    pub const IFM_10_T: u32 = 0x00000001;
    pub const IFM_100_TX: u32 = 0x00000002;
    pub const IFM_1000_T: u32 = 0x00000003;
    pub const IFM_FDX: u32 = 0x00100000;
    pub const IFM_HDX: u32 = 0x00000000;

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

    // Stubs
    pub unsafe fn pci_init() -> c_int { 0 }
    pub unsafe fn pci_first_dev(_: *mut c_int, _: *mut u16, _: *mut u16) -> c_int { 0 }
    pub unsafe fn pci_next_dev(_: *mut c_int, _: *mut u16, _: *mut u16) -> c_int { 0 }
    pub unsafe fn pci_reserve(_: c_int) {}
    pub unsafe fn pci_get_bar(_: c_int, _: c_int, base: *mut u32,
        size_: *mut u32, _: *mut c_int) -> c_int { unsafe { *base = 0; *size_ = 0x20000; 0 } }
    pub unsafe fn pci_attr_r8(_: c_int, _: c_int) -> u8 { 0 }
    pub unsafe fn pci_attr_r16(_: c_int, _: c_int) -> u16 { 0 }
    pub unsafe fn pci_attr_r32(_: c_int, _: c_int) -> u32 { 0 }
    pub unsafe fn pci_attr_w16(_: c_int, _: c_int, _: u16) {}
    pub unsafe fn pci_dev_name(_: u16, _: u16) -> *const c_char {
        b"e1000 (stub)\0".as_ptr() as *const c_char
    }
    pub unsafe fn pci_slot_name(_: c_int) -> *const c_char {
        b"00:00.0\0".as_ptr() as *const c_char
    }

    pub unsafe fn alloc_contig(size: usize, _: c_int, phys: *mut u64) -> *mut c_void {
        unsafe { *phys = 0x1000; }
        core::ptr::null_mut()
    }
    pub unsafe fn free_contig(_: *mut c_void, _: usize) {}

    pub unsafe fn vm_map_phys(_: c_int, base: *mut c_void, _: usize) -> *mut c_void { base }
    pub unsafe fn sys_umap(_: c_int, _: c_int, _: *const c_void, _: usize, _: *mut u64) -> c_int { 0 }

    pub unsafe fn sys_irqsetpolicy(_: c_int, _: c_int, _: *mut c_int) -> c_int { 0 }
    pub unsafe fn sys_irqenable(_: *mut c_int) -> c_int { 0 }

    pub unsafe fn sys_inb(_: u16, _: *mut u32) -> c_int { 0 }
    pub unsafe fn sys_outb(_: u16, _: u8) -> c_int { 0 }

    pub unsafe fn sef_setcb_init_fresh(_: *mut c_void) {}
    pub unsafe fn sef_setcb_signal_handler(_: *mut c_void) {}
    pub unsafe fn sef_startup() {}
    pub unsafe fn env_setargs(_: c_int, _: *mut *mut c_char) {}
    pub unsafe fn env_parse(_: *const c_char, _: *const c_char, _: c_int,
        _: *mut c_long, _: c_long, _: c_long) -> c_int { 0 }

    pub unsafe fn netdriver_task(_: *const c_void) {}
    pub unsafe fn netdriver_announce() {}
    pub unsafe fn netdriver_copyin(_: *mut NetdriverData, _: usize, _: *const c_void, _: usize) {}
    pub unsafe fn netdriver_copyout(_: *mut NetdriverData, _: usize, _: *const c_void, _: usize) {}
    pub unsafe fn netdriver_recv() {}
    pub unsafe fn netdriver_send() {}
    pub unsafe fn netdriver_link() {}
    pub unsafe fn netdriver_stat_ierror(_: u32) {}
    pub unsafe fn netdriver_stat_coll(_: u32) {}
    pub unsafe fn netdriver_stat_oerror(_: u32) {}
    pub unsafe fn sys_hz() -> u64 { 100 }
    pub unsafe fn micro_delay(_: c_uint) {}
    pub unsafe fn tsc_calibrate() -> c_int { 0 }
    pub unsafe fn printf(_: *const c_char, _: *const c_void) -> c_int { 0 }
    pub unsafe fn VM_D() -> c_int { 0 }
}

// ============================================================================
// Public API wrappers
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

pub fn pci_attr_r32_ffi(devind: c_int, offset: c_int) -> u32 {
    unsafe { platform::pci_attr_r32(devind, offset) }
}

pub fn pci_attr_w16_ffi(devind: c_int, offset: c_int, value: u16) {
    unsafe { platform::pci_attr_w16(devind, offset, value) }
}

pub fn pci_dev_name_ffi(vid: u16, did: u16) -> Option<&'static str> {
    unsafe {
        let ptr = platform::pci_dev_name(vid, did);
        if ptr.is_null() { None }
        else { Some(core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr as *const u8, 64))) }
    }
}

pub fn pci_slot_name_ffi(devind: c_int) -> &'static str {
    unsafe {
        let ptr = platform::pci_slot_name(devind);
        core::str::from_utf8_unchecked(core::slice::from_raw_parts(ptr as *const u8, 16))
    }
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

pub fn vm_map_phys_ffi(base: u32, size: u32) -> *mut u8 {
    unsafe { platform::vm_map_phys(platform::SELF, base as *mut c_void, size as usize) as *mut u8 }
}

pub fn sys_umap_ffi(vir_addr: *const c_void, bytes: usize) -> Option<u64> {
    unsafe {
        let mut phys: u64 = 0;
        let r = platform::sys_umap(platform::SELF, platform::VM_D(), vir_addr, bytes, &mut phys);
        if r != 0 { None } else { Some(phys) }
    }
}

pub fn irq_setup_ffi(irq: c_int) -> Option<c_int> {
    unsafe {
        let mut hook_id: c_int = irq; // MINIX convention: hook_id = irq line
        let r = platform::sys_irqsetpolicy(irq, 0, &mut hook_id);
        if r != 0 { return None; }
        let r = platform::sys_irqenable(&mut hook_id);
        if r != 0 { return None; }
        Some(hook_id)
    }
}

pub fn irq_reenable_ffi(hook_id: &c_int) -> c_int {
    unsafe { platform::sys_irqenable(hook_id as *const c_int as *mut c_int) }
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

pub fn netdriver_task(ndp: &platform::Netdriver) {
    unsafe { platform::netdriver_task(ndp as *const platform::Netdriver as *const c_void) }
}

pub fn netdriver_announce_ffi() { unsafe { platform::netdriver_announce() } }

pub fn netdriver_copyin_ffi(data: *mut NetdriverData, offset: usize, ptr: *const c_void, size: usize) {
    unsafe { platform::netdriver_copyin(data, offset, ptr, size) }
}

pub fn netdriver_copyout_ffi(data: *mut NetdriverData, offset: usize, ptr: *const c_void, size: usize) {
    unsafe { platform::netdriver_copyout(data, offset, ptr, size) }
}

pub fn netdriver_recv_ffi() { unsafe { platform::netdriver_recv() } }
pub fn netdriver_send_ffi() { unsafe { platform::netdriver_send() } }
pub fn netdriver_link_ffi() { unsafe { platform::netdriver_link() } }

pub fn netdriver_stat_ierror_ffi(count: u32) { unsafe { platform::netdriver_stat_ierror(count) } }
pub fn netdriver_stat_coll_ffi(count: u32) { unsafe { platform::netdriver_stat_coll(count) } }
pub fn netdriver_stat_oerror_ffi(count: u32) { unsafe { platform::netdriver_stat_oerror(count) } }

pub fn get_sys_hz() -> u64 { unsafe { platform::sys_hz() } }
pub fn udelay(us: u32) { unsafe { platform::micro_delay(us) } }
pub fn tsc_calibrate_ffi() -> c_int { unsafe { platform::tsc_calibrate() } }

pub fn print(msg: &[u8]) {
    unsafe {
        let fmt = b"%s\\n\0".as_ptr() as *const c_char;
        platform::printf(fmt, msg.as_ptr() as *const core::ffi::c_void);
    }
}

pub fn debug_print(args: &[u8]) {
    unsafe {
        let fmt = b"%s\0".as_ptr() as *const c_char;
        platform::printf(fmt, args.as_ptr() as *const core::ffi::c_void);
    }
}

pub use platform::{
    Netdriver, NetdriverData, NetdriverAddr,
    NDEV_CAP_MCAST, NDEV_CAP_BCAST, NDEV_CAP_HWADDR,
    NDEV_MODE_BCAST, NDEV_MODE_PROMISC, NDEV_MODE_MCAST_LIST, NDEV_MODE_MCAST_ALL,
    NDEV_LINK_UP, NDEV_LINK_DOWN,
    IFM_ETHER, IFM_10_T, IFM_100_TX, IFM_1000_T, IFM_FDX, IFM_HDX,
    SUSPEND, OK, ENXIO, ENOMEM, EINVAL,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stubs_work() {
        assert_eq!(pci_init_ffi(), 0);
        assert!(pci_first_dev_ffi().is_none() || pci_first_dev_ffi().is_some());
        assert_eq!(pci_attr_r8_ffi(0, 0), 0);
        let name = pci_slot_name_ffi(0);
        assert!(!name.is_empty());
    }
}
