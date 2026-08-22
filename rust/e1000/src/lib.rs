//! # e1000 — Rust Intel PRO/1000 Gigabit Ethernet Driver for MINIX

#![cfg_attr(target_os = "minix", no_std)]

pub mod ffi;
pub mod reg;
pub mod desc;
pub mod pci_ids;
pub mod eeprom;
pub mod driver;

use core::ffi::{c_char, c_int, c_uint};
use core::ptr;
use driver::E1000;
use minix_driver::work_queue::{PriorityWorkQueue, SoftIrqPriority};

/// Work queue for e1000 bottom-half processing.
///
/// Priority mapping:
/// - Hi: RX/TX events (performance-critical, must be serviced quickly)
/// - Tasklet: Link status changes (less critical)
static E1000_WQ: PriorityWorkQueue = PriorityWorkQueue::new();

// ============================================================================
// Global driver state
// ============================================================================

static mut E1000_STATE: Option<E1000> = None;
static mut TERMINATING: bool = false;

/// SAFETY: called only from single-threaded netdriver context.
fn global_e1000() -> *mut Option<E1000> {
    unsafe { ptr::addr_of_mut!(E1000_STATE) }
}

fn blk() -> &'static mut E1000 {
    unsafe { &mut *(*global_e1000()).as_mut().expect("e1000: not initialized") }
}

// ============================================================================
// Netdriver callbacks
// ============================================================================

unsafe extern "C" fn ndr_init(
    instance: c_uint,
    addr: *mut ffi::NetdriverAddr,
    caps: *mut u32,
    ticks: *mut c_uint,
) -> c_int {
    unsafe {
        let mut dev = E1000::new();
        let devind = dev.probe_v2(instance as c_int);
        if devind < 0 {
            return ffi::ENXIO;
        }

        ffi::tsc_calibrate_ffi();
        dev.init_hw(devind);
        dev.init_buffers();
        dev.enable_intr();

        let mac = &mut *addr;
        dev.read_mac(mac);

        *caps = ffi::NDEV_CAP_MCAST | ffi::NDEV_CAP_BCAST | ffi::NDEV_CAP_HWADDR
            | ffi::NDEV_CAP_CS_IP4_TX | ffi::NDEV_CAP_CS_IP4_RX;
        *ticks = (ffi::get_sys_hz() / 10) as c_uint;

        *global_e1000() = Some(dev);
        ffi::OK
    }
}

unsafe extern "C" fn ndr_stop() {
    // Signal handler may have already reset the hardware.
    if !unsafe { core::ptr::addr_of_mut!(TERMINATING).read() } {
        unsafe { blk().reset_hw(); }
    }
}

unsafe extern "C" fn ndr_set_mode(
    mode: u32,
    _mcast_list: *const ffi::NetdriverAddr,
    _mcast_count: c_uint,
) {
    unsafe {
        let dev = blk();
        let mut rctl = eeprom::read_reg(dev.regs, reg::RCTL);
        rctl &= !(reg::RCTL_BAM | reg::RCTL_MPE | reg::RCTL_UPE);

        if (mode & ffi::NDEV_MODE_BCAST) != 0 { rctl |= reg::RCTL_BAM; }
        if (mode & (ffi::NDEV_MODE_MCAST_LIST | ffi::NDEV_MODE_MCAST_ALL)) != 0 { rctl |= reg::RCTL_MPE; }
        if (mode & ffi::NDEV_MODE_PROMISC) != 0 { rctl |= reg::RCTL_BAM | reg::RCTL_MPE | reg::RCTL_UPE; }

        eeprom::write_reg(dev.regs, reg::RCTL, rctl);
    }
}

unsafe extern "C" fn ndr_set_hwaddr(hwaddr: *const ffi::NetdriverAddr) {
    unsafe { blk().set_hwaddr(&*hwaddr); }
}

unsafe extern "C" fn ndr_send(data: *mut ffi::NetdriverData, size: usize) -> c_int {
    unsafe { blk().send(data, size) }
}

unsafe extern "C" fn ndr_recv(data: *mut ffi::NetdriverData, max: usize) -> isize {
    unsafe { blk().recv(data, max) }
}

unsafe extern "C" fn ndr_get_link(media: *mut u32) -> c_uint {
    unsafe {
        let dev = blk();
        let (link, med) = dev.get_link();
        *media = med;
        link
    }
}

