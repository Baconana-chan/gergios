//! # minix-rs — MINIX IPC and syscall FFI bindings for Rust
//!
//! This crate provides low-level FFI bindings to the MINIX microkernel's IPC
//! mechanism, including IPC message validation. It is `no_std` compatible and
//! designed to be used by both kernel-adjacent and userland Rust components.
//!
//! ## Architecture
//!
//! - **Real MINIX builds** (`#[cfg(target_os = "minix")]`): link to the actual
//!   `_syscall()` function from `libc` / `libminc`.
//! - **Host development** (everything else): provide stub implementations that
//!   compile and allow type-checking, but return `ENOSYS` at runtime.
//!
//! ## Usage
//!
//! ```
//! use minix_rs::*;
//!
//! let mut msg = Message::default();
//! msg.set_type(PM_EXIT);
//! assert!(msg.is_pm_call());
//! assert!(!msg.is_vfs_call());
//! ```

#![no_std]

// ---------------------------------------------------------------------------
// Endpoint type
// ---------------------------------------------------------------------------

/// Microkernel process identifier.
pub type Endpoint = i32;

// ---------------------------------------------------------------------------
// Special endpoint constants  (from minix/com.h)
// ---------------------------------------------------------------------------

pub const ASYNCM:     Endpoint = -5;
pub const IDLE:       Endpoint = -4;
pub const CLOCK:      Endpoint = -3;
pub const SYSTEM:     Endpoint = -2;
pub const KERNEL:     Endpoint = -1;
pub const HARDWARE:   Endpoint = KERNEL;

pub const PM_PROC_NR:  Endpoint = 0;
pub const VFS_PROC_NR: Endpoint = 1;
pub const RS_PROC_NR:  Endpoint = 2;
pub const MEM_PROC_NR: Endpoint = 3;
pub const SCHED_PROC_NR: Endpoint = 4;
pub const TTY_PROC_NR: Endpoint = 5;
pub const DS_PROC_NR:  Endpoint = 6;
pub const MIB_PROC_NR: Endpoint = 7;
pub const VM_PROC_NR:  Endpoint = 8;
pub const PFS_PROC_NR: Endpoint = 9;
pub const MFS_PROC_NR: Endpoint = 10;
pub const INIT_PROC_NR: Endpoint = 11;

/// _ENDPOINT_GENERATION_SHIFT = 15 => _ENDPOINT_GENERATION_SIZE = 32768
/// MAX_NR_TASKS = 1023 => _ENDPOINT_SLOT_TOP = 31745
const ENDPOINT_SLOT_TOP: Endpoint = 31745;

/// Special endpoint: any process.
pub const ANY:  Endpoint = ENDPOINT_SLOT_TOP - 1; // 31744
/// Special endpoint: no process.
pub const NONE: Endpoint = ENDPOINT_SLOT_TOP - 2; // 31743
/// Special endpoint: self (own process).
pub const SELF: Endpoint = ENDPOINT_SLOT_TOP - 3; // 31742

// ---------------------------------------------------------------------------
// Message type
// ---------------------------------------------------------------------------

/// Size of a MINIX IPC message in bytes (guaranteed by the kernel ABI).
pub const MESSAGE_SIZE: usize = 64;

/// The core IPC message type. Exactly 64 bytes, `repr(C)` and `align(16)`.
///
/// On MINIX, the kernel copies exactly 64 bytes between sender and receiver.
/// The `m_source` field is set by the kernel on receive to identify the
/// sender. The `m_type` field is the message type / request number.
#[repr(C, align(16))]
pub struct Message {
    /// Who sent the message (filled by kernel on receive).
    pub m_source: Endpoint,
    /// What kind of message this is (request/response type).
    pub m_type: i32,
    /// Raw payload bytes (56 bytes — message size minus header).
    pub payload: [u8; 56],
}

impl Message {
    /// Create a new, zeroed-out message.
    pub const fn new() -> Self {
        Self {
            m_source: 0,
            m_type: 0,
            payload: [0u8; 56],
        }
    }

    /// Set the message type.
    pub fn set_type(&mut self, msg_type: i32) {
        self.m_type = msg_type;
    }

    /// Get the message type.
    pub fn msg_type(&self) -> i32 {
        self.m_type
    }

    /// Get the source endpoint (sender).
    pub fn source(&self) -> Endpoint {
        self.m_source
    }

    /// Read an `i32` at the given byte offset in the payload.
    pub fn read_i32(&self, offset: usize) -> i32 {
        let mut bytes = [0u8; 4];
        bytes.copy_from_slice(&self.payload[offset..offset + 4]);
        i32::from_ne_bytes(bytes)
    }

