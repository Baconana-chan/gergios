//! # MINIX IPC Daemon Framework
//!
//! Provides the SEF lifecycle, IPC message loop, and message dispatch
//! infrastructure needed by a MINIX system service daemon.
//!
//! ## Architecture
//!
//! ```text
//! main()
//!  ├── sef_setcb_init_fresh()    // register init callback
//!  ├── sef_setcb_signal_handler()// register signal handler
//!  ├── sef_startup()             // let RS know we're ready
//!  └── daemon_loop()             // receive & dispatch messages
//!       ├── sef_receive(ANY)
//!       ├── IS_SEF_INIT → handle re-init
//!       ├── IS_SIGNAL  → handle signal
//!       ├── IS_FI      → fault injection
//!       ├── IS_BT_CMD  → dispatch to BtDaemonCmd handler
//!       └── send(reply)
//! ```
//!
//! ## Message types
//!
//! BT daemon commands use the `BT_RQ_BASE` (0x1D00) range defined in
//! `minix/com.h`. Each command is a `u32` message type. Replies use the
//! standard MINIX convention: `m_type = OK` (0) or a negative errno.
//!
//! ## Payload encoding
//!
//! Arguments are packed into the `mess_4` layout (four `long` fields):
//!
//! | Field   | Offset | Description                     |
//! |---------|--------|---------------------------------|
//! | m4_l1   | 0      | Command-specific arg / count    |
//! | m4_l2   | 8      | BD_ADDR low 32 bits or grant ID |
//! | m4_l3   | 16     | BD_ADDR high 16 bits or handle  |
//! | m4_l4   | 24     | Connection handle / flags / PSM |
//! | m4_ll1  | 32     | 64-bit extension / capabilities |
//!
//! For variable-length data (device list, connection list), the daemon
//! copies data via a grant that the client has set up.

#![allow(dead_code)]

use minix_rs::{self, Endpoint, Message, SefInitInfo};

// ============================================================================
// SEF Event Detection Macros
// ============================================================================

/// True if the message is an RS init request (fresh, restart, or live update).
pub fn is_sef_init(msg: &Message) -> bool {
    msg.m_type == RS_INIT && msg.m_source == RS_PROC_NR
}

/// True if the message is a signal from the system signal manager.
pub fn is_signal(msg: &Message, _status: i32) -> bool {
    msg.m_type == SIGS_SIGNAL_RECEIVED
}

/// True if the message is a fault injection control request.
pub fn is_fault_injection(msg: &Message) -> bool {
    msg.m_type == COMMON_REQ_FI_CTL
}

/// True if the message is a BT daemon command.
pub fn is_bt_cmd(msg: &Message) -> bool {
    let base = msg.m_type & !0x3f;
    base == BT_RQ_BASE && msg.m_type >= BT_RQ_BASE && msg.m_type <= BT_RQ_BASE + 15
}

/// True if the message is a notify from the kernel.
pub fn is_notify(msg: &Message) -> bool {
    msg.m_type == NOTIFY_MESSAGE
}

/// True if the BT daemon should exit (SIGTERM or kernel shutdown).
pub fn is_shutdown(_msg: &Message, signo: i32) -> bool {
    signo == SIGTERM || signo == SIGHUP
}

// ============================================================================
// SEF Constants (from minix/sef.h and minix/com.h)
// ============================================================================

// RS init types
pub const RS_INIT: i32 = 0x720; // RS_RQ_BASE + 20
pub const RS_PROC_NR: Endpoint = 2;

// Signal types
pub const SIGS_SIGNAL_RECEIVED: i32 = 0xE00; // COMMON_RQ_BASE + 0
pub const COMMON_REQ_FI_CTL: i32 = 0xE02;    // COMMON_RQ_BASE + 2

// Signals
pub const SIGTERM: i32 = 15;
pub const SIGHUP: i32 = 1;
pub const SIGALRM: i32 = 14;

// SEF init types
pub const SEF_INIT_FRESH: i32 = 0;
pub const SEF_INIT_RESTART: i32 = 2;

// OK / errno
pub const OK: i32 = 0;
pub const EINVAL: i32 = 22;   // positive for m_type = -EINVAL
pub const ENOSYS: i32 = 78;

// BT daemon message range (from minix/com.h)
pub const BT_RQ_BASE: i32 = 0x1D00;

// Notify message type (from minix/com.h)
pub const NOTIFY_MESSAGE: i32 = 0x1000;

// Special endpoint: no process (from minix-rs)
pub const NONE: Endpoint = 31743;

