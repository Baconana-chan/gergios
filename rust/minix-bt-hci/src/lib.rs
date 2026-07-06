//! # GergiOS Bluetooth HCI USB Transport Driver
//!
//! A native Rust driver for USB Bluetooth adapters (class 0xE0, subclass 0x01).
//! Exposes `/dev/hci0` through the MINIX chardriver framework.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌──────────────┐
//! │  /dev/hci0  │────▶│  chardev.rs  │────▶│ usb_transport│
//! │  (userland) │     │  ioctl/read  │     │  USB bulk EP │
//! └─────────────┘     │  /write      │     │  async pool   │
//!                     └──────────────┘     └──────┬───────┘
//!                                                │
//!                                     ┌──────────▼────────┐
//!                                     │    hci.rs         │
//!                                     │  HCI cmd/event    │
//!                                     │  ACL/SCO packet    │
//!                                     └───────────────────┘
//! ```
//!
//! ## Supported hardware
//!
//! - Intel Wireless-AC 3160/7260/8260 (VID 0x8086)
//! - Broadcom BCM20702 / BCM4335
//! - Realtek RTL8761B / RTL8821CE
//! - CSR Cambridge Silicon Radio (generic)
//! - MediaTek MT7921 / MT7922

pub mod chardev;
pub mod ffi;
pub mod hci;
pub mod ids;
pub mod uart_transport;
pub mod usb_transport;

use std::boxed::Box;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use usb_transport::HciUsbTransport;

/// Global driver state — shared between chardev callbacks and USB event loop.
static DRIVER_STATE: AtomicPtr<DriverInner> = AtomicPtr::new(std::ptr::null_mut());

/// Driver has been initialised at least once.
static INITIALIZED: AtomicBool = AtomicBool::new(false);

// ── Driver inner state ──────────────────────────────────────────────

struct DriverInner {
    transport: Arc<HciUsbTransport>,
}

// ── SEF lifecycle callbacks ─────────────────────────────────────────

/// Called once at driver startup — probe USB for a BT adapter.
fn driver_init() -> i32 {
    let transport = match HciUsbTransport::probe() {
        Ok(t) => Arc::new(t),
        Err(_e) => {
            ffi::print(b"bt-hci: no Bluetooth adapter found\n\0");
            return 0;
        }
    };

    let inner = Box::into_raw(Box::new(DriverInner { transport }));
    DRIVER_STATE.store(inner, Ordering::SeqCst);
    INITIALIZED.store(true, Ordering::SeqCst);

    ffi::print(b"bt-hci: driver initialised\n\0");
    0
}

/// Cleanup — free transport resources.
fn driver_cleanup() {
    if let Some(inner) = unsafe { DRIVER_STATE.swap(std::ptr::null_mut(), Ordering::SeqCst).as_mut() } {
        unsafe { let _ = Box::from_raw(inner); }
    }
    INITIALIZED.store(false, Ordering::SeqCst);
}

// ── Startup / chardriver loop ───────────────────────────────────────

/// Entry point called by the MINIX init/SEF framework.
#[cfg(not(test))]
#[no_mangle]
pub extern "C" fn main(_argc: i32, _argv: *const *const u8) -> i32 {
    unsafe { ffi::sef_startup_ffi(); }

    let rc = driver_init();
    if rc != 0 { return rc; }

    unsafe {
        ffi::chardriver_announce(-1);
        ffi::chardriver_task(&chardev::as_chardriver());
    }

    driver_cleanup();
    0
}

/// Return a pointer to the transport, or `None` if not initialised.
fn get_transport() -> Option<Arc<HciUsbTransport>> {
    let ptr = DRIVER_STATE.load(Ordering::Acquire);
    if ptr.is_null() { return None; }
    Some(unsafe { (*ptr).transport.clone() })
}
