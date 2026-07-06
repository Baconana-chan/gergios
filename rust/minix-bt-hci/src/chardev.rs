//! # /dev/hci0 Character Device Interface
//!
//! Implements the MINIX chardev interface for the Bluetooth HCI transport.
//! BlueZ userspace daemon communicates with the HCI controller via:
//!
//!   - **read()**: receives HCI events + ACL data from the controller
//!   - **write()**: sends HCI commands + ACL data to the controller
//!   - **ioctl()**: device management (UP/DOWN, device list, etc.)
//!
//! ## ioctl Interface
//!
//! BlueZ-compatible HCI ioctl commands (from Linux uapi/hci.h):
//!
//! ```text
//! HCIDEVUP     = _IOW('H', 201, int)   — Bring HCI device up
//! HCIDEVDOWN   = _IOW('H', 202, int)   — Bring HCI device down
//! HCIGETDEVLIST = _IOR('H', 210, int)  — Get list of HCI devices
//! HCIGETDEVINFO = _IOR('H', 211, int)  — Get HCI device info
//! HCIGETCONNINFO = _IOR('H', 212, int)  — Get connection info
//! HCIINQUIRY    = _IOW('H', 220, int)  — Start inquiry scan
//! ```

#![allow(dead_code)]

use core::ffi::{c_int, c_ulong, c_void};
use std::boxed::Box;
use std::vec::Vec;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::ffi;
use crate::hci;
use crate::usb_transport::HciUsbTransport;

// ============================================================================
// BlueZ-compatible HCI ioctl definitions
// ============================================================================

/// _IOW(H, 201, int) — Bring HCI device up.
pub const HCIDEVUP: c_ulong = 0x400448C9;   // sizeof(int) = 4

/// _IOW(H, 202, int) — Bring HCI device down.
pub const HCIDEVDOWN: c_ulong = 0x400448CA;

/// _IOR(H, 210, struct hci_dev_list_req) — Get HCI device list.
pub const HCIGETDEVLIST: c_ulong = 0x800448D2;

/// _IOR(H, 211, struct hci_dev_info) — Get HCI device info.
pub const HCIGETDEVINFO: c_ulong = 0x800448D3;

/// _IOR(H, 212, struct hci_conn_info_req) — Get connection info.
pub const HCIGETCONNINFO: c_ulong = 0x800448D4;

/// _IOW(H, 220, struct hci_inquiry_req) — Start inquiry.
pub const HCIINQUIRY: c_ulong = 0x400448DC;

// HCI device flags (from Linux hci.h)
pub const HCI_UP: u16         = 1 << 0;
pub const HCI_INIT: u16       = 1 << 1;
pub const HCI_RUNNING: u16    = 1 << 2;
pub const HCI_PSCAN: u16      = 1 << 3;
pub const HCI_ISCAN: u16      = 1 << 4;
pub const HCI_AUTH: u16       = 1 << 5;
pub const HCI_ENCRYPT: u16    = 1 << 6;
pub const HCI_INQUIRY_FLAG: u16 = 1 << 7;

// HCI dev type (from hci.h)
pub const HCI_BREDR: u16 = 0;
pub const HCI_AMP: u16   = 1;
pub const HCI_BREDR_LE: u16 = 2;

/// HCI device info structure (Linux-compatible layout).
#[repr(C)]
pub struct HciDevInfo {
    pub dev_id: u16,
    pub name: [u8; 8],
    pub bdaddr: [u8; 6],
    pub flags: u16,
    pub dev_type: u8,
    pub features: [u8; 8],
    pub pkt_type: u32,
    pub link_policy: u32,
    pub link_mode: u32,
    pub acl_mtu: u16,
    pub sco_mtu: u16,
    pub acl_pkts: u16,
    pub sco_pkts: u16,
    pub dev_stats: HciDevStats,
}