/// Bottom-half work function for e1000 interrupt processing.
/// Runs in worker context, calls netdriver callbacks.
unsafe extern "C" fn e1000_bottom_half(data: usize) {
    unsafe {
        let dev = blk();
        let events = data as u32;

        if (events & 1) != 0 { ffi::netdriver_link_ffi(); }
        if (events & 2) != 0 { ffi::netdriver_recv_ffi(); }
        if (events & 4) != 0 { ffi::netdriver_send_ffi(); }
    }
}

/// Top-half interrupt handler — quick ACK, then schedule bottom-half work.
unsafe extern "C" fn ndr_intr(_mask: c_uint) {
    unsafe {
        let dev = blk();
        let cause = eeprom::read_reg(dev.regs, crate::reg::ICR);

        if cause == 0 {
            // Spurious
            if !dev.msix_available {
                let _ = ffi::irq_reenable_ffi(&dev.irq_hook);
            }
            return;
        }

        // Convert ICR bits to abstract event flags, split by priority:
        // - Link events (less critical) → Tasklet priority
        // - RX/TX events (performance-critical) → Hi priority
        // Abstract flags: 1=link, 2=rx, 4=tx (e1000_bottom_half uses these)
        let mut link_flag = 0u32;
        let mut io_flags = 0u32;
        if (cause & crate::reg::ICR_LSC) != 0 { link_flag = 1; }
        if (cause & (crate::reg::ICR_RXO | crate::reg::ICR_RXT)) != 0 { io_flags |= 2; }
        if (cause & (crate::reg::ICR_TXQE | crate::reg::ICR_TXDW)) != 0 { io_flags |= 4; }

        // Tasklet: link status changes
        if link_flag != 0
            && !E1000_WQ.enqueue(SoftIrqPriority::Tasklet, e1000_bottom_half, link_flag as usize)
        {
            e1000_bottom_half(link_flag as usize);
        }

        // Hi: RX/TX (performance-critical)
        if io_flags != 0
            && !E1000_WQ.enqueue(SoftIrqPriority::Hi, e1000_bottom_half, io_flags as usize)
        {
            e1000_bottom_half(io_flags as usize);
        }

        // Re-enable legacy IRQ
        if !dev.msix_available {
            let _ = ffi::irq_reenable_ffi(&dev.irq_hook);
        }
    }
}

/// Drain pending e1000 bottom-half work.
pub fn drain_e1000_work() {
    E1000_WQ.process_all();
}

unsafe extern "C" fn ndr_tick() {
    unsafe {
        drain_e1000_work();
        blk().update_stats();
    }
}

// ============================================================================
// Netdriver table
// ============================================================================

static mut NDR_TABLE: ffi::Netdriver = ffi::Netdriver {
    ndr_name: b"e1000\0" as *const u8 as *const c_char,
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
unsafe extern "C" fn sef_signal_handler(signo: c_int) {
    // Only handle SIGTERM (signal 15)
    if signo != 15 { return; }

    unsafe {
        core::ptr::addr_of_mut!(TERMINATING).write(true);

        let dev = &mut *global_e1000();
        // Rust 2024: matching on &mut implicitly borrows, no `ref mut` needed.
        if let Some(e) = dev {
            // Reset hardware immediately to stop DMA and interrupts.
            e.reset_hw();
        }
    }
}

// ============================================================================
// C-compatible main entry
// ============================================================================

#[unsafe(no_mangle)]
pub unsafe extern "C" fn e1000_rust_main(argc: c_int, argv: *mut *mut c_char) -> c_int {
    ffi::env_setargs_ffi(argc, argv);

    // Register SEF signal handler for clean shutdown on SIGTERM.
    ffi::sef_set_signal_handler(sef_signal_handler);

    unsafe {
        let ndp = ptr::addr_of_mut!(NDR_TABLE);
        ffi::netdriver_task(&*ndp);
    }

    0
}

// ============================================================================
// Panic handler
// ============================================================================

#[cfg(all(not(test), target_os = "minix"))]
#[panic_handler]
fn panic_handler(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ndr_table_size() {
        // 1 name ptr + 9 callbacks = 10 pointer-sized fields
        let expected = 10 * core::mem::size_of::<usize>();
        assert_eq!(core::mem::size_of::<ffi::Netdriver>(), expected);
    }
}
