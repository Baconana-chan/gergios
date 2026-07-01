//! # FFI — MINIX C system call bindings for the PCI bus driver
//!
//! All extern blocks use `unsafe extern \"C\"` (Rust 2024 edition).
//! Non-MINIX targets get stub implementations for host-side testing.

#![allow(dead_code, unused_imports)]

use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};

pub type CLong = isize;
pub use CLong as c_long;

// ============================================================================
// Platform selection: MINIX (real FFI) vs host (linkable stubs)
// ============================================================================

#[cfg(target_os = "minix")]
pub(crate) mod platform {
    use super::*;

    unsafe extern "C" {
        // Chardriver
        pub fn chardriver_task(cdp: *const c_void);
        pub fn chardriver_announce();

        // SEF
        pub fn sef_setcb_init_fresh(cb: Option<SefInitFreshFn>);
        pub fn sef_setcb_signal_handler(cb: Option<SefSignalHandlerFn>);
        pub fn sef_startup();

        // Environment
        pub fn env_setargs(argc: c_int, argv: *mut *mut c_char);
        pub fn env_parse(name: *const c_char, fmt: *const c_char, field: c_int,
            val: *mut c_long, min: c_long, max: c_long) -> c_int;

        // PCI I/O port access (legacy config mechanism #1: 0xCF8/0xCFC)
        pub fn sys_outl(port: u16, value: u32) -> c_int;
        pub fn sys_inb(port: u16, value: *mut u32) -> c_int;
        pub fn sys_inw(port: u16, value: *mut u32) -> c_int;
        pub fn sys_inl(port: u16, value: *mut u32) -> c_int;
        pub fn sys_outb(port: u16, value: u8) -> c_int;
        pub fn sys_outw(port: u16, value: u16) -> c_int;

        // IPC
        pub fn ipc_send(dest: c_int, m_ptr: *mut c_void) -> c_int;
        pub fn sef_receive_status(src: *mut c_int, m_ptr: *mut c_void,
            status: *mut c_uint) -> c_int;

        // System
        pub fn sys_hz() -> u64;
        pub fn micro_delay(us: c_uint);

        // ACPI
        pub fn acpi_init() -> c_int;
        pub fn acpi_get_irq(bus: c_int, dev: c_int, pin: c_int) -> c_int;
        pub fn acpi_map_bridge(bus: c_int, dev: c_int, sec_bus: c_int);

        // Interrupt
        pub fn sys_irqsetpolicy(irq: c_int, policy: c_int, hook_id: *mut c_int) -> c_int;
        pub fn sys_irqenable(hook_id: *mut c_int) -> c_int;

        // Privilege control
        pub fn sys_privctl(proc: c_int, req: c_int, p: *const c_void) -> c_int;

        // Safe copy (for IPC with large data)
        pub fn sys_safecopyfrom(proc: c_int, grant: c_int, offset: c_ulong,
            buf: *mut c_void, bytes: c_ulong) -> c_int;
        pub fn sys_safecopyto(proc: c_int, grant: c_int, offset: c_ulong,
            buf: *const c_void, bytes: c_ulong) -> c_int;

        // Misc
        pub fn printf(fmt: *const c_char, arg1: *const c_char) -> c_int;
    }

    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

    // Message types for BUSC_PCI protocol
    pub const BUSC_PCI_INIT: c_int = 0x1001;
    pub const BUSC_PCI_FIRST_DEV: c_int = 0x1002;
    pub const BUSC_PCI_NEXT_DEV: c_int = 0x1003;
    pub const BUSC_PCI_FIND_DEV: c_int = 0x1004;
    pub const BUSC_PCI_IDS: c_int = 0x1005;
    pub const BUSC_PCI_RESERVE: c_int = 0x1006;
    pub const BUSC_PCI_ATTR_R8: c_int = 0x1007;
    pub const BUSC_PCI_ATTR_R16: c_int = 0x1008;
    pub const BUSC_PCI_ATTR_R32: c_int = 0x1009;
    pub const BUSC_PCI_ATTR_W8: c_int = 0x100A;
    pub const BUSC_PCI_ATTR_W16: c_int = 0x100B;
    pub const BUSC_PCI_ATTR_W32: c_int = 0x100C;
    pub const BUSC_PCI_RESCAN: c_int = 0x100D;
    pub const BUSC_PCI_DEV_NAME_S: c_int = 0x100E;
    pub const BUSC_PCI_SLOT_NAME_S: c_int = 0x100F;
    pub const BUSC_PCI_SET_ACL: c_int = 0x1010;
    pub const BUSC_PCI_DEL_ACL: c_int = 0x1011;
    pub const BUSC_PCI_GET_BAR: c_int = 0x1012;

