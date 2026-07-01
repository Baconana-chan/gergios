//! # minix-pci — Rust PCI Bus Driver for MINIX (Phase 5 Pilot)
//!
//! Provides `pci_rust_main()` C entry point that replaces the original C
//! main() in the PCI bus driver. Implements PCI bus scanning via legacy
//! I/O ports (PCI configuration mechanism #1) and provides IPC services
//! to other drivers: device enumeration, config space access, BAR info.
//!
//! Architecture overview:
//!   - Scans PCI bus(es) via I/O ports (0xCF8/0xCFC)
//!   - Maintains a device table with vendor/device IDs, BARs, IRQ
//!   - Handles BUSC_PCI IPC messages from other drivers
//!   - Supports ISA bridge detection (PIIX, VIA, AMD, SIS)
//!   - Integrates with ACPI for IRQ routing (when available)

#![no_std]

pub mod ffi;
pub mod devices;
pub mod server;

use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};
use core::ptr;

use devices::PciDeviceTable;

// ============================================================================
// Global PCI state
// ============================================================================

/// Global PCI device table — holds all discovered PCI devices.
pub(crate) static mut PCI_STATE: Option<PciDeviceTable> = None;

/// SAFETY: only called from single-threaded chardriver/SEF context.
pub(crate) unsafe fn pci_state() -> &'static mut PciDeviceTable {
    unsafe { &mut *core::ptr::addr_of_mut!(PCI_STATE) }
        .as_mut()
        .expect("PCI: not initialized")
}

// ============================================================================
// SEF callbacks
// ============================================================================

unsafe extern "C" fn sef_init_fresh(_type: c_int, _info: *const c_void) -> c_int {
    let mut table = PciDeviceTable::new();
    table.probe_all();
    // SAFETY: SEF init runs single-threaded
    unsafe { PCI_STATE = Some(table); }
    ffi::chardriver_announce_ffi();
    0
}

unsafe extern "C" fn sef_signal_handler(signo: c_int) {
    if signo != 15 { return; } // SIGTERM
    // PCI server doesn't need cleanup — RS handles restart
}

// ============================================================================
// C-compatible main entry
// ============================================================================

/// C-compatible main entry — called from the C shim.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pci_rust_main(argc: c_int, argv: *mut *mut c_char) -> c_int {
    ffi::env_setargs_ffi(argc, argv);
    ffi::sef_set_init_fresh(sef_init_fresh);
    ffi::sef_set_signal_handler(sef_signal_handler);
    ffi::sef_startup_ffi();

    // Register chardriver callbacks and enter the main loop
    let drv = server::PciServer::new();
    // SAFETY: chardriver_task runs the main IPC loop, single-threaded
    unsafe { ffi::chardriver_task(&drv.as_chardriver()); }
    0
}

// ============================================================================
// Panic handler
// ============================================================================

#[cfg(not(test))]
#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::*;

    #[test]
    fn pci_config_offsets() {
        assert_eq!(PCI_VENDOR_ID, 0x00);
        assert_eq!(PCI_DEVICE_ID, 0x02);
        assert_eq!(PCI_COMMAND, 0x04);
        assert_eq!(PCI_STATUS, 0x06);
        assert_eq!(PCI_CLASS_CODE, 0x08);
        assert_eq!(PCI_HEADER_TYPE, 0x0E);
        assert_eq!(PCI_BAR_0, 0x10);
        assert_eq!(PCI_INTERRUPT_LINE, 0x3C);
    }

    #[test]
    fn device_table_empty() {
        let table = PciDeviceTable::new();
        assert_eq!(table.count(), 0);
    }

    #[test]
    fn device_table_add_and_count() {
        let mut table = PciDeviceTable::new();
        table.add_for_test(0x8086, 0x100E, 0, 0, 0x020000, 0);
        assert_eq!(table.count(), 1);
        table.add_for_test(0x8086, 0x2922, 1, 0, 0x010601, 0);
        assert_eq!(table.count(), 2);
    }

    #[test]
    fn device_table_find() {
        let mut table = PciDeviceTable::new();
        table.add_for_test(0x8086, 0x100E, 0, 0, 0x020000, 0);
        table.add_for_test(0x8086, 0x2922, 1, 1, 0x010601, 0);

        let (idx, _) = table.find_first(0x8086, 0x100E).unwrap();
        assert_eq!(idx, 0);

        let (idx, _) = table.find_first(0x8086, 0x2922).unwrap();
        assert_eq!(idx, 1);

        assert!(table.find_first(0x10EC, 0x8139).is_none());
    }

    #[test]
    fn device_table_first_next() {
        let mut table = PciDeviceTable::new();
        table.add_for_test(0x8086, 0x100E, 0, 0, 0x020000, 0);
        table.add_for_test(0x8086, 0x2922, 1, 1, 0x010601, 0);
        table.add_for_test(0x10EC, 0x8139, 2, 2, 0x020000, 0);

        // First
        let (mut idx, vid, did) = table.first_dev().unwrap();
        assert_eq!(idx, 0);
        assert_eq!(vid, 0x8086);

        // Next
        (idx, _, _) = table.next_dev(idx).unwrap();
        assert_eq!(idx, 1);
        (idx, _, _) = table.next_dev(idx).unwrap();
        assert_eq!(idx, 2);
        assert!(table.next_dev(idx).is_none());
    }
}