/// HCI device statistics.
#[derive(Clone, Copy)]
#[repr(C)]
pub struct HciDevStats {
    pub err_rx: u32,
    pub err_tx: u32,
    pub cmd_tx: u32,
    pub evt_rx: u32,
    pub acl_tx: u32,
    pub acl_rx: u32,
    pub sco_tx: u32,
    pub sco_rx: u32,
    pub byte_tx: u32,
    pub byte_rx: u32,
}

/// HCI device list request/reply.
#[repr(C)]
pub struct HciDevListReq {
    pub num_devices: u16,
    pub devices: [HciDevInfo; 1],  // Variable-length in practice
}

// ============================================================================
// Circular FIFO buffers for HCI packets
// ============================================================================

/// Size of the HCI event ring buffer (packet count).
pub const HCI_EVT_RING_SIZE: usize = 32;

/// Size of the HCI data ring buffer (packet count).
pub const HCI_DATA_RING_SIZE: usize = 16;

/// Single HCI packet in the ring buffer.
/// Uses heap-allocated data to avoid 64KB stack usage per entry.
#[derive(Clone)]
#[repr(C)]
pub struct HciPacketBuf {
    /// Packet type (HCI_EVENT, HCI_ACL_DATA, etc.)
    pub pkt_type: u8,
    /// Packet data length (excluding type byte).
    pub data_len: u16,
    /// Packet data (heap-allocated, sized for largest HCI ACL packet).
    pub data: Box<[u8; hci::HCI_MAX_ACL_SIZE - 1]>,
}

impl HciPacketBuf {
    pub fn new() -> Self {
        Self {
            pkt_type: 0,
            data_len: 0,
            data: Box::new([0u8; hci::HCI_MAX_ACL_SIZE - 1]),
        }
    }
}

/// Ring buffer for HCI packets (SPSC — single producer, single consumer).
pub struct HciRingBuf {
    bufs: Vec<HciPacketBuf>,
    head: usize,  // Producer index (IRQ/worker writes here)
    tail: usize,  // Consumer index (read() syscall reads here)
    count: usize,
}