// ============================================================================
// Daemon Framework Callbacks
// ============================================================================

/// User-supplied callbacks for the daemon framework.
pub struct DaemonCallbacks {
    /// Called on fresh init / restart init. Return OK to proceed.
    pub init_fresh: Option<fn() -> i32>,
    /// Called on SIGTERM / SIGHUP. Should clean up and return.
    pub signal_term: Option<fn(i32)>,
    /// Called on SIGALRM (periodic timer). Return 0 to re-arm, non-zero to stop.
    pub signal_alarm: Option<fn() -> i32>,
    /// Dispatch a BT daemon command message. Returns reply as i32 (OK or errno).
    pub dispatch_cmd: Option<fn(&mut Message) -> i32>,
    /// Called each loop iteration before sef_receive (for polling HCI, etc.).
    pub poll_hook: Option<fn() -> usize>,
}

/// Set the global daemon pointer. Called by `BtDaemon::run_minix()`.
///
/// # Safety
/// Must be called once, before entering the daemon loop. The pointer
/// must remain valid for the lifetime of the program.
pub unsafe fn set_global_daemon_ptr(ptr: *mut core::ffi::c_void) {
    unsafe {
        GLOBAL_DAEMON_PTR = ptr;
    }
}

/// Get the global daemon pointer. Returns null if not set.
///
/// # Safety
/// The returned pointer is only safe to dereference if the pointer
/// was previously set by `set_global_daemon_ptr` and the daemon
/// instance remains alive.
pub unsafe fn get_global_daemon_ptr() -> *mut core::ffi::c_void {
    unsafe { GLOBAL_DAEMON_PTR }
}

// ============================================================================
// SEF Native Callbacks (C-compatible, registered with SEF library)
// ============================================================================

/// Global pointer to the daemon callbacks, set by `run_daemon_loop`.
/// This is needed because SEF callbacks have a fixed C signature and
/// cannot capture closures.
///
/// Uses a raw pointer instead of `Option<&T>` to avoid `static_mut_refs`
/// errors in Rust 2024 edition.
static mut GLOBAL_CALLBACKS: *const DaemonCallbacks = core::ptr::null();

/// Global pointer to the BtDaemon instance, set by `run_minix()` before
/// entering the daemon loop. This allows `dispatch_ipc_message()` to
/// call methods on the daemon instance even though the SEF callback
/// interface doesn't support closure capture.
///
/// Uses `*mut core::ffi::c_void` to avoid circular dependency with
/// `bt_daemon.rs` (which imports this module). The `BtDaemon::run_minix()`
/// method sets this to `self as *mut _` before entering the loop.
///
/// # Safety
/// The pointer is set once before entering the loop and the daemon
/// instance lives on the caller's stack (which never returns on MINIX).
static mut GLOBAL_DAEMON_PTR: *mut core::ffi::c_void = core::ptr::null_mut();

/// Access the global daemon pointer. Returns a raw pointer that can be
/// cast back to `&mut BtDaemon` by the dispatch handler.
///
/// # Safety
/// The caller must ensure:
/// - The pointer was set by `BtDaemon::run_minix()` before this call
/// - The daemon instance remains alive (it does — the loop never returns)
/// - This is called from a single-threaded context (MINIX IPC loop)

/// C-compatible SEF init callback.
///
/// # Safety
/// Called by the MINIX SEF framework. Accesses the global callback table
/// via raw pointer to avoid static_mut_refs issues in Rust 2024.
pub unsafe extern "C" fn sef_cb_init_fresh_impl(_type: i32, _info: *mut SefInitInfo) -> i32 {
    // SAFETY: Single-threaded context, only called from SEF during init.
    // The pointer is set once before entering the IPC loop and never changes.
    let cbs_ptr = unsafe { GLOBAL_CALLBACKS };
    if !cbs_ptr.is_null() {
        let callbacks = unsafe { &*cbs_ptr };
        if let Some(init) = callbacks.init_fresh {
            return init();
        }
    }
    OK
}

