//! # minix-ahci — Rust AHCI SATA Driver for MINIX (Phase 5 Pilot)
//!
//! FFI bridge: provides `ahci_rust_main()` C entry point that replaces the
//! original C main() in the AHCI driver. Calls into libblockdriver via FFI
//! and handles AHCI hardware operations in pure Rust.

#![no_std]

pub mod registers;
pub mod ffi;
pub mod port;
pub mod hba;
pub mod ata;

use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};
use core::ptr;

use registers::{port_flags, PortState};
use hba::HbaController;
use port::Prd;

/// Helper: return a negative errno value (MINIX convention).
#[inline]
fn libc_errno(e: c_int) -> c_int { e }

// ============================================================================
// Global HBA instance
// ============================================================================

static mut GLOBAL_HBA: Option<HbaController> = None;

/// SAFETY: only called from single-threaded interrupt/blockdriver context
unsafe fn global_hba() -> &'static mut HbaController {
    unsafe { &mut *core::ptr::addr_of_mut!(GLOBAL_HBA) }
        .as_mut()
        .expect("AHCI: HBA not initialized")
}

// ============================================================================
// Blockdriver table constants
// ============================================================================

const BDR_TYPE_DISK: c_int = 0;
const BDEV_FORCEWRITE: c_int = 0x0002;
const BDEV_W_BIT: c_int = 0x0002;

// ============================================================================
// C-callable blockdriver operations
// ============================================================================

unsafe extern "C" fn ahci_c_open(minor: ffi::DevMinor, access: c_int) -> c_int {
    let hba = global_hba();
    let port_idx = map_minor_to_port(minor);
    if port_idx >= hba.nr_ports { return libc_errno(-6); /* ENXIO */ }

    let port = &mut hba.ports[port_idx];
    if port.state != PortState::GoodDev { return libc_errno(-6); }
    if (port.flags & port_flags::READONLY) != 0 && (access & BDEV_W_BIT) != 0 {
        return libc_errno(-13); /* EACCES */
    }

    if port.open_count == 0 {
        port.flags &= !port_flags::BARRIER;
        port.open_count = 1;
        ffi::blockdriver_set_workers(port.device_id, port.queue_depth as c_int);
    } else {
        if (port.flags & port_flags::BARRIER) != 0 {
            return libc_errno(-6); /* ENXIO */
        }
        port.open_count += 1;
    }
    0 // OK
}

unsafe extern "C" fn ahci_c_close(minor: ffi::DevMinor) -> c_int {
    let hba = global_hba();
    let port_idx = map_minor_to_port(minor);
    if port_idx >= hba.nr_ports { return 0; }

    let port = &mut hba.ports[port_idx];
    if port.open_count == 0 { return 0; }
    port.open_count -= 1;

    if port.open_count == 0 {
        ffi::blockdriver_set_workers(port.device_id, 1);
        let _ = ata::ata_flush(&hba.mmio, port_idx, port);
    }
    0
}

unsafe extern "C" fn ahci_c_transfer(
    minor: ffi::DevMinor,
    do_write: c_int,
    position: u64,
    _endpt: c_int,
    _iovec: *mut c_void,
    _count: c_uint,
    _flags: c_int,
) -> isize {
    let hba = global_hba();
    let port_idx = map_minor_to_port(minor);
    if port_idx >= hba.nr_ports { return libc_errno(-6) as isize; }

    let port = &mut hba.ports[port_idx];
    if port.state != PortState::GoodDev || (port.flags & port_flags::BARRIER) != 0 {
        return libc_errno(-5) as isize; /* EIO */
    }

    let sector = position / port.sector_size as u64;
    let prd = Prd {
        dba: port.bufs.tmp_phys as u32,
        resv: 0,
        resv2: 0,
        size: port.sector_size - 1,
    };

    let result = ata::ata_transfer(
        &hba.mmio, port_idx, port, sector, 1, do_write != 0, false, &[prd],
    );
    match result {
        ata::CmdResult::Success => port.sector_size as isize,
        ata::CmdResult::Failure => libc_errno(-5) as isize,
    }
}

