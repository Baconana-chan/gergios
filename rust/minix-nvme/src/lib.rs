//! # minix-nvme — NVMe (NVM Express) SSD Driver for MINIX
//!
//! Native Rust implementation following the patterns from minix-ahci and
//! virtio-blk: `no_std`, PCI probe, MMIO BAR0, admin/I/O queues,
//! blockdriver_mt interface with SEF lifecycle.
//!
//! ## Architecture
//!
//! ```ignore
//! PCI probe → BAR0 MMIO → disable controller → admin queue setup →
//! enable controller → Identify → I/O queues → blockdriver_mt_task()
//! ```

#![cfg_attr(target_os = "minix", no_std)]

pub mod ffi;
pub mod registers;
pub mod controller;

use core::ffi::{c_char, c_int, c_uint, c_ulong, c_void};
use core::ptr;

use registers::SqEntry;
use controller::{NvmeController, PartGeom};
use minix_driver::mmio::*;

// ============================================================================
// Constants
// ============================================================================

const BDR_TYPE_DISK: c_int = 0;
const BDEV_W_BIT: c_int = 0x0002;
const DEV_PER_DRIVE: usize = 4;
const MAX_DRIVES: usize = 1;              // Single NVMe controller
const NR_MINORS: usize = MAX_DRIVES * DEV_PER_DRIVE;
const SUB_PER_DRIVE: usize = 4;
const MINOR_D0P0S0: c_int = 4;

/// Temporary transfer buffer size (64KB — enough for typical I/O).
const DATA_BUF_SIZE: usize = 65536;

// ============================================================================
// Global state
// ============================================================================

static mut NVME: Option<NvmeController> = None;
static mut OPEN_COUNT: c_int = 0;
static mut TERMINATING: bool = false;
static mut DATA_BUF_VIRT: *mut u8 = ptr::null_mut();
static mut DATA_BUF_PHYS: u64 = 0;

/// SAFETY: only called from single-threaded blockdriver/SEF context.
fn global_nvme() -> &'static mut NvmeController {
    unsafe { &mut *core::ptr::addr_of_mut!(NVME) }
        .as_mut()
        .expect("NVMe: not initialized")
}

// ============================================================================
// Partition table
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

fn map_minor_to_nsid(minor: c_int) -> u32 {
    // Minor 0 → NSID 1 (single namespace for now)
    1
}

fn map_minor_to_lba_offset(minor: c_int) -> u64 {
    // Minor 0 → LBA 0 (base of namespace)
    0
}

// ============================================================================
// Blockdriver callbacks
// ============================================================================

unsafe extern "C" fn nvme_open(minor: c_int, access: c_int) -> c_int {
    if minor < 0 || minor as usize >= NR_MINORS {
        return ffi::ENXIO;
    }

    let dev = nvme_part(minor);
    if dev.is_null() { return ffi::ENXIO; }

    // Check read-only (NVMe is read-write)
    if (access & BDEV_W_BIT) == 0 {
        // Allow read-only access
    }

    if OPEN_COUNT == 0 {
        // First open: set up partition table
        let nvme = global_nvme();
        let nsid = map_minor_to_nsid(minor);
        let idx = (nsid - 1) as usize;
        if idx < 32 {
            let total_bytes = nvme.ns_block_count[idx] * nvme.ns_block_size[idx] as u64;
            PART[0] = Device {
                dv_base: 0,
                dv_size: total_bytes,
            };
        }
        // Reset subpartitions
        let subpart = unsafe { &mut *core::ptr::addr_of_mut!(SUBPART) };
        for s in subpart.iter_mut() {
            *s = Device { dv_base: 0, dv_size: 0 };
        }
        ffi::blockdriver_set_workers(0, 1);
    }

    OPEN_COUNT += 1;
    ffi::OK
}

unsafe extern "C" fn nvme_close(minor: c_int) -> c_int {
    if minor < 0 || minor as usize >= NR_MINORS { return ffi::OK; }
    if OPEN_COUNT == 0 { return ffi::EINVAL; }

    OPEN_COUNT -= 1;

    if OPEN_COUNT == 0 && TERMINATING {
        if let Some(ref mut nvme) = *core::ptr::addr_of_mut!(NVME) {
            nvme.stop();
        }
        ffi::blockdriver_terminate();
    }

    ffi::OK
}