    /// Write an `i32` at the given byte offset in the payload.
    pub fn write_i32(&mut self, offset: usize, val: i32) {
        let bytes = val.to_ne_bytes();
        self.payload[offset..offset + 4].copy_from_slice(&bytes);
    }

    /// Read a `u64` at the given byte offset in the payload.
    pub fn read_u64(&self, offset: usize) -> u64 {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.payload[offset..offset + 8]);
        u64::from_ne_bytes(bytes)
    }

    /// Write a `u64` at the given byte offset in the payload.
    pub fn write_u64(&mut self, offset: usize, val: u64) {
        let bytes = val.to_ne_bytes();
        self.payload[offset..offset + 8].copy_from_slice(&bytes);
    }

    /// Read a pointer/usize at the given byte offset.
    pub fn read_ptr(&self, offset: usize) -> usize {
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&self.payload[offset..offset + 8]);
        usize::from_ne_bytes(bytes)
    }

    /// Write a pointer/usize at the given byte offset.
    pub fn write_ptr(&mut self, offset: usize, val: usize) {
        let bytes = val.to_ne_bytes();
        self.payload[offset..offset + 8].copy_from_slice(&bytes);
    }
}

impl Message {
    /// Check whether the payload offset + size is within bounds.
    /// Returns `None` if out of bounds, `Some(())` if valid.
    pub fn check_offset(&self, offset: usize, size: usize) -> Option<()> {
        if offset + size <= self.payload.len() {
            Some(())
        } else {
            None
        }
    }

    /// Returns `true` if this message type is a PM (Process Manager) call.
    pub fn is_pm_call(&self) -> bool {
        (self.m_type & !0xff) == PM_BASE
    }

    /// Returns `true` if this message type is a VFS call.
    pub fn is_vfs_call(&self) -> bool {
        (self.m_type & !0xff) == VFS_BASE
    }

    /// Returns `true` if this message type is a kernel call.
    pub fn is_kernel_call(&self) -> bool {
        (self.m_type & !0xff) == KERNEL_CALL
    }

    /// Returns `true` if this is a notify message.
    pub fn is_notify(&self) -> bool {
        self.m_type == NOTIFY_MESSAGE
    }

    /// Returns `true` if this message has a meaningful source endpoint.
    ///
    /// Returns `false` when `m_source` is 0 (before receive) or one of
    /// the sentinel values (`NONE`, `SELF`).
    ///
    /// Note: PM_PROC_NR (endpoint 0) is indistinguishable from the
    /// "not yet received" state. This is a known limitation.
    pub fn has_source(&self) -> bool {
        self.m_source != 0 && self.m_source != NONE && self.m_source != SELF
    }

    /// Returns `true` if the message type is a PM reply (bit 7 set in call byte).
    pub fn is_pm_reply(&self) -> bool {
        // MINIX convention: PM replies have 0x80 set: PM_BASE + 0x80 + call_nr
        self.is_pm_call() && (self.m_type & 0x80) != 0
    }

    /// Returns `true` if the message type is a VFS reply (bit 7 set in call byte).
    pub fn is_vfs_reply(&self) -> bool {
        // MINIX convention: VFS replies have 0x80 set: VFS_BASE + 0x80 + call_nr
        self.is_vfs_call() && (self.m_type & 0x80) != 0
    }

    /// Check if a source endpoint is a valid MINIX endpoint number.
    /// Valid ranges: tasks (<0), servers (0..NR_PROCS), any/none/self.
    pub fn is_valid_source(&self) -> bool {
        const NR_PROCS: i32 = 256;
        (self.m_source >= 0 && self.m_source < NR_PROCS) ||
        (self.m_source < 0 && self.m_source >= -24)
    }
}