/// C-compatible SEF signal handler.
///
/// # Safety
/// Called by the MINIX SEF framework when a signal is received.
/// Accesses the global callback table via raw pointer.
pub unsafe extern "C" fn sef_cb_signal_handler_impl(signo: i32) {
    // SAFETY: Single-threaded context, only called from SEF on signal delivery.
    let cbs_ptr = unsafe { GLOBAL_CALLBACKS };
    if !cbs_ptr.is_null() {
        let callbacks = unsafe { &*cbs_ptr };
        match signo {
            SIGTERM | SIGHUP => {
                if let Some(term) = callbacks.signal_term {
                    term(signo);
                }
            }
            SIGALRM => {
                // Call the alarm callback and re-arm the timer if it returns OK.
                // The alarm callback (signal_alarm) already polls HCI events,
                // so we don't need another poll_hook call here.
                let rearm = callbacks.signal_alarm
                    .map(|f| f())
                    .unwrap_or(OK);
                if rearm == OK {
                    // Re-arm at 5 ticks (50ms at HZ=100).
                    // This gives a stable polling interval independent of
                    // how many events were processed.
                    minix_rs::sys_setalarm(5, 0);
                }
            }
            _ => {}
        }
    }
}

// ============================================================================
// Main Loop
// ============================================================================

/// Run the MINIX daemon main loop.
///
/// 1. Registers SEF callbacks
/// 2. Calls `sef_startup()` to announce to RS
/// 3. Loops: `sef_receive(ANY)` → dispatch → send reply
///
/// This function never returns on MINIX (the daemon runs until killed).
pub fn run_daemon_loop(callbacks: DaemonCallbacks) -> ! {
    // Store callbacks in the global for the C callbacks to access
    // Use a raw pointer to avoid static_mut_refs issues in Rust 2024.
    // Safety: the callbacks struct lives on the stack of this function
    // which never returns (loop forever), so the pointer remains valid.
    unsafe {
        GLOBAL_CALLBACKS = &callbacks as *const DaemonCallbacks;
    }

    // Register SEF callbacks
    minix_rs::sef_setcb_init_fresh(sef_cb_init_fresh_impl);
    minix_rs::sef_setcb_signal_handler(sef_cb_signal_handler_impl);

    // Announce to RS — this is where RS sends RS_INIT to us
    minix_rs::sef_startup();

    // Set initial alarm timer for periodic HCI polling.
    // If there's a poll_hook, arm the timer at 5 ticks (50ms at HZ=100).
    // If not, use a default interval of 5 ticks anyway.
    // The SIGALRM handler will re-arm on each fire.
    unsafe {
        let cbs_ptr = GLOBAL_CALLBACKS;
        if !cbs_ptr.is_null() {
            let cbs = &*cbs_ptr;
            if cbs.signal_alarm.is_some() || cbs.poll_hook.is_some() {
                minix_rs::sys_setalarm(5, 0); // 50ms delay, non-absolute
            }
        }
    }

    // Main IPC loop
    //
    // Note: SEF processes init (RS_INIT), signal (SIGS_SIGNAL_RECEIVED),
    // and ping (NOTIFY_MESSAGE from RS) messages internally via the
    // registered callbacks. The loop only needs to handle BT commands
    // and unknown messages. The poll_hook is called before each receive
    // to process any pending HCI events.
    loop {
        // Call poll hook before blocking on sef_receive.
        // This handles any events that arrived since the last poll.
        unsafe {
            let cbs_ptr = GLOBAL_CALLBACKS;
            if !cbs_ptr.is_null() {
                let cbs = &*cbs_ptr;
                if let Some(poll) = cbs.poll_hook {
                    poll();
                }
            }
        }

        let mut msg = Message::new();
        let r = minix_rs::sef_receive(minix_rs::ANY, &mut msg);

        if r != OK {
            continue;
        }

        let source = msg.m_source;

        // SEF handles: RS_INIT, signals, RS ping internally.
        // We only dispatch BT commands and unknown messages.

        // Check for BT daemon command
        if is_bt_cmd(&msg) {
            // SAFETY: Single-threaded, pointer is set once before loop.
            let cbs_ptr = unsafe { GLOBAL_CALLBACKS };
            let result = if !cbs_ptr.is_null() {
                let callbacks = unsafe { &*cbs_ptr };
                callbacks.dispatch_cmd
                    .map(|f| f(&mut msg))
                    .unwrap_or(-(ENOSYS as i32))
            } else {
                -(ENOSYS as i32)
            };

            // Set reply type: OK or errno (convention: 0 = OK, negative = errno)
            msg.m_type = result;

            // Send reply back to the caller (unless it's NONE = notification)
            if source != NONE {
                let _ = minix_rs::ipc_send(source, &mut msg);
            }
            continue;
        }

        // Unknown or unhandled message — send EINVAL if it has a real source
        if source != NONE && source > 0 {
            msg.m_type = -(EINVAL as i32);
            let _ = minix_rs::ipc_send(source, &mut msg);
        }
    }
}

// ============================================================================
// Message Payload Helpers (for BT daemon commands)
// ============================================================================