    // PCI I/O ports for config mechanism #1
    pub const PCII_CONFADD: u16 = 0xCF8;
    pub const PCII_CONFDATA: u16 = 0xCFC;
    pub const PCII_UNSEL: u32 = 0;

    pub const SELF: c_int = -0x100;
    pub const RS_PROC_NR: c_int = -5;

    #[repr(C)]
    pub struct Chardriver {
        pub cdr_open: Option<unsafe extern "C" fn(DevMinor, c_int, c_int) -> c_int>,
        pub cdr_close: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
        pub cdr_ioctl: Option<unsafe extern "C" fn(
            DevMinor, c_ulong, c_int, c_int, c_int, c_int, c_uint) -> c_int>,
        pub cdr_other: Option<unsafe extern "C" fn(
            m_ptr: *mut c_void, ipc_status: c_int)>,
    }

    pub type DevMinor = c_int;
}

#[cfg(not(target_os = "minix"))]
pub(crate) mod platform {
    use super::*;
    use core::ptr;

    pub type SefInitFreshFn = unsafe extern "C" fn(c_int, *const c_void) -> c_int;
    pub type SefSignalHandlerFn = unsafe extern "C" fn(c_int);

    pub const BUSC_PCI_INIT: c_int = 0x1001;
    pub const BUSC_PCI_FIRST_DEV: c_int = 0x1002;
    pub const BUSC_PCI_NEXT_DEV: c_int = 0x1003;
    pub const BUSC_PCI_FIND_DEV: c_int = 0x1004;
    pub const BUSC_PCI_IDS: c_int = 0x1005;
    pub const BUSC_PCI_RESERVE: c_int = 0x1006;
    pub const BUSC_PCI_ATTR_R8: c_int = 0x1007;
    pub const BUSC_PCI_ATTR_R16: c_int = 0x1008;
    pub const BUSC_PCI_ATTR_R32: c_int = 0x1009;
    pub const BUSC_PCI_ATTR_W8: c_int = 0x100A;
    pub const BUSC_PCI_ATTR_W16: c_int = 0x100B;
    pub const BUSC_PCI_ATTR_W32: c_int = 0x100C;
    pub const BUSC_PCI_RESCAN: c_int = 0x100D;
    pub const BUSC_PCI_DEV_NAME_S: c_int = 0x100E;
    pub const BUSC_PCI_SLOT_NAME_S: c_int = 0x100F;
    pub const BUSC_PCI_SET_ACL: c_int = 0x1010;
    pub const BUSC_PCI_DEL_ACL: c_int = 0x1011;
    pub const BUSC_PCI_GET_BAR: c_int = 0x1012;

    pub const PCII_CONFADD: u16 = 0xCF8;
    pub const PCII_CONFDATA: u16 = 0xCFC;
    pub const PCII_UNSEL: u32 = 0;

    pub const SELF: c_int = -0x100;
    pub const RS_PROC_NR: c_int = -5;

    #[repr(C)]
    pub struct Chardriver {
        pub cdr_open: Option<unsafe extern "C" fn(DevMinor, c_int, c_int) -> c_int>,
        pub cdr_close: Option<unsafe extern "C" fn(DevMinor) -> c_int>,
        pub cdr_ioctl: Option<unsafe extern "C" fn(
            DevMinor, c_ulong, c_int, c_int, c_int, c_int, c_uint) -> c_int>,
        pub cdr_other: Option<unsafe extern "C" fn(
            m_ptr: *mut c_void, ipc_status: c_int)>,
    }

    pub type DevMinor = c_int;

    // SAFETY: stubs only used on host for cargo test
    pub unsafe fn chardriver_task(_cdp: *const c_void) {}
    pub unsafe fn chardriver_announce() {}

