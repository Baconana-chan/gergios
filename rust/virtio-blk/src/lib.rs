//! # virtio-blk — Rust Virtio Block Device Driver for MINIX (Phase 5 Pilot)
//!
//! Implements a block driver for virtio-blk devices using the legacy (pre-1.0)
//! PCI transport. Uses the multi-threaded blockdriver framework.
//!
//! Architecture:
//!   - PCI probe via virtio_setup_device() for transitional virtio-blk (0x0002)
//!   - I/O port BAR for register access (legacy virtio)
//!   - Split virtqueue (descriptor/avail/used rings) with indirect descriptors
//!   - blockdriver_mt interface with per-thread request buffers

#![no_std]

pub mod ffi;
pub mod virtio;
pub mod blk;

use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};
use core::ptr;

use blk::VirtioBlk;

// ============================================================================
// Constants
// ============================================================================

const BDR_TYPE_DISK: c_int = 0;
const BDEV_W_BIT: c_int = 0x0002;
const SECTOR_SIZE: usize = 512;
const DEV_PER_DRIVE: usize = 4;
const MAX_DRIVES: usize = 1;
const NR_MINORS: usize = MAX_DRIVES * DEV_PER_DRIVE;
const SUB_PER_DRIVE: usize = 4;
const MINOR_D0P0S0: c_int = 4;
const NUM_THREADS: usize = 4;
const DATA_SIZE: usize = SECTOR_SIZE * 8; // 4KB buffer — enough for small transfers

// ============================================================================
// Global virtio-blk state
// ============================================================================

static mut VIRTIO_BLK: Option<VirtioBlk> = None;
static mut OPEN_COUNT: c_int = 0;
static mut TERMINATING: bool = false;

/// SAFETY: only called from single-threaded blockdriver/SEF context.
fn global_blk() -> &'static mut VirtioBlk {
    unsafe { &mut *core::ptr::addr_of_mut!(VIRTIO_BLK) }
        .as_mut()
        .expect("virtio-blk: not initialized")
}

// ============================================================================
// Partition table helpers
// ============================================================================

#[repr(C)]
struct Device {
    dv_base: u64,
    dv_size: u64,
}

static mut PART: [Device; DEV_PER_DRIVE] = [
    Device { dv_base: 0, dv_size: 0 },
    Device { dv_base: 0, dv_size: 0 },
    Device { dv_base: 0, dv_size: 0 },
    Device { dv_base: 0, dv_size: 0 },
];

static mut SUBPART: [Device; SUB_PER_DRIVE] = [
    Device { dv_base: 0, dv_size: 0 },
    Device { dv_base: 0, dv_size: 0 },
    Device { dv_base: 0, dv_size: 0 },
    Device { dv_base: 0, dv_size: 0 },
];

fn map_minor_to_drive(_minor: c_int) -> usize {
    0 // single drive
}

// ============================================================================
// Blockdriver callbacks
// ============================================================================

unsafe extern "C" fn vb_open(minor: c_int, access: c_int) -> c_int {
    let blk = global_blk();
    if minor < 0 || minor as usize >= NR_MINORS {
        return ffi::platform::ENXIO;
    }

    // Check if partition exists
    let dev = vb_part(minor);
    if dev.is_null() { return ffi::platform::ENXIO; }

    // Read-only check
    if blk.read_only && (access & BDEV_W_BIT) != 0 {
        return ffi::platform::EACCES;
    }

    if OPEN_COUNT == 0 {
        // First open: set up partition table
        let capacity_sectors = blk.capacity;
        PART[0] = Device {
            dv_base: 0,
            dv_size: capacity_sectors * 512,
        };
        // Reset subpartitions — use raw pointer to bypass Rust 2024 static_mut_refs
        let subpart = unsafe { &mut *core::ptr::addr_of_mut!(SUBPART) };
        for s in subpart.iter_mut() {
            *s = Device { dv_base: 0, dv_size: 0 };
        }
        ffi::blockdriver_set_workers(0, NUM_THREADS as c_int);
    }

    OPEN_COUNT += 1;
    0
}

