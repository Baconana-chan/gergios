//! # virtio-net — Rust Virtio Network Device Driver for MINIX (Phase 3 Pilot)
//!
//! Implements a network driver for virtio-net devices using the legacy (pre-1.0)
//! PCI transport.  Uses the netdriver framework with a single thread.
//!
//! Architecture:
//!   - PCI probe via `VirtioDevice::probe()` for transitional virtio-net (0x1000)
//!   - I/O port BAR for register access (legacy virtio)
//!   - Split virtqueue (descriptor/avail/used rings)
//!   - netdriver interface with RX/TX queue management

#![no_std]

pub mod device;
pub mod driver;
pub mod ffi;
pub mod net;
pub mod queue;

use core::ffi::{c_char, c_int, c_uint};
use core::ptr;

use driver::VirtioNet;
use ffi::NetdriverAddr;

// ============================================================================
// Global virtio-net state
// ============================================================================

static mut VIRTIO_NET: Option<VirtioNet> = None;
static mut TERMINATING: bool = false;

fn global_net() -> &'static mut VirtioNet {
    unsafe { &mut *(*ptr::addr_of_mut!(VIRTIO_NET)).as_mut().expect("virtio-net: not initialized") }
}

// ============================================================================
// Netdriver callbacks
// ============================================================================

unsafe extern "C" fn ndr_init(
    instance: c_uint,
    addr: *mut NetdriverAddr,
    caps: *mut u32,
    _ticks: *mut c_uint,
) -> c_int {
    let mut net = match VirtioNet::probe_and_init(instance as c_int) {
        Some(n) => n,
        None => return ffi::ENXIO,
    };

    // Fill in MAC address
    unsafe {
        *addr = net.mac;
    }

    // Set capabilities
    unsafe {
        *caps = ffi::NDEV_CAP_MCAST | ffi::NDEV_CAP_BCAST | ffi::NDEV_CAP_HWADDR;
    }

    // Pre-fill RX queue before setting DRV_OK, so the device can
    // start receiving packets immediately once ready.
    net.refill_rx_queue();

    // Register IRQ and set DRV_OK
    if net.dev.ready().is_err() {
        return ffi::ENXIO;
    }

    VIRTIO_NET = Some(net);

    ffi::OK
}

unsafe extern "C" fn ndr_stop() {
    if !unsafe { ptr::addr_of_mut!(TERMINATING).read() } {
        let net = global_net();
        net.stop();
    }
}

unsafe extern "C" fn ndr_set_mode(
    _mode: u32,
    _mcast_list: *const NetdriverAddr,
    _mcast_count: c_uint,
) {
    // No special mode handling needed for the pilot — virtio devices
    // handle promisc/multicast through the control virtqueue (optional).
    // For now, accept all packets (the host decides what to deliver).
}

unsafe extern "C" fn ndr_set_hwaddr(_hwaddr: *const NetdriverAddr) {
    // MAC address is set by the host; we don't support changing it.
}

unsafe extern "C" fn ndr_send(data: *mut ffi::NetdriverData, size: usize) -> c_int {
    unsafe { global_net().send(data, size) }
}

unsafe extern "C" fn ndr_recv(data: *mut ffi::NetdriverData, max: usize) -> isize {
    unsafe { global_net().recv(data, max) }
}

unsafe extern "C" fn ndr_get_link(media: *mut u32) -> c_uint {
    unsafe {
        let net = global_net();
        let (link, med) = net.get_link();
        *media = med;
        link
    }
}

unsafe extern "C" fn ndr_intr(_mask: c_uint) {
    unsafe {
        global_net().handle_intr();
    }
}

unsafe extern "C" fn ndr_tick() {
    // Periodic polling not needed for virtio (interrupt-driven).
}

// ============================================================================
// Netdriver table
// ============================================================================

static mut NDR_TABLE: ffi::Netdriver = ffi::Netdriver {
    ndr_name: b"vio\0" as *const u8 as *const c_char,
    ndr_init: Some(ndr_init),
    ndr_stop: Some(ndr_stop),
    ndr_set_mode: Some(ndr_set_mode),
    ndr_set_hwaddr: Some(ndr_set_hwaddr),
    ndr_recv: Some(ndr_recv),
    ndr_send: Some(ndr_send),
    ndr_get_link: Some(ndr_get_link),
    ndr_intr: Some(ndr_intr),
    ndr_tick: Some(ndr_tick),
};

// ============================================================================
// SEF signal handler
// ============================================================================

unsafe extern "C" fn sef_signal_handler(signo: c_int) {
    if signo != 15 { return; } // SIGTERM only

    unsafe {
        ptr::addr_of_mut!(TERMINATING).write(true);

        if let Some(ref mut net) = *ptr::addr_of_mut!(VIRTIO_NET) {
            net.dev.reset();
        }
    }
}

// ============================================================================
// C-compatible main entry
// ============================================================================

/// C-compatible main entry — called from a C shim directly.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn virtio_net_rust_main(argc: c_int, argv: *mut *mut c_char) -> c_int {
    ffi::env_setargs_ffi(argc, argv);

    // Register SEF signal handler for clean shutdown on SIGTERM.
    // (SEF init is handled by the C framework — ours is a direct netdriver task)
    ffi::platform::sef_setcb_signal_handler(Some(sef_signal_handler));

    unsafe {
        let ndp = ptr::addr_of_mut!(NDR_TABLE);
        ffi::netdriver_task(&*ndp);
    }

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
    fn ndr_table_size() {
        // 1 name ptr + 9 callbacks = 10 pointer-sized fields
        let expected = 10 * core::mem::size_of::<usize>();
        assert_eq!(core::mem::size_of::<ffi::Netdriver>(), expected);
    }

    #[test]
    fn constant_sanity() {
        assert_eq!(driver::BUF_PACKETS, 64);
        assert_eq!(driver::MAX_PACK_SIZE, 1514);
        assert_eq!(driver::HDR_SIZE, core::mem::size_of::<net::VirtioNetHdr>());
    }
}