    pub unsafe fn sef_setcb_init_fresh(_cb: *mut c_void) {}
    pub unsafe fn sef_setcb_signal_handler(_cb: *mut c_void) {}
    pub unsafe fn sef_startup() {}
    pub unsafe fn env_setargs(_argc: c_int, _argv: *mut *mut c_char) {}
    pub unsafe fn env_parse(_name: *const c_char, _fmt: *const c_char, _field: c_int,
        _val: *mut c_long, _min: c_long, _max: c_long) -> c_int { 0 }

    // PCI I/O port stubs
    pub unsafe fn sys_outl(_port: u16, _value: u32) -> c_int { 0 }
    pub unsafe fn sys_inb(_port: u16, _value: *mut u32) -> c_int { *(_value as *mut u32) = 0xFF; 0 }
    pub unsafe fn sys_inw(_port: u16, _value: *mut u32) -> c_int { *(_value as *mut u32) = 0xFFFF; 0 }
    pub unsafe fn sys_inl(_port: u16, _value: *mut u32) -> c_int { *(_value as *mut u32) = 0xFFFFFFFF; 0 }
    pub unsafe fn sys_outb(_port: u16, _value: u8) -> c_int { 0 }
    pub unsafe fn sys_outw(_port: u16, _value: u16) -> c_int { 0 }

    pub unsafe fn ipc_send(_dest: c_int, _m_ptr: *mut c_void) -> c_int { 0 }
    pub unsafe fn sef_receive_status(_src: *mut c_int, _m_ptr: *mut c_void,
        _status: *mut c_uint) -> c_int { 0 }

    pub unsafe fn sys_hz() -> u64 { 100 }
    pub unsafe fn micro_delay(_us: c_uint) {}

    pub unsafe fn acpi_init() -> c_int { -1 }
    pub unsafe fn acpi_get_irq(_bus: c_int, _dev: c_int, _pin: c_int) -> c_int { -1 }
    pub unsafe fn acpi_map_bridge(_bus: c_int, _dev: c_int, _sec_bus: c_int) {}

    pub unsafe fn sys_irqsetpolicy(_irq: c_int, _policy: c_int, _hook_id: *mut c_int) -> c_int { -1 }
    pub unsafe fn sys_irqenable(_hook_id: *mut c_int) -> c_int { -1 }

    pub unsafe fn sys_privctl(_proc: c_int, _req: c_int, _p: *const c_void) -> c_int { -1 }

    pub unsafe fn sys_safecopyfrom(_proc: c_int, _grant: c_int, _offset: c_ulong,
        _buf: *mut c_void, _bytes: c_ulong) -> c_int { -1 }
    pub unsafe fn sys_safecopyto(_proc: c_int, _grant: c_int, _offset: c_ulong,
        _buf: *const c_void, _bytes: c_ulong) -> c_int { -1 }

    pub unsafe fn printf(_fmt: *const c_char, _arg: *const c_char) -> c_int { 0 }
}

// ============================================================================
// Public API wrappers
// ============================================================================

pub type DevMinor = platform::DevMinor;

pub fn chardriver_task(cdp: &platform::Chardriver) {
    unsafe { platform::chardriver_task(cdp as *const platform::Chardriver as *const c_void) }
}