unsafe extern "C" fn ahci_c_ioctl(
    minor: ffi::DevMinor,
    request: c_ulong,
    _endpt: c_int,
    _grant: c_int,
    _user_endpt: c_int,
) -> c_int {
    let hba = global_hba();
    let port_idx = map_minor_to_port(minor);
    if port_idx >= hba.nr_ports { return libc_errno(-6); }

    let port = &mut hba.ports[port_idx];
    const DIOCFLUSH: c_ulong = 0x6403;
    if request == DIOCFLUSH {
        if port.state != PortState::GoodDev || (port.flags & port_flags::BARRIER) != 0 {
            return libc_errno(-5);
        }
        match ata::ata_flush(&hba.mmio, port_idx, port) {
            ata::CmdResult::Success => 0,
            ata::CmdResult::Failure => libc_errno(-5),
        }
    } else {
        libc_errno(-25) /* ENOTTY */
    }
}

unsafe extern "C" fn ahci_c_part(_minor: ffi::DevMinor) -> *mut c_void {
    ptr::null_mut()
}

unsafe extern "C" fn ahci_c_intr(_mask: c_uint) {
    let hba = global_hba();
    let pi = hba.mmio.hba_read32(registers::hba::IS);

    for port_idx in 0..hba.nr_ports {
        if (pi & (1 << port_idx as u32)) == 0 { continue; }
        let port = &mut hba.ports[port_idx];
        if port.state == PortState::NoPort { continue; }

        let smask = hba.mmio.port_read32(port_idx, registers::port::IS);
        hba.mmio.port_write32(port_idx, registers::port::IS, smask);

        if smask & registers::port::IS_PCS != 0 {
            hba.mmio.port_write32(port_idx, registers::port::SERR, registers::port::SERR_DIAG_X);
            handle_port_connect(hba, port_idx);
        } else if smask & registers::port::IS_PRCS != 0 {
            hba.mmio.port_write32(port_idx, registers::port::SERR, registers::port::SERR_DIAG_N);
            handle_port_disconnect(hba, port_idx);
        }
    }
    hba.mmio.hba_write32(registers::hba::IS, pi);

    // Re-enable IRQ using the stored hook_id (not irq_setup which would re-register)
    ffi::irq_reenable(&hba.hook_id);
}

unsafe extern "C" fn ahci_c_alarm(_stamp: u64) {}

unsafe extern "C" fn ahci_c_device(minor: ffi::DevMinor, id: *mut c_int) -> c_int {
    let hba = global_hba();
    let port_idx = map_minor_to_port(minor);
    if port_idx >= hba.nr_ports { return libc_errno(-6); }
    *id = hba.ports[port_idx].device_id as c_int;
    0
}

// ============================================================================
// Blockdriver table (mutable statics for function pointer init)
// ============================================================================

static mut BDR_TABLE: ffi::Blockdriver = ffi::Blockdriver {
    bdr_type: BDR_TYPE_DISK,
    bdr_open: Some(ahci_c_open),
    bdr_close: Some(ahci_c_close),
    bdr_transfer: Some(ahci_c_transfer),
    bdr_ioctl: Some(ahci_c_ioctl),
    bdr_part: Some(ahci_c_part),
    bdr_intr: Some(ahci_c_intr),
    bdr_alarm: Some(ahci_c_alarm),
    bdr_device: Some(ahci_c_device),
};

// ============================================================================
// Port event handlers
// ============================================================================

fn handle_port_connect(hba: &mut HbaController, port_idx: usize) {
    let port = &mut hba.ports[port_idx];
    if !matches!(port.state, PortState::SpinUp | PortState::NoDev) { return; }
    if port.state == PortState::SpinUp {
        // Cancel spin-up timer (future: use alarm framework)
    }
    port.state = PortState::WaitDev;
}