impl Default for Message {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Free-standing validation helpers
// ---------------------------------------------------------------------------

/// Check whether the given message type is a PM (Process Manager) call.
pub fn is_pm_call(msg_type: i32) -> bool {
    (msg_type & !0xff) == PM_BASE
}

/// Check whether the given message type is a VFS call.
pub fn is_vfs_call(msg_type: i32) -> bool {
    (msg_type & !0xff) == VFS_BASE
}

/// Check whether the given message type is a kernel call.
pub fn is_kernel_call(msg_type: i32) -> bool {
    (msg_type & !0xff) == KERNEL_CALL
}

/// Check whether the given message type is a notify message.
pub fn is_notify(msg_type: i32) -> bool {
    msg_type == NOTIFY_MESSAGE
}

/// Check if an endpoint value is valid (not special sentinel).
pub fn is_valid_endpoint(ep: Endpoint) -> bool {
    // Special endpoints (ANY, NONE, SELF) are valid for send operations
    ep != 0
}

/// Convert a PM call number to a base call index.
pub fn pm_call_index(msg_type: i32) -> Option<usize> {
    if is_pm_call(msg_type) {
        Some((msg_type & 0xff) as usize)
    } else {
        None
    }
}

/// Convert a VFS call number to a base call index.
pub fn vfs_call_index(msg_type: i32) -> Option<usize> {
    if is_vfs_call(msg_type) {
        Some((msg_type & 0xff) as usize)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Syscall numbers  (from minix/callnr.h)
// ---------------------------------------------------------------------------

// -- PM calls --
pub const PM_BASE: i32 = 0x000;
pub const PM_EXIT: i32       = PM_BASE + 1;
pub const PM_FORK: i32       = PM_BASE + 2;
pub const PM_WAIT4: i32      = PM_BASE + 3;
pub const PM_GETPID: i32     = PM_BASE + 4;
pub const PM_SETUID: i32     = PM_BASE + 5;
pub const PM_GETUID: i32     = PM_BASE + 6;
pub const PM_STIME: i32      = PM_BASE + 7;
pub const PM_PTRACE: i32     = PM_BASE + 8;
pub const PM_SETGROUPS: i32  = PM_BASE + 9;
pub const PM_GETGROUPS: i32  = PM_BASE + 10;
pub const PM_KILL: i32       = PM_BASE + 11;
pub const PM_SETGID: i32     = PM_BASE + 12;
pub const PM_GETGID: i32     = PM_BASE + 13;
pub const PM_EXEC: i32       = PM_BASE + 14;
pub const PM_SETSID: i32     = PM_BASE + 15;
pub const PM_GETPGRP: i32    = PM_BASE + 16;
pub const PM_ITIMER: i32     = PM_BASE + 17;
pub const PM_SIGACTION: i32  = PM_BASE + 20;
pub const PM_SIGSUSPEND: i32 = PM_BASE + 21;
pub const PM_SIGRETURN: i32  = PM_BASE + 24;
pub const PM_REBOOT: i32     = PM_BASE + 37;
pub const PM_GETEPINFO: i32  = PM_BASE + 45;
pub const PM_GETPROCNR: i32  = PM_BASE + 46;

// -- VFS calls --
pub const VFS_BASE: i32 = 0x100;
pub const VFS_READ: i32    = VFS_BASE + 0;
pub const VFS_WRITE: i32   = VFS_BASE + 1;
pub const VFS_LSEEK: i32   = VFS_BASE + 2;
pub const VFS_OPEN: i32    = VFS_BASE + 3;
pub const VFS_CREAT: i32   = VFS_BASE + 4;
pub const VFS_CLOSE: i32   = VFS_BASE + 5;
pub const VFS_LINK: i32    = VFS_BASE + 6;
pub const VFS_UNLINK: i32  = VFS_BASE + 7;
pub const VFS_CHDIR: i32   = VFS_BASE + 8;
pub const VFS_CHMOD: i32   = VFS_BASE + 11;
pub const VFS_CHOWN: i32   = VFS_BASE + 12;
pub const VFS_MOUNT: i32   = VFS_BASE + 13;
pub const VFS_UMOUNT: i32  = VFS_BASE + 14;
pub const VFS_STAT: i32    = VFS_BASE + 21;
pub const VFS_FSTAT: i32   = VFS_BASE + 22;
pub const VFS_LSTAT: i32   = VFS_BASE + 23;
pub const VFS_IOCTL: i32   = VFS_BASE + 24;
pub const VFS_FCNTL: i32   = VFS_BASE + 25;
pub const VFS_PIPE2: i32   = VFS_BASE + 26;
pub const VFS_UMASK: i32   = VFS_BASE + 27;

// -- Kernel calls  (from minix/com.h) --
pub const KERNEL_CALL: i32 = 0x600;
pub const SYS_FORK: i32       = KERNEL_CALL + 0;
pub const SYS_EXEC: i32       = KERNEL_CALL + 1;
pub const SYS_CLEAR: i32      = KERNEL_CALL + 2;
pub const SYS_SCHEDULE: i32   = KERNEL_CALL + 3;
pub const SYS_PRIVCTL: i32    = KERNEL_CALL + 4;
pub const SYS_TRACE: i32      = KERNEL_CALL + 5;
pub const SYS_KILL: i32       = KERNEL_CALL + 6;
pub const SYS_IRQCTL: i32     = KERNEL_CALL + 19;
pub const SYS_DEVIO: i32      = KERNEL_CALL + 21;
pub const SYS_SETALARM: i32   = KERNEL_CALL + 24;
pub const SYS_TIMES: i32      = KERNEL_CALL + 25;
pub const SYS_GETINFO: i32    = KERNEL_CALL + 26;
pub const SYS_ABORT: i32      = KERNEL_CALL + 27;
pub const SYS_EXIT: i32       = KERNEL_CALL + 53;

// -- Notify --
pub const NOTIFY_MESSAGE: i32 = 0x1000;

// ---------------------------------------------------------------------------
// _syscall() — send and receive a message
// ---------------------------------------------------------------------------

/// Send a message to `who` with the given syscall number and receive the reply
/// in the same message buffer.
///
/// Returns 0 on success, or a negative errno on error.
///
/// ## MINIX (real)
///
/// Calls the C `_syscall()` function from the MINIX libc.
///
/// ## Host (stub)
///
/// Returns `-ENOSYS` (function not implemented) — the stub exists only to allow
/// compilation and type-checking during host-side development.
#[cfg(target_os = "minix")]
pub fn syscall(who: Endpoint, nr: i32, msg: &mut Message) -> i32 {
    extern "C" {
        fn _syscall(who: Endpoint, nr: i32, msg: *mut Message) -> i32;
    }
    unsafe { _syscall(who, nr, msg as *mut Message) }
}

#[cfg(not(target_os = "minix"))]
pub fn syscall(_who: Endpoint, _nr: i32, _msg: &mut Message) -> i32 {
    // ENOSYS: function not implemented on host platform
    -78
}

// ---------------------------------------------------------------------------
// IPC convenience wrappers
// ---------------------------------------------------------------------------

/// Send a synchronous IPC message and receive a reply.
/// Convenience wrapper that sets `m_type` before calling `syscall`.
pub fn sendrec(who: Endpoint, nr: i32, msg: &mut Message) -> i32 {
    msg.set_type(nr);
    syscall(who, nr, msg)
}

/// Send a message to PM (Process Manager) and receive a reply.
pub fn pm_syscall(nr: i32, msg: &mut Message) -> i32 {
    sendrec(PM_PROC_NR, nr, msg)
}

/// Send a message to VFS (Virtual File System) and receive a reply.
pub fn vfs_syscall(nr: i32, msg: &mut Message) -> i32 {
    sendrec(VFS_PROC_NR, nr, msg)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_size() {
        assert_eq!(core::mem::size_of::<Message>(), MESSAGE_SIZE);
    }

    #[test]
    fn message_alignment() {
        assert_eq!(core::mem::align_of::<Message>(), 16);
    }

    #[test]
    fn message_default_is_zeroed() {
        let msg = Message::default();
        assert_eq!(msg.m_source, 0);
        assert_eq!(msg.m_type, 0);
        assert_eq!(msg.payload, [0u8; 56]);
    }

    #[test]
    fn message_read_write_i32() {
        let mut msg = Message::new();
        msg.write_i32(0, 42);
        msg.write_i32(4, -123);
        assert_eq!(msg.read_i32(0), 42);
        assert_eq!(msg.read_i32(4), -123);
    }

    #[test]
    fn message_read_write_u64() {
        let mut msg = Message::new();
        msg.write_u64(0, 0xDEAD_BEEF_CAFE_BABE);
        assert_eq!(msg.read_u64(0), 0xDEAD_BEEF_CAFE_BABE);
    }

    #[test]
    fn message_set_type() {
        let mut msg = Message::new();
        msg.set_type(PM_EXIT);
        assert_eq!(msg.msg_type(), PM_EXIT);
        assert_eq!(msg.m_type, PM_EXIT);
    }

    #[test]
    fn endpoint_constants() {
        assert_eq!(PM_PROC_NR, 0);
        assert_eq!(VFS_PROC_NR, 1);
        assert_eq!(SYSTEM, -2);
        assert_eq!(SELF, 31742);
        assert_eq!(ANY, 31744);
        assert_eq!(NONE, 31743);
    }

    #[test]
    fn syscall_stub_on_host() {
        let mut msg = Message::new();
        let result = syscall(PM_PROC_NR, PM_GETPID, &mut msg);
        // On non-MINIX platforms, syscall returns -ENOSYS
        assert_eq!(result, -78);
    }

    #[test]
    fn pm_constants() {
        assert_eq!(PM_EXIT, 1);
        assert_eq!(PM_FORK, 2);
        assert_eq!(PM_GETPID, 4);
        assert_eq!(PM_KILL, 11);
    }

    #[test]
    fn vfs_constants() {
        assert_eq!(VFS_READ, 0x100);
        assert_eq!(VFS_WRITE, 0x101);
        assert_eq!(VFS_OPEN, 0x103);
    }

    #[test]
    fn sys_constants() {
        assert_eq!(SYS_EXIT, 0x600 + 53);
        assert_eq!(NOTIFY_MESSAGE, 0x1000);
    }

    // -- IPC validation tests --

    #[test]
    fn is_pm_call_detection() {
        let mut msg = Message::new();
        msg.set_type(PM_EXIT);
        assert!(msg.is_pm_call());
        assert!(!msg.is_vfs_call());
        assert!(!msg.is_kernel_call());
        assert!(!msg.is_notify());
    }

    #[test]
    fn is_vfs_call_detection() {
        let mut msg = Message::new();
        msg.set_type(VFS_OPEN);
        assert!(!msg.is_pm_call());
        assert!(msg.is_vfs_call());
        assert!(!msg.is_kernel_call());
    }

    #[test]
    fn is_kernel_call_detection() {
        let mut msg = Message::new();
        msg.set_type(SYS_FORK);
        assert!(msg.is_kernel_call());
        assert!(!msg.is_pm_call());
        assert!(!msg.is_vfs_call());
    }

    #[test]
    fn is_notify_detection() {
        let mut msg = Message::new();
        msg.set_type(NOTIFY_MESSAGE);
        assert!(msg.is_notify());
        assert!(!msg.is_pm_call());
        assert!(!msg.is_vfs_call());
    }

    #[test]
    fn check_offset_valid() {
        let msg = Message::new();
        assert!(msg.check_offset(0, 56).is_some());
        assert!(msg.check_offset(52, 4).is_some());
        assert!(msg.check_offset(0, 0).is_some());
    }

    #[test]
    fn check_offset_invalid() {
        let msg = Message::new();
        assert!(msg.check_offset(0, 57).is_none());
        assert!(msg.check_offset(56, 1).is_none());
        assert!(msg.check_offset(100, 4).is_none());
    }

    #[test]
    fn has_source() {
        let mut msg = Message::new();
        assert!(!msg.has_source()); // after new() — no source
        msg.m_source = VFS_PROC_NR; // VFS_PROC_NR (1) is valid
        assert!(msg.has_source());
        msg.m_source = NONE;         // NONE is not a valid source
        assert!(!msg.has_source());
        msg.m_source = SELF;         // SELF is not a valid source (sentinel)
        assert!(!msg.has_source());
    }

    #[test]
    fn free_standing_validators() {
        assert!(is_pm_call(PM_GETPID));
        assert!(!is_pm_call(VFS_OPEN));
        assert!(is_vfs_call(VFS_READ));
        assert!(!is_vfs_call(PM_EXIT));
        assert!(is_kernel_call(SYS_FORK));
        assert!(!is_kernel_call(PM_EXIT));
        assert!(is_notify(NOTIFY_MESSAGE));
        assert!(!is_notify(PM_EXIT));
    }

    #[test]
    fn call_index_functions() {
        assert_eq!(pm_call_index(PM_EXIT), Some(1));
        assert_eq!(pm_call_index(PM_FORK), Some(2));
        assert_eq!(pm_call_index(VFS_OPEN), None);
        assert_eq!(vfs_call_index(VFS_OPEN), Some(3));
        assert_eq!(vfs_call_index(VFS_READ), Some(0));
        assert_eq!(vfs_call_index(PM_EXIT), None);
    }

    #[test]
    fn pm_reply_detection() {
        // MINIX convention: PM replies have bit 7 (0x80) set
        // Reply to PM_EXIT = PM_BASE + 0x80 + 1 = 0x81 = 129
        let mut msg = Message::new();
        msg.set_type(PM_BASE + 0x80 + 1); // reply to PM_EXIT
        assert!(msg.is_pm_reply(), "PM_BASE + 0x81 should be a reply");
        msg.set_type(PM_BASE + 2); // PM_FORK request (no bit 7)
        assert!(!msg.is_pm_reply(), "PM_BASE + 2 should be a request");
        // VFS call should not be detected as PM reply
        msg.set_type(VFS_BASE + 0x80 + 0); // VFS reply
        assert!(!msg.is_pm_reply(), "VFS reply should not be PM reply");
    }
}