pub fn chardriver_announce_ffi() {
    unsafe { platform::chardriver_announce() }
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

/// Build a PCI configuration address for mechanism #1.
#[inline]
pub fn pci_make_addr(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
    0x8000_0000u32
        | ((bus as u32) << 16)
        | ((dev as u32) << 11)
        | ((func as u32) << 8)
        | ((offset as u32) & 0xFC)
}

/// Read a byte from PCI config space via I/O ports.
pub unsafe fn pci_read_config8(bus: u8, dev: u8, func: u8, offset: u8) -> u8 {
    let addr = pci_make_addr(bus, dev, func, offset);
    let _ = platform::sys_outl(platform::PCII_CONFADD, addr);
    let mut val: u32 = 0;
    let _ = platform::sys_inb(platform::PCII_CONFDATA, &mut val);
    let _ = platform::sys_outl(platform::PCII_CONFADD, platform::PCII_UNSEL);
    (val & 0xFF) as u8
}

/// Read a word from PCI config space via I/O ports.
pub unsafe fn pci_read_config16(bus: u8, dev: u8, func: u8, offset: u8) -> u16 {
    let addr = pci_make_addr(bus, dev, func, offset);
    let _ = platform::sys_outl(platform::PCII_CONFADD, addr);
    let mut val: u32 = 0;
    let _ = platform::sys_inw(platform::PCII_CONFDATA, &mut val);
    let _ = platform::sys_outl(platform::PCII_CONFADD, platform::PCII_UNSEL);
    (val & 0xFFFF) as u16
}

/// Read a dword from PCI config space via I/O ports.
pub unsafe fn pci_read_config32(bus: u8, dev: u8, func: u8, offset: u8) -> u32 {
    let addr = pci_make_addr(bus, dev, func, offset);
    let _ = platform::sys_outl(platform::PCII_CONFADD, addr);
    let mut val: u32 = 0;
    let _ = platform::sys_inl(platform::PCII_CONFDATA, &mut val);
    let _ = platform::sys_outl(platform::PCII_CONFADD, platform::PCII_UNSEL);
    val
}

/// Write a byte to PCI config space via I/O ports.
pub unsafe fn pci_write_config8(bus: u8, dev: u8, func: u8, offset: u8, value: u8) {
    // Read-modify-write for byte access
    let addr = pci_make_addr(bus, dev, func, offset);
    let _ = platform::sys_outl(platform::PCII_CONFADD, addr);
    let mut val: u32 = 0;
    let _ = platform::sys_inl(platform::PCII_CONFDATA, &mut val);
    val = (val & !0xFF) | (value as u32);
    let _ = platform::sys_outl(platform::PCII_CONFADD, addr);
    let _ = platform::sys_outl(platform::PCII_CONFDATA, val);
    let _ = platform::sys_outl(platform::PCII_CONFADD, platform::PCII_UNSEL);
}

/// Write a word to PCI config space via I/O ports.
pub unsafe fn pci_write_config16(bus: u8, dev: u8, func: u8, offset: u8, value: u16) {
    let addr = pci_make_addr(bus, dev, func, offset);
    let _ = platform::sys_outl(platform::PCII_CONFADD, addr);
    let mut val: u32 = 0;
    let _ = platform::sys_inl(platform::PCII_CONFDATA, &mut val);
    val = (val & !0xFFFF) | (value as u32);
    let _ = platform::sys_outl(platform::PCII_CONFADD, addr);
    let _ = platform::sys_outl(platform::PCII_CONFDATA, val);
    let _ = platform::sys_outl(platform::PCII_CONFADD, platform::PCII_UNSEL);
}

/// Write a dword to PCI config space via I/O ports.
pub unsafe fn pci_write_config32(bus: u8, dev: u8, func: u8, offset: u8, value: u32) {
    let addr = pci_make_addr(bus, dev, func, offset);
    let _ = platform::sys_outl(platform::PCII_CONFADD, addr);
    let _ = platform::sys_outl(platform::PCII_CONFDATA, value);
    let _ = platform::sys_outl(platform::PCII_CONFADD, platform::PCII_UNSEL);
}

pub fn get_sys_hz() -> u64 { unsafe { platform::sys_hz() } }
pub fn udelay(us: u32) { unsafe { platform::micro_delay(us) } }

pub fn millis_to_ticks(ms: u64) -> u64 {
    let hz = get_sys_hz();
    (ms * hz + 999) / 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pci_make_addr_works() {
        let addr = pci_make_addr(0, 0, 0, 0);
        assert_eq!(addr, 0x8000_0000);
        let addr = pci_make_addr(0, 0, 0, PCI_VENDOR_ID);
        assert_eq!(addr, 0x8000_0000);
        let addr = pci_make_addr(0, 2, 0, PCI_CLASS_CODE);
        assert_eq!(addr, 0x8000_1008); // bus=0, dev=2, func=0, offset=8
    }

    #[test]
    fn pci_config_offsets_known() {
        assert_eq!(pci_make_addr(0, 0, 0, 0x00), 0x8000_0000);
        assert_eq!(pci_make_addr(0, 0, 0, 0x3C), 0x8000_003C);
    }
}

// PCI config space register offsets (for tests)
#[cfg(test)]
const PCI_VENDOR_ID: u8 = 0x00;
#[cfg(test)]
const PCI_CLASS_CODE: u8 = 0x08;