fn handle_port_disconnect(hba: &mut HbaController, port_idx: usize) {
    let port = &mut hba.ports[port_idx];
    if !matches!(port.state, PortState::WaitId | PortState::GoodDev | PortState::BadDev) {
        return;
    }
    hba.mmio.port_stop(port_idx);
    port.state = PortState::NoDev;
    port.flags &= !port_flags::BUSY;
    port.flags |= port_flags::BARRIER;
}

// ============================================================================
// Minor device → port mapping
// ============================================================================

const DEV_PER_DRIVE: usize = 4;
const MAX_DRIVES: usize = 8;
const NR_MINORS: usize = MAX_DRIVES * DEV_PER_DRIVE;

fn map_minor_to_port(minor: ffi::DevMinor) -> usize {
    if minor >= 0 && (minor as usize) < NR_MINORS {
        (minor as usize) / DEV_PER_DRIVE
    } else {
        usize::MAX
    }
}

// ============================================================================
// SEF callbacks
// ============================================================================

unsafe extern "C" fn sef_init_fresh(_type: c_int, _info: *const c_void) -> c_int {
    let instance = ffi::env_parse_long(b"instance\0", 0, 0, 255);
    let verbose = ffi::env_parse_long(b"ahci_verbose\0", 1, 0, 4) as u8;

    let devind = match HbaController::probe(instance as i32) {
        Some(d) => d,
        None => {
            ffi::print(b"AHCI: no matching device found\n\0");
            return libc_errno(-5);
        }
    };

    let hba = HbaController::init(devind, instance as i32, verbose);
    // SAFETY: SEF init runs before any IRQ or worker threads
    unsafe { GLOBAL_HBA = Some(hba); }
    ffi::blockdriver_announce_ffi(_type);
    0
}

unsafe extern "C" fn sef_signal_handler(signo: c_int) {
    if signo != 15 { return; } // SIGTERM
    // SAFETY: SEF guarantees single-threaded context during signal handling
    unsafe {
        if let Some(ref mut hba) = GLOBAL_HBA {
            hba.stop();
        }
    }
}

/// C-compatible main entry — called from the C shim.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ahci_rust_main(argc: c_int, argv: *mut *mut c_char) -> c_int {
    ffi::env_setargs_ffi(argc, argv);
    ffi::sef_set_init_fresh(sef_init_fresh);
    ffi::sef_set_signal_handler(sef_signal_handler);
    ffi::sef_startup_ffi();
    // SAFETY: single-threaded blockdriver context, no concurrent access
    let bdp = unsafe { &*core::ptr::addr_of_mut!(BDR_TABLE) };
    ffi::blockdriver_task(bdp);
    0
}

// ============================================================================
// Panic handler (required for no_std; ignored by test builds where std provides one)
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
    use crate::registers::*;

    #[test]
    fn register_constants() {
        assert_eq!(hba::CAP, 0);
        assert_eq!(hba::GHC, 1);
        assert_eq!(hba::PI, 3);
        assert_eq!(port::IS_TFES, 1 << 30);
        assert_eq!(port::IS_PCS, 1 << 6);
        assert_eq!(port::CMD_ST, 1 << 0);
        assert_eq!(port::CMD_FRE, 1 << 4);
    }

    #[test]
    fn fis_constants() {
        assert_eq!(fis::TYPE_H2D, 0x27);
        assert_eq!(fis::H2D_FLAGS_C, 0x80);
        assert_eq!(fis::DEV_LBA, 0x40);
    }

    #[test]
    fn port_state_transitions() {
        assert!(!PortState::NoPort.is_device_present());
        assert!(!PortState::NoPort.is_operational());
        assert!(PortState::GoodDev.is_device_present());
        assert!(PortState::GoodDev.is_operational());
    }

    #[test]
    fn port_flags_constants() {
        assert_eq!(port_flags::ATAPI, 0x01);
        assert_eq!(port_flags::BARRIER, 0x40);
        assert_eq!(port_flags::HAS_NCQ, 0x0800);
    }
}
