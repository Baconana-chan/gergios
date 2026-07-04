//! # xHCI (USB 3.0) Driver for MINIX — Native Rust Implementation
//!
//! Implements: PCI probe, xHCI controller init, device enumeration,
//! TRB ring management, USB control/bulk/interrupt transfers.
//!
//! ## Architecture
//!
//! ```text
//! lib.rs (main entry, SEF lifecycle, blockdriver integration)
//!   ├── ffi.rs (MINIX C FFI bindings)
//!   ├── registers.rs (xHCI register set, TRB structs, Device Context)
//!   ├── xhci.rs (controller init/stop, port mgmt, command ring ops)
//!   ├── ring.rs (TRB ring management — cmd ring, event ring, transfer rings)
//!   └── devmgr.rs (device slot mgmt, address device, configure endpoint)
//! ```

#![no_std]
#![allow(dead_code)]

mod ffi;
mod registers;
mod ring;
mod usb_device;
mod usb_hid;
mod usb_hub;
mod usb_interface;
mod usb_msc;
mod xhci;

use core::ffi::{c_int, c_uint};
use core::panic::PanicInfo;

use usb_device::UsbClassDriver;
use usb_hid::HidDriver;
use usb_hub::UsbHubDriver;
use usb_msc::MscDevice;
use xhci::XhciController;

/// Global xHCI controller instance (static for C callback access).
static mut XHC: Option<XhciController> = None;

/// Global HUB driver instance.
pub static mut HUB_DRIVER: UsbHubDriver = UsbHubDriver::new_static();

/// Global HID driver instance.
pub static mut HID_DRIVER: HidDriver = HidDriver::new_static();

/// Verbosity level (from env).
static mut VERBOSE: u8 = 0;

// ============================================================================
// Panic Handler
// ============================================================================

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    ffi::print(b"xHCI: panic\0");
    let _ = info.message();
    loop {}
}

// ============================================================================
// Blockdriver Callbacks
// ============================================================================

unsafe extern "C" fn xhci_open(minor: ffi::DevMinor, _access: c_int) -> c_int {
    // Check if minor corresponds to an MSC device
    if minor >= 0 && (minor as usize) < usb_msc::MAX_MSC_DEVICES {
        if let Some(xhc) = unsafe { &mut *core::ptr::addr_of_mut!(XHC) } {
            if (minor as usize) < usb_msc::MAX_MSC_DEVICES && xhc.msc_devices[minor as usize].ready {
                return ffi::OK;
            }
        }
    }
    if minor == 0 {
        ffi::OK
    } else {
        ffi::ENXIO
    }
}

unsafe extern "C" fn xhci_close(_minor: ffi::DevMinor) -> c_int {
    ffi::OK
}

unsafe extern "C" fn xhci_transfer(
    minor: ffi::DevMinor,
    write: c_int,
    pos: u64,
    _proc: c_int,
    _buf: *mut core::ffi::c_void,
    bytes: c_uint,
    _flags: c_int,
) -> isize {
    // Route to USB Mass Storage if minor is an MSC device
    if minor >= 0 && (minor as usize) < usb_msc::MAX_MSC_DEVICES {
        if let Some(xhc) = unsafe { &mut *core::ptr::addr_of_mut!(XHC) } {
            let dev_idx = minor as usize;
            if dev_idx < usb_msc::MAX_MSC_DEVICES && xhc.msc_devices[dev_idx].ready {
                // Use the MSC DMA buffer for data (pre-allocated 64KB)
                // The buffer address is passed via grant, but we need to use
                // safecopy. For now, use a simple DMA bounce buffer approach.
                let block_size = xhc.msc_devices[dev_idx].block_size;
                let count = (bytes as usize + block_size as usize - 1) / block_size as usize;
                let lba = pos / block_size as u64;

                // Allocate a temporary DMA buffer for this transfer
                let transfer_len = (count as u32) * block_size;
                let dma_size = core::cmp::max(transfer_len as usize, 512);
                let (dma_virt, dma_phys) = match ffi::alloc_contig_ffi(dma_size) {
                    Some((v, p)) => (v as *mut u8, p),
                    None => return ffi::ENOMEM as isize,
                };

                let result = if write != 0 {
                    // Write: data comes from _buf via safecopy
                    // For now, this is a stub — real implementation would use
                    // sys_safecopyfrom to copy user data into DMA buffer
                    let n = usb_msc::msc_write(xhc, dev_idx, lba, count, dma_phys);
                    if n < 0 { ffi::EIO as isize } else { n }
                } else {
                    // Read: data goes to _buf via safecopy
                    let n = usb_msc::msc_read(xhc, dev_idx, lba, count, dma_phys);
                    if n < 0 { ffi::EIO as isize } else { n }
                };

                ffi::free_contig_ffi(dma_virt as *mut core::ffi::c_void, dma_size);
                return result;
            }
        }
    }
    ffi::ENXIO as isize
}