unsafe extern "C" fn nvme_transfer(
    minor: c_int,
    do_write: c_int,
    position: u64,
    endpt: c_int,
    iovec: *mut c_void,
    count: c_uint,
    _flags: c_int,
) -> isize {
    if minor < 0 || minor as usize >= NR_MINORS {
        return ffi::ENXIO as isize;
    }

    let nvme = global_nvme();
    let nsid = map_minor_to_nsid(minor);
    let idx = (nsid - 1) as usize;

    if idx >= 32 || nvme.ns_block_size[idx] == 0 {
        return ffi::ENXIO as isize;
    }

    let blk_size = nvme.ns_block_size[idx] as u64;
    let lba = position / blk_size;
    if position % blk_size != 0 {
        return ffi::EINVAL as isize;
    }

    // Limit transfer size
    let cnt = core::cmp::min(count as usize, 64) as u32;
    let total_bytes = cnt as u64 * blk_size;

    if total_bytes > DATA_BUF_SIZE as u64 {
        return ffi::EINVAL as isize;
    }

    // For now, use a pre-allocated DMA buffer
    let data_buf_virt = unsafe { DATA_BUF_VIRT };
    let data_buf_phys = unsafe { DATA_BUF_PHYS };
    if data_buf_virt.is_null() {
        return ffi::ENOMEM as isize;
    }

    if do_write != 0 {
        // Copy from user via safecopy
        let iv = iovec as *const crate::ffi::IoVec;
        let mut offset: usize = 0;
        let cnt_usize = count as usize;
        for i in 0..cnt_usize {
            let entry = unsafe { &*iv.add(i) };
            let grant = entry.iovec_grant;
            let size = entry.iovec_size;
            if offset + size > DATA_BUF_SIZE {
                break;
            }
            let r = ffi::sys_safecopyfrom_ffi(
                endpt, grant, 0,
                unsafe { data_buf_virt.add(offset) } as *mut c_void,
                size as c_ulong,
            );
            if r != ffi::OK { return ffi::EIO as isize; }
            offset += size;
        }
    }

    // Issue NVMe I/O
    let success = nvme.io_transfer(nsid, lba, cnt, do_write != 0, data_buf_phys, 0);
    if !success {
        return ffi::EIO as isize;
    }

    if do_write == 0 {
        // Copy to user
        let iv = iovec as *const crate::ffi::IoVec;
        let mut offset: usize = 0;
        let cnt_usize = count as usize;
        for i in 0..cnt_usize {
            let entry = unsafe { &*iv.add(i) };
            let grant = entry.iovec_grant;
            let size = entry.iovec_size;
            if offset + size > DATA_BUF_SIZE {
                break;
            }
            let r = ffi::sys_safecopyto_ffi(
                endpt, grant, 0,
                unsafe { data_buf_virt.add(offset) } as *const c_void,
                size as c_ulong,
            );
            if r != ffi::OK { return ffi::EIO as isize; }
            offset += size;
        }
    }

    total_bytes as isize
}

unsafe extern "C" fn nvme_ioctl(
    _minor: c_int,
    request: c_ulong,
    endpt: c_int,
    grant: c_int,
    _user_endpt: c_int,
) -> c_int {
    const DIOCOPENCT: c_ulong = 0x4004_6407;
    const DIOCFLUSH: c_ulong = 0x6403;

    match request {
        DIOCOPENCT => {
            let oc = unsafe { core::ptr::addr_of_mut!(OPEN_COUNT).read() };
            ffi::sys_safecopyto_ffi(
                endpt, grant, 0,
                &oc as *const c_int as *const c_void,
                core::mem::size_of::<c_int>() as c_ulong,
            )
        }
        DIOCFLUSH => {
            let nvme = global_nvme();
            let nsid = 1;
            if nvme.io_flush(nsid) {
                ffi::OK
            } else {
                ffi::EIO
            }
        }
        _ => ffi::ENOTTY,
    }
}