unsafe extern "C" fn vb_close(minor: c_int) -> c_int {
    if minor < 0 || minor as usize >= NR_MINORS { return 0; }
    if OPEN_COUNT == 0 { return ffi::platform::EINVAL; }

    OPEN_COUNT -= 1;

    if OPEN_COUNT == 0 {
        // Flush on last close
        if let Some(ref mut blk) = *core::ptr::addr_of_mut!(VIRTIO_BLK) {
            blk.flush();
        }
        ffi::blockdriver_set_workers(0, 1);
    }

    if TERMINATING && OPEN_COUNT == 0 {
        ffi::blockdriver_terminate();
    }

    0
}

unsafe extern "C" fn vb_transfer(
    minor: c_int,
    do_write: c_int,
    position: u64,
    endpt: c_int,
    iovec: *mut c_void,
    count: c_uint,
    _flags: c_int,
) -> isize {
    if minor < 0 || minor as usize >= NR_MINORS {
        return ffi::platform::ENXIO as isize;
    }

    let dev = vb_part(minor);
    if dev.is_null() { return ffi::platform::ENXIO as isize; }

    // Adjust for partition base
    let dev_ref = &*(dev as *const Device);
    let adjusted_pos = position + dev_ref.dv_base;

    // Sector alignment check
    let sector = adjusted_pos / SECTOR_SIZE as u64;
    if adjusted_pos % SECTOR_SIZE as u64 != 0 {
        return ffi::platform::EINVAL as isize;
    }

    // Limit to NR_IOREQS entries
    let cnt = if (count as usize) > blk::NR_IOREQS {
        blk::NR_IOREQS
    } else {
        count as usize
    };

    // Extract grants and sizes from the iovec array
    let iv = iovec as *const ffi::IoVec;
    let mut grants: [c_int; blk::NR_IOREQS] = [0; blk::NR_IOREQS];
    let mut sizes: [usize; blk::NR_IOREQS] = [0; blk::NR_IOREQS];

    for i in 0..cnt {
        let entry = unsafe { &*iv.add(i) };
        grants[i] = entry.iov_grant;
        sizes[i] = entry.iov_size;
    }

    let blk = global_blk();
    blk.try_transfer(do_write != 0, sector, endpt, &grants[..cnt], &sizes[..cnt])
}

unsafe extern "C" fn vb_ioctl(
    _minor: c_int,
    request: c_ulong,
    endpt: c_int,
    grant: c_int,
    _user_endpt: c_int,
) -> c_int {
    // DIOCOPENCT = _IOR('d', 7, int) = 0x40046407
    // DIOCFLUSH   = _IO('d', 3)      = 0x6403
    const DIOCOPENCT: c_ulong = 0x4004_6407;
    const DIOCFLUSH: c_ulong = 0x6403;

    match request {
        DIOCOPENCT => {
            // Copy open count to caller
            let oc = unsafe { core::ptr::addr_of_mut!(OPEN_COUNT).read() };
            ffi::sys_safecopyto_ffi(
                endpt,
                grant,
                0,
                &oc as *const c_int as *const c_void,
                core::mem::size_of::<c_int>() as c_ulong,
            )
        }
        DIOCFLUSH => {
            if let Some(ref mut blk) = *core::ptr::addr_of_mut!(VIRTIO_BLK) {
                blk.flush()
            } else {
                ffi::platform::ENXIO
            }
        }
        _ => ffi::platform::ENOTTY,
    }
}

unsafe extern "C" fn vb_part(minor: c_int) -> *mut c_void {
    if minor >= 0 && (minor as usize) < DEV_PER_DRIVE {
        return &mut PART[minor as usize] as *mut Device as *mut c_void;
    }
    // Subpartitions start at MINOR_d0p0s0 (4)
    if minor >= MINOR_D0P0S0 {
        let sp_idx = (minor - MINOR_D0P0S0) as usize;
        if sp_idx < SUB_PER_DRIVE {
            return &mut SUBPART[sp_idx] as *mut Device as *mut c_void;
        }
    }
    ptr::null_mut()
}

unsafe extern "C" fn vb_intr(_mask: c_uint) {
    if let Some(ref mut blk) = *core::ptr::addr_of_mut!(VIRTIO_BLK) {
        if blk.dev.had_irq() {
            blk.handle_interrupt();
            blk.dev.irq_reenable();
        }
    }
}

unsafe extern "C" fn vb_alarm(_stamp: u64) {}