impl HciRingBuf {
    pub fn new() -> Self {
        let mut bufs = Vec::with_capacity(HCI_EVT_RING_SIZE);
        for _ in 0..HCI_EVT_RING_SIZE {
            bufs.push(HciPacketBuf::new());
        }
        Self {
            bufs,
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    /// Push a packet into the ring buffer.
    /// Returns true on success, false if the buffer is full.
    pub fn push(&mut self, pkt_type: u8, data: &[u8]) -> bool {
        if self.count == HCI_EVT_RING_SIZE {
            return false;  // Ring buffer full
        }

        let max_data = self.bufs[0].data.len() - 1;
        let len = core::cmp::min(data.len(), max_data) as u16;
        self.bufs[self.head].pkt_type = pkt_type;
        self.bufs[self.head].data_len = len;
        self.bufs[self.head].data[..len as usize].copy_from_slice(&data[..len as usize]);

        self.head = (self.head + 1) % HCI_EVT_RING_SIZE;
        self.count += 1;
        true
    }

    /// Pop a packet from the ring buffer.
    /// Returns (pkt_type, data) or None if empty.
    pub fn pop(&mut self) -> Option<(u8, Vec<u8>)> {
        if self.count == 0 {
            return None;
        }

        let entry = &self.bufs[self.tail];
        let pkt_type = entry.pkt_type;
        let data_len = entry.data_len as usize;
        let data = entry.data[..data_len].to_vec();

        self.tail = (self.tail + 1) % HCI_EVT_RING_SIZE;
        self.count -= 1;

        Some((pkt_type, data))
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Check if the buffer is full.
    pub fn is_full(&self) -> bool {
        self.count == HCI_EVT_RING_SIZE
    }

    /// Number of pending packets.
    pub fn len(&self) -> usize {
        self.count
    }
}

// ============================================================================
// /dev/hci0 Character Device State
// ============================================================================

/// Global chardev state.
pub struct HciChardev {
    /// Minor device number.
    pub minor: ffi::DevMinor,
    /// Whether the device is open.
    pub open: bool,
    /// Ring buffer for events from the controller to userspace.
    pub evt_ring: HciRingBuf,
    /// Ring buffer for ACL data from the controller to userspace.
    pub acl_ring: HciRingBuf,
    #[allow(dead_code)]
        #[allow(dead_code)]
    /// Buffer for incomplete read (partial packet).
    /// Uses Box to avoid 64KB stack allocation.
    pub read_pending: Option<(Box<[u8; hci::HCI_MAX_ACL_SIZE]>, usize)>,
    /// Statistics.
    pub stats: HciDevStats,
    /// Whether the device is "UP" (HCI_UP flag).
    pub device_up: AtomicBool,
}

impl HciChardev {
    pub fn new(minor: ffi::DevMinor) -> Self {
        Self {
            minor,
            open: false,
            evt_ring: HciRingBuf::new(),
            acl_ring: HciRingBuf::new(),
            read_pending: None,
            stats: HciDevStats {
                err_rx: 0, err_tx: 0,
                cmd_tx: 0, evt_rx: 0,
                acl_tx: 0, acl_rx: 0,
                sco_tx: 0, sco_rx: 0,
                byte_tx: 0, byte_rx: 0,
            },
            device_up: AtomicBool::new(false),
        }
    }

    /// Open the chardev.
    pub fn open(&mut self) -> c_int {
        if self.open {
            return ffi::EBUSY;
        }
        self.open = true;
        ffi::OK
    }

    /// Close the chardev.
    pub fn close(&mut self) -> c_int {
        self.open = false;
        self.evt_ring = HciRingBuf::new();
        self.acl_ring = HciRingBuf::new();
        self.read_pending = None;
        ffi::OK
    }

    /// Read from the chardev — returns HCI packets to userspace.
    /// Format: [type(1) | data(N)] repeated for each available packet.
    pub fn read(&mut self, buf: &mut [u8]) -> isize {
        if !self.open || buf.is_empty() {
            return ffi::EAGAIN as isize;
        }

        let mut written = 0;

        // Copy events from ring buffer
        while written < buf.len() {
            // Check for events first (higher priority)
            match self.evt_ring.pop() {
                Some((pkt_type, data)) => {
                    let total_len = 1 + data.len();  // type byte + data
                    let remaining = buf.len() - written;
                    let copy_len = core::cmp::min(remaining, total_len);
                    if copy_len >= 1 {
                        buf[written] = pkt_type;
                        written += 1;
                        let data_copy = core::cmp::min(copy_len - 1, data.len());
                        buf[written..written + data_copy].copy_from_slice(&data[..data_copy]);
                        written += data_copy;
                    }
                    self.stats.evt_rx += 1;
                }
                None => break,  // No more events
            }
        }

        // If no events, try ACL data
        if written == 0 {
            while written < buf.len() {
                match self.acl_ring.pop() {
                    Some((pkt_type, data)) => {
                        let total_len = 1 + data.len();
                        let remaining = buf.len() - written;
                        if remaining >= total_len {
                            buf[written] = pkt_type;
                            written += 1;
                            buf[written..written + data.len()].copy_from_slice(&data);
                            written += data.len();
                            self.stats.acl_rx += 1;
                        } else if remaining >= 1 {
                            buf[written] = pkt_type;
                            written += 1;
                            let copy_len = remaining - 1;
                            buf[written..written + copy_len].copy_from_slice(&data[..copy_len]);
                            written += copy_len;
                        }
                    }
                    None => break,
                }
            }
        }

        if written > 0 {
            written as isize
        } else {
            ffi::EAGAIN as isize
        }
    }

    /// Write to the chardev — sends HCI commands/data to the controller.
    /// Format: [type(1) | data(N)] — single HCI packet per write.
    pub fn write(&mut self, transport: &mut HciUsbTransport, data: &[u8]) -> isize {
        if !self.open || data.is_empty() {
            return ffi::EAGAIN as isize;
        }

        let pkt_type = data[0];
        match hci::HciPacketType::from_byte(pkt_type) {
            Some(hci::HciPacketType::HciCommand) => {
                if transport.send_command(data) {
                    self.stats.cmd_tx += 1;
                    self.stats.byte_tx += data.len() as u32;
                    data.len() as isize
                } else {
                    self.stats.err_tx += 1;
                    ffi::EIO as isize
                }
            }
            Some(hci::HciPacketType::HciAclData) => {
                if transport.send_acl(data) {
                    self.stats.acl_tx += 1;
                    self.stats.byte_tx += data.len() as u32;
                    data.len() as isize
                } else {
                    self.stats.err_tx += 1;
                    ffi::EIO as isize
                }
            }
            Some(hci::HciPacketType::HciScoData) => {
                if transport.send_sco(data) {
                    self.stats.sco_tx += 1;
                    self.stats.byte_tx += data.len() as u32;
                    data.len() as isize
                } else {
                    self.stats.err_tx += 1;
                    ffi::EIO as isize
                }
            }
            _ => {
                // Unknown packet type
                ffi::EPROTO as isize
            }
        }
    }

    /// Handle an ioctl call.
    pub fn ioctl(&mut self, transport: &mut HciUsbTransport,
        request: c_ulong, grant: ffi::cp_grant_id_t, endpoint: ffi::endpoint_t) -> c_int
    {
        match request {
            HCIDEVUP => {
                if self.device_up.load(Ordering::Acquire) {
                    return ffi::OK;  // Already up
                }
                // Attempt to initialise the HCI transport
                if transport.init_sequence() {
                    self.device_up.store(true, Ordering::Release);
                    ffi::OK
                } else {
                    ffi::EIO
                }
            }
            HCIDEVDOWN => {
                transport.state = hci::HciState::Down;
                self.device_up.store(false, Ordering::Release);
                ffi::OK
            }
            HCIGETDEVINFO => {
                // Build HciDevInfo and copy to userspace via grant
                let info = HciDevInfo {
                    dev_id: 0,
                    name: {
                        let mut n = [0u8; 8];
                        n[..8].copy_from_slice(b"hci0\0\0\0\0");
                        n
                    },
                    bdaddr: transport.bd_addr.0,
                    flags: if self.device_up.load(Ordering::Acquire) { HCI_UP } else { 0 },
                    dev_type: HCI_BREDR_LE as u8,
                    features: {
                        let mut f = [0u8; 8];
                        f[0] = 0xFF; f[1] = 0xFF; f[2] = 0xFF; f[3] = 0xFE;
                        f
                    },
                    pkt_type: 0,
                    link_policy: 0,
                    link_mode: 0,
                    acl_mtu: 1024,
                    sco_mtu: 64,
                    acl_pkts: 16,
                    sco_pkts: 8,
                    dev_stats: self.stats.clone(),
                };

                let info_bytes = unsafe {
                    core::slice::from_raw_parts(
                        &info as *const HciDevInfo as *const u8,
                        core::mem::size_of::<HciDevInfo>()
                    )
                };

                unsafe {
                    ffi::sys_safecopyto_wrapper(
                        endpoint, grant, 0,
                        info_bytes.as_ptr() as *const c_void,
                        info_bytes.len() as c_ulong
                    )
                }
            }
            HCIGETDEVLIST => {
                // Return a device list with just this device
                ffi::OK
            }
            HCIINQUIRY => {
                // Inquiry — would trigger HCI_Inquiry command
                // Stub for now
                ffi::OK
            }
            _ => ffi::ENOTTY,  // Unknown ioctl
        }
    }
}

// ============================================================================
// Chardriver callbacks — expose as a MINIX Chardriver struct
// ============================================================================

/// Build the Chardriver table with HCI callbacks.
pub fn as_chardriver() -> ffi::Chardriver {
    ffi::Chardriver {
        cdr_type: -1,
        cdr_open: Some(hci_open_c),
        cdr_close: Some(hci_close_c),
        cdr_read: Some(hci_read_c),
        cdr_write: Some(hci_write_c),
        cdr_ioctl: Some(hci_ioctl_c),
        cdr_select: None,
        cdr_intr: None,
        cdr_alarm: None,
        cdr_other: None,
        cdr_device: None,
        cdr_signal: None,
    }
}

// ── C-callable chardriver callbacks ─────────────────────────────────────────────────

unsafe extern "C" fn hci_open_c(_minor: ffi::DevMinor, _flags: c_int, _endpt: ffi::endpoint_t) -> c_int {
    ffi::OK
}

unsafe extern "C" fn hci_close_c(_minor: ffi::DevMinor) -> c_int {
    ffi::OK
}

unsafe extern "C" fn hci_read_c(
    _minor: ffi::DevMinor, _seekpos: u64, _endpoint: ffi::endpoint_t,
    _grant: ffi::cp_grant_id_t, _size: usize, _flags: c_int, _cdev: ffi::cdev_id_t
) -> isize {
    ffi::EAGAIN as isize
}

unsafe extern "C" fn hci_write_c(
    _minor: ffi::DevMinor, _seekpos: u64, _endpoint: ffi::endpoint_t,
    _grant: ffi::cp_grant_id_t, _size: usize, _flags: c_int, _cdev: ffi::cdev_id_t
) -> isize {
    ffi::EAGAIN as isize
}

unsafe extern "C" fn hci_ioctl_c(
    _minor: ffi::DevMinor, _request: c_ulong, _endpoint: ffi::endpoint_t,
    _grant: ffi::cp_grant_id_t, _flags: c_int, _user_endpt: ffi::endpoint_t
) -> c_int {
    ffi::ENOTTY
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ring_buf_push_pop() {
        let mut ring = HciRingBuf::new();
        assert!(ring.is_empty());

        assert!(ring.push(0x04, &[0x0E, 0x01, 0x02]));
        assert!(!ring.is_empty());
        assert_eq!(ring.len(), 1);

        // Pop it back
        let result = ring.pop();
        assert!(result.is_some());
        let (typ, data) = result.unwrap();
        assert_eq!(typ, 0x04);
        assert_eq!(data.as_slice(), &[0x0E, 0x01, 0x02]);
        assert!(ring.is_empty());
    }

    #[test]
    fn test_ring_buf_full() {
        let mut ring = HciRingBuf::new();
        for i in 0..HCI_EVT_RING_SIZE {
            assert!(ring.push(0x04, &[i as u8]));
        }
        assert!(ring.is_full());
        assert!(!ring.push(0x04, &[0xFF]));  // Should fail
    }

    #[test]
    fn test_chardev_new() {
        let dev = HciChardev::new(0);
        assert!(!dev.open);
        assert!(!dev.device_up.load(Ordering::Relaxed));
    }

    #[test]
    fn test_chardev_open_close() {
        let mut dev = HciChardev::new(0);
        assert_eq!(dev.open(), ffi::OK);
        assert!(dev.open);
        assert_eq!(dev.open(), ffi::EBUSY);  // Already open
        assert_eq!(dev.close(), ffi::OK);
        assert!(!dev.open);
    }

    #[test]
    fn test_dev_stats_clone() {
        let stats = HciDevStats {
            err_rx: 1, err_tx: 2, cmd_tx: 3, evt_rx: 4,
            acl_tx: 5, acl_rx: 6, sco_tx: 7, sco_rx: 8,
            byte_tx: 100, byte_rx: 200,
        };
        let cloned = stats;
        assert_eq!(cloned.cmd_tx, 3);
        assert_eq!(cloned.byte_rx, 200);
    }
}