unsafe extern "C" fn nvme_part(minor: c_int) -> *mut c_void {
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

unsafe extern "C" fn nvme_intr(_mask: c_uint) {
    // Process completions on all active I/O queues (MSI-X per-queue vectors)
    if let Some(ref mut nvme) = *core::ptr::addr_of_mut!(NVME) {
        nvme.process_all_queues();
    }
}

unsafe extern "C" fn nvme_alarm(_stamp: u64) {}

unsafe extern "C" fn nvme_device(minor: c_int, id: *mut c_int) -> c_int {
    if minor < 0 || minor as usize >= NR_MINORS {
        return ffi::ENXIO;
    }
    *id = 0; // single device
    ffi::OK
}

// ============================================================================
// Blockdriver table
// ============================================================================

static mut BDR_TABLE: ffi::Blockdriver = ffi::Blockdriver {
    bdr_type: BDR_TYPE_DISK,
    bdr_open: Some(nvme_open),
    bdr_close: Some(nvme_close),
    bdr_transfer: Some(nvme_transfer),
    bdr_ioctl: Some(nvme_ioctl),
    bdr_part: Some(nvme_part),
    bdr_intr: Some(nvme_intr),
    bdr_alarm: Some(nvme_alarm),
    bdr_device: Some(nvme_device),
};

// ============================================================================
// SEF callbacks
// ============================================================================

unsafe extern "C" fn sef_init_fresh(_type: c_int, _info: *const c_void) -> c_int {
    let instance = ffi::env_parse_long(b"instance\0", 0, 0, 255);
    let verbose = ffi::env_parse_long(b"nvme_verbose\0", 1, 0, 4) as u8;

    // Probe and init NVMe controller
    let devind = match NvmeController::probe(instance as c_int) {
        Some(d) => d,
        None => {
            ffi::print(b"NVMe: no matching device found\0");
            return ffi::ENXIO;
        }
    };

    let nvme = match NvmeController::init(devind, instance as c_int, verbose) {
        Some(n) => n,
        None => {
            ffi::print(b"NVMe: controller init failed\0");
            return ffi::EIO;
        }
    };

    // Allocate a DMA buffer for data transfers
    let (vbuf, pbuf) = match ffi::alloc_contig_ffi(DATA_BUF_SIZE) {
        Some((v, p)) => (v as *mut u8, p),
        None => {
            ffi::print(b"NVMe: unable to allocate DMA buffer\0");
            return ffi::ENOMEM;
        }
    };
    DATA_BUF_VIRT = vbuf;
    DATA_BUF_PHYS = pbuf;

    NVME = Some(nvme);
    ffi::blockdriver_announce_ffi(_type);

    ffi::print(b"NVMe: driver initialized\0");
    ffi::OK
}

unsafe extern "C" fn sef_signal_handler(signo: c_int) {
    if signo != 15 { return; } // SIGTERM
    TERMINATING = true;
    if OPEN_COUNT == 0 {
        unsafe {
            if let Some(ref mut nvme) = NVME {
                nvme.stop();
            }
            if !DATA_BUF_VIRT.is_null() {
                ffi::free_contig_ffi(DATA_BUF_VIRT as *mut core::ffi::c_void, DATA_BUF_SIZE);
                DATA_BUF_VIRT = ptr::null_mut();
                DATA_BUF_PHYS = 0;
            }
        }
    }
}

// ============================================================================
// C-compatible main entry
// ============================================================================

/// C-compatible main entry — called from a C shim or directly.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nvme_rust_main(argc: c_int, argv: *mut *mut c_char) -> c_int {
    ffi::env_setargs_ffi(argc, argv);
    ffi::sef_set_init_fresh(sef_init_fresh);
    ffi::sef_set_signal_handler(sef_signal_handler);
    ffi::blockdriver_support_lu();
    ffi::sef_startup_ffi();
    let bdp = unsafe { &*core::ptr::addr_of_mut!(BDR_TABLE) };
    ffi::blockdriver_task(bdp);
    ffi::OK
}

// ============================================================================
// Panic handler
// ============================================================================

#[cfg(all(not(test), target_os = "minix"))]
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
    fn constants() {
        assert_eq!(BDR_TYPE_DISK, 0);
        assert_eq!(BDEV_W_BIT, 0x0002);
        assert_eq!(DEV_PER_DRIVE, 4);
        assert_eq!(NR_MINORS, MAX_DRIVES * DEV_PER_DRIVE);
    }

    #[test]
    fn device_partition_layout() {
        // PART/SUBPART are mutable statics; read lengths via addr_of! to avoid
        // creating a reference to a mutable static (UB).
        assert_eq!(unsafe { (*core::ptr::addr_of!(PART)).len() }, 4);
        assert_eq!(unsafe { (*core::ptr::addr_of!(SUBPART)).len() }, 4);
    }

    #[test]
    fn blockdriver_table_layout() {
        // bdr_type (c_int) + padding to pointer alignment + 8 function pointers
        let expected = 8 + core::mem::size_of::<usize>() * 8;
        assert_eq!(core::mem::size_of::<ffi::Blockdriver>(), expected);
    }

    #[test]
    fn errno_wrappers() {
        assert_eq!(ffi::ENXIO, -6);
        assert_eq!(ffi::EACCES, -13);
        assert_eq!(ffi::EIO, -5);
        assert_eq!(ffi::OK, 0);
    }

    #[test]
    fn minor_mapping() {
        assert_eq!(map_minor_to_nsid(0), 1);
        assert_eq!(map_minor_to_nsid(3), 1);
    }

    #[test]
    fn device_partition_size() {
        assert_eq!(core::mem::size_of::<Device>(), 16);
    }

    #[test]
    fn part_geom_size() {
        assert_eq!(core::mem::size_of::<PartGeom>(), 32);
    }
}