unsafe extern "C" fn xhci_ioctl(
    minor: ffi::DevMinor,
    request: core::ffi::c_ulong,
    _endpt: c_int,
    _grant: c_int,
    _endpt2: c_int,
) -> c_int {
    match (minor, request) {
        (0, ..) => ffi::OK,
        _ => ffi::ENOTTY,
    }
}

unsafe extern "C" fn xhci_part(_minor: ffi::DevMinor) -> *mut core::ffi::c_void {
    core::ptr::null_mut()
}

unsafe extern "C" fn xhci_intr(_mask: c_uint) {
    // Handle interrupts from xHC
    if let Some(xhc) = unsafe { &mut *core::ptr::addr_of_mut!(XHC) } {
        // Process event ring
        let mut handled = false;
        while let Some(_event) = xhc.event_ring.next_event() {
            handled = true;
            // Dispatch based on event type
            // (actual dispatch in xhci.poll_event_ring is poll-based)
        }
        if handled {
            // Update ERDP
            let erdp = xhc.event_ring.dequeue_phys() | crate::registers::rt::erdp::EHB;
            xhc.rt_w64(0, crate::registers::rt::ERDP, erdp);
        }
        // Re-enable IRQ
        if xhc.msix_available {
            let _ = ffi::msix_setup(xhc.irq);
        } else {
            let _ = ffi::irq_remove(&mut xhc.hook_id);
            let _ = ffi::irq_setup(xhc.irq);
        }
    }
}

unsafe extern "C" fn xhci_alarm(_stamp: u64) {
    // Periodic tick — scan for port changes
    if let Some(xhc) = unsafe { &mut *core::ptr::addr_of_mut!(XHC) } {
        for port in 1..=xhc.max_ports {
            if xhc.port_connected(port) {
                let speed = xhc.port_speed(port);
                if speed > 0 && xhc.port_link_state(port) == 0 {
                    let sc = xhc.portsc(port);
                    if (sc & crate::registers::op::portsc::CSC) != 0 {
                        xhc.set_portsc(port, sc | crate::registers::op::portsc::CSC);

                        // Allocate a buffer for full enumeration
                        let mut enum_buf = match crate::ring::RingMem::alloc(512) {
                            Some(b) => b,
                            None => continue,
                        };

                        // Full enumeration with descriptor reading + class dispatch
                        xhc.enumerate_port_full(port, speed,
                            enum_buf.virt, enum_buf.phys, enum_buf.size);

                        // Fallback: Try legacy MSC probe for devices already
                        // enumerated but not claimed by a class driver
                        let slot_id = xhc.slots.iter().position(|s| {
                            s.assigned && s.port == port && !xhc.msc_devices.iter().any(|d| d.slot_id == s.id)
                        }).map(|i| i as u8);

                        if let Some(sid) = slot_id {
                            if sid > 0 {
                                // Check if already handled by device framework
                                if xhc.device_registry.find_by_slot(sid).is_none() {
                                    let result = usb_msc::probe_msc_device(
                                        xhc, sid, enum_buf.virt, enum_buf.phys, 512
                                    );
                                    if result.is_some() {
                                        ffi::print(b"xHCI: USB Mass Storage detected\0");
                                    }
                                }
                            }
                        }

                        enum_buf.free();
                    }
                }
            }
        }
    }
}