/// Read a BD_ADDR from a message payload.
/// BD_ADDR is packed as: m4_l2 = low 32 bits, m4_l3 bits [0..15] = high 16 bits.
pub fn msg_read_bdaddr_low(msg: &Message) -> u32 {
    msg.read_i32(8) as u32 // m4_l2 offset
}

pub fn msg_read_bdaddr_high(msg: &Message) -> u16 {
    (msg.read_i32(16) & 0xFFFF) as u16 // m4_l3 offset, low 16 bits
}

/// Write a BD_ADDR into a message payload.
pub fn msg_write_bdaddr(msg: &mut Message, low: u32, high: u16) {
    msg.write_i32(8, low as i32);
    msg.write_i32(16, high as i32);
}

/// Read a connection handle from a message (m4_l3, bits 16..31).
pub fn msg_read_handle(msg: &Message) -> u16 {
    ((msg.read_i32(16) >> 16) & 0xFFFF) as u16
}

/// Write a connection handle into a message.
pub fn msg_write_handle(msg: &mut Message, handle: u16) {
    let existing = msg.read_i32(16);
    let new_val = (existing & 0x0000FFFF) | ((handle as i32) << 16);
    msg.write_i32(16, new_val);
}

/// Read a name string from a message payload.
///
/// Name starts at offset 32 in the 56-byte payload, so max
/// readable length is 24 bytes (56 - 32). The search stops at
/// the first null byte or the payload boundary.
pub fn msg_read_name<'a>(msg: &'a Message) -> &'a [u8] {
    let max_bytes = msg.payload.len().saturating_sub(32); // 24
    let mut len = max_bytes;
    for i in 0..max_bytes {
        if msg.payload[32 + i] == 0 {
            len = i;
            break;
        }
    }
    &msg.payload[32..32 + len]
}

/// Write a name string into a message payload.
///
/// Name starts at offset 32 in the 56-byte payload. Max writable
/// length is 23 bytes (24 available - 1 for null terminator).
pub fn msg_write_name(msg: &mut Message, name: &[u8]) {
    let max_bytes = msg.payload.len().saturating_sub(33); // 56 - 32 - 1 = 23
    let len = name.len().min(max_bytes);
    msg.payload[32..32 + len].copy_from_slice(&name[..len]);
    msg.payload[32 + len] = 0; // null-terminate
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_bt_cmd() {
        let mut msg = Message::new();
        msg.set_type(BT_RQ_BASE + 0);
        assert!(is_bt_cmd(&msg));

        msg.set_type(BT_RQ_BASE + 12);
        assert!(is_bt_cmd(&msg));

        msg.set_type(BT_RQ_BASE - 1);
        assert!(!is_bt_cmd(&msg));

        msg.set_type(0x1C00); // auditd range
        assert!(!is_bt_cmd(&msg));
    }

    #[test]
    fn test_is_sef_init() {
        let mut msg = Message::new();
        msg.set_type(RS_INIT);
        msg.m_source = RS_PROC_NR;
        assert!(is_sef_init(&msg));

        msg.m_source = 42;
        assert!(!is_sef_init(&msg));
    }

    #[test]
    fn test_is_signal() {
        let mut msg = Message::new();
        msg.set_type(SIGS_SIGNAL_RECEIVED);
        assert!(is_signal(&msg, 0));
    }

    #[test]
    fn test_msg_write_read_bdaddr() {
        let mut msg = Message::new();
        msg_write_bdaddr(&mut msg, 0xAABBCCDD, 0xEEFF);

        assert_eq!(msg_read_bdaddr_low(&msg), 0xAABBCCDD);
        assert_eq!(msg_read_bdaddr_high(&msg), 0xEEFF);
    }

    #[test]
    fn test_msg_write_read_handle() {
        let mut msg = Message::new();
        msg_write_handle(&mut msg, 0x0042);
        assert_eq!(msg_read_handle(&msg), 0x0042);
    }

    #[test]
    fn test_msg_write_read_name() {
        let mut msg = Message::new();
        msg_write_name(&mut msg, b"TestDevice");
        assert_eq!(msg_read_name(&msg), b"TestDevice");
    }

    #[test]
    fn test_msg_name_truncation() {
        let mut msg = Message::new();
        let long_name = [b'A'; 100];
        msg_write_name(&mut msg, &long_name);
        let readback = msg_read_name(&msg);
        // Max name = 23 bytes (56 - 32 - 1 for null)
        assert!(readback.len() <= 23);
        assert_eq!(readback.len(), 23);
        assert_eq!(readback, &[b'A'; 23]);
    }
}