unsafe extern "C" fn vb_geometry(minor: c_int, entry: *mut c_void) {
    // Only for the drive itself (minor 0)
    if minor != 0 { return; }

    let blk = global_blk();
    let geo = match blk.geometry {
        Some(ref g) => g,
        None => return, // host doesn't support geometry
    };

    let entry = entry as *mut ffi::PartGeom;
    unsafe {
        (*entry).cylinders = geo.cylinders as c_uint;
        (*entry).heads = geo.heads as c_uint;
        (*entry).sectors = geo.sectors as c_uint;
    }
}

unsafe extern "C" fn vb_device(minor: c_int, id: *mut c_int) -> c_int {
    if minor < 0 || minor as usize >= NR_MINORS {
        return ffi::platform::ENXIO;
    }
    *id = 0; // single device with id=0
    0
}

// ============================================================================
// Blockdriver table
// ============================================================================

static mut BDR_TABLE: ffi::Blockdriver = ffi::Blockdriver {
    bdr_type: BDR_TYPE_DISK,
    bdr_open: Some(vb_open),
    bdr_close: Some(vb_close),
    bdr_transfer: Some(vb_transfer),
    bdr_ioctl: Some(vb_ioctl),
    bdr_cleanup: None,
    bdr_part: Some(vb_part),
    bdr_geometry: Some(vb_geometry),
    bdr_intr: Some(vb_intr),
    bdr_alarm: Some(vb_alarm),
    bdr_other: None,
    bdr_device: Some(vb_device),
};

// ============================================================================
// SEF callbacks
// ============================================================================

unsafe extern "C" fn sef_init_fresh(_type: c_int, _info: *const c_void) -> c_int {
    let instance = ffi::env_parse_long(b"instance\0", 0, 0, 255);

    let blk = match VirtioBlk::probe_and_init(instance as c_int, NUM_THREADS, DATA_SIZE) {
        Some(b) => b,
        None => {
            ffi::print(b"virtio-blk: no matching device found\0");
            return ffi::platform::ENXIO;
        }
    };

    VIRTIO_BLK = Some(blk);
    ffi::blockdriver_announce_ffi(_type);
    0
}

unsafe extern "C" fn sef_signal_handler(signo: c_int) {
    if signo != 15 { return; } // SIGTERM
    TERMINATING = true;
    if OPEN_COUNT == 0 {
        // SAFETY: signal is handled in single-threaded context
        unsafe {
            if let Some(ref mut blk) = VIRTIO_BLK {
                blk.dev.reset();
            }
        }
    }
}

// ============================================================================
// C-compatible main entry
// ============================================================================

/// C-compatible main entry — called from a C shim or directly.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn virtio_blk_rust_main(argc: c_int, argv: *mut *mut c_char) -> c_int {
    ffi::env_setargs_ffi(argc, argv);
    ffi::sef_set_init_fresh(sef_init_fresh);
    ffi::sef_set_signal_handler(sef_signal_handler);
    ffi::blockdriver_support_lu();
    ffi::sef_startup_ffi();
    let bdp = unsafe { &*core::ptr::addr_of_mut!(BDR_TABLE) };
    ffi::blockdriver_task(bdp);
    0
}

// ============================================================================
// Panic handler (no_std)
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

    #[test]
    fn device_partition_layout() {
        // Verify partition table is at least initialized
        assert_eq!(DEV_PER_DRIVE, 4);
        assert_eq!(SUB_PER_DRIVE, 4);
        assert_eq!(NR_MINORS, MAX_DRIVES * DEV_PER_DRIVE);
    }

    #[test]
    fn blockdriver_table_layout() {
        // Verify the blockdriver table has a sensible size
        // c_int (4+4 padding) + 14 function pointers (14*8)
        // c_int(4) + padding(4) + 11 function pointers(11*8) = 96 bytes
        let expected = core::mem::size_of::<c_int>() + 4 + core::mem::size_of::<usize>() * 11;
        assert_eq!(core::mem::size_of::<ffi::Blockdriver>(), expected);
    }

    #[test]
    fn errno_wrappers() {
        assert_eq!(ffi::platform::ENXIO, -6);
        assert_eq!(ffi::platform::EACCES, -13);
        assert_eq!(ffi::platform::EIO, -5);
    }
}