unsafe extern "C" fn xhci_device(minor: ffi::DevMinor, dev_id: *mut c_int) -> c_int {
    if minor == 0 {
        *dev_id = 0;
        ffi::OK
    } else {
        ffi::ENXIO
    }
}

// ============================================================================
// SEF Lifecycle
// ============================================================================

unsafe extern "C" fn sef_init_fresh(_type: c_int, _info: *const core::ffi::c_void) -> c_int {
    ffi::print(b"xHCI: initializing...\0");

    let skip = ffi::env_parse_long(b"instance\0", 0, 0, 10);
    let verbose = ffi::env_parse_long(b"verbose\0", 0, 0, 3) as u8;
    unsafe { VERBOSE = verbose; }

    // Parse device instance from env
    let devind = match XhciController::probe(skip as c_int) {
        Some(d) => d,
        None => {
            ffi::print(b"xHCI: no controller found\0");
            return ffi::EIO;
        }
    };

    let mut xhc = match XhciController::init(devind, verbose) {
        Some(x) => x,
        None => {
            ffi::print(b"xHCI: init failed\0");
            return ffi::EIO;
        }
    };

    // Initialize ports
    xhc.init_ports();

    // Register class drivers
    // The Hub driver for downstream port management
    {
        let xhc = &mut xhc;
        let hub_verbose = verbose;
        let hub_drv: &'static mut crate::usb_hub::UsbHubDriver =
            unsafe { &mut *core::ptr::addr_of_mut!(crate::HUB_DRIVER) };
        if hub_drv.class_code() == crate::registers::usb_class::HUB {
            xhc.device_registry.register_driver(hub_drv);
        }
    }
    // The HID driver for keyboards and mice
    {
        let xhc = &mut xhc;
        let hid_drv: &'static mut crate::usb_hid::HidDriver =
            unsafe { &mut *core::ptr::addr_of_mut!(crate::HID_DRIVER) };
        if hid_drv.class_code() == crate::registers::usb_class::HID {
            xhc.device_registry.register_driver(hid_drv);
        }
    }

    // Store as global
    unsafe { XHC = Some(xhc); }

    // Announce to blockdriver framework
    ffi::blockdriver_support_lu();
    ffi::blockdriver_announce_ffi(0);

    ffi::print(b"xHCI: ready\0");
    ffi::OK
}

unsafe extern "C" fn sef_signal_handler(signo: c_int) {
    if signo == 0x1001 {
        ffi::print(b"xHCI: shutting down...\0");
        if let Some(xhc) = unsafe { &mut *core::ptr::addr_of_mut!(XHC) } {
            xhc.stop();
        }
        ffi::blockdriver_terminate();
    }
}

// ============================================================================
// Entry Point
// ============================================================================

/// xHCI driver entry point (called from C main).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn xhci_rust_main(argc: c_int, argv: *mut *mut core::ffi::c_char) {
    ffi::sef_set_init_fresh(sef_init_fresh);
    ffi::sef_set_signal_handler(sef_signal_handler);
    ffi::sef_startup_ffi();

    // Blockdriver table
    let bd = ffi::Blockdriver {
        bdr_type: 0,
        bdr_open: Some(xhci_open),
        bdr_close: Some(xhci_close),
        bdr_transfer: Some(xhci_transfer),
        bdr_ioctl: Some(xhci_ioctl),
        bdr_part: Some(xhci_part),
        bdr_intr: Some(xhci_intr),
        bdr_alarm: Some(xhci_alarm),
        bdr_device: Some(xhci_device),
    };
    ffi::blockdriver_task(&bd);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xhci_crate_exists() {
        // Basic compile check
        assert_eq!(core::mem::size_of::<ffi::Blockdriver>(), 8 * core::mem::size_of::<usize>());
    }
}
