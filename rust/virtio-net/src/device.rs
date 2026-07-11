//! # Virtio Device — PCI transport layer for virtio-net
//!
//! Implements virtio device discovery via PCI, I/O port register access,
//! feature negotiation, status lifecycle, and queue setup.
//!
//! Adapted from `virtio-blk/src/virtio.rs` for the netdriver context.

#![allow(dead_code)]

use core::ffi::c_int;

use crate::queue::VirtQueue;

// ============================================================================
// Virtio PCI vendor / device IDs
// ============================================================================

pub const VIRTIO_VENDOR_ID: u16 = 0x1AF4;

// ============================================================================
// Virtio PCI register offsets (legacy, I/O port BAR)
// ============================================================================

pub const HOST_F_OFF: u16 = 0x0000;     // Host features (R)
pub const GUEST_F_OFF: u16 = 0x0004;    // Guest features (W)
pub const QADDR_OFF: u16 = 0x0008;      // Queue PFN (W)
pub const QSIZE_OFF: u16 = 0x000C;      // Queue size (R)
pub const QSEL_OFF: u16 = 0x000E;       // Queue select (W)
pub const QNOTIFY_OFF: u16 = 0x0010;    // Queue notify (W)
pub const DEV_STATUS_OFF: u16 = 0x0012; // Device status (R/W)
pub const ISR_STATUS_OFF: u16 = 0x0013; // ISR status (R)
pub const DEV_SPECIFIC_OFF: u16 = 0x0014; // Device-specific config start

pub const STATUS_ACK: u8 = 0x01;
pub const STATUS_DRV: u8 = 0x02;
pub const STATUS_DRV_OK: u8 = 0x04;
pub const STATUS_FAIL: u8 = 0x80;

// ============================================================================
// Feature flags (common)
// ============================================================================

pub const F_INDIRECT_DESC: u32 = 28;

// ============================================================================
// Virtio device
// ============================================================================

pub struct VirtioDevice {
    pub port: u16,           // I/O port base address (BAR 0)
    pub irq: c_int,          // IRQ line
    pub hook_id: c_int,      // IRQ hook ID
    pub num_queues: usize,   // Number of allocated queues
    pub queues: *mut VirtQueue, // Pointer to queue array
}

impl VirtioDevice {
    pub fn new() -> Self {
        VirtioDevice {
            port: 0,
            irq: 0,
            hook_id: 0,
            num_queues: 0,
            queues: core::ptr::null_mut(),
        }
    }

    /// Probe for a virtio-net device at the given instance.
    /// Returns the PCI devind and a configured VirtioDevice, or None.
    pub fn probe(subdevid: u16, instance: c_int) -> Option<(c_int, Self)> {
        pci_init_ffi();
        let mut dev = Self::new();

        let mut found = false;
        let mut skip = instance;

        // Wrap the pci_first_dev / pci_next_dev loop
        let first = pci_first_dev_ffi()?;
        let mut iter_devind = first;
        let mut found = false;

        loop {
            let vid = pci_attr_r16_ffi(iter_devind, 0x00); // PCI_VENDOR_ID
            let sdid = pci_attr_r16_ffi(iter_devind, 0x2E); // PCI_SUBSYSTEM_ID

            if vid == VIRTIO_VENDOR_ID && sdid == subdevid {
                if skip == 0 {
                    found = true;
                    pci_reserve_ffi(iter_devind);

                    // Get BAR 0 (I/O port base)
                    let (base, _size, ioflag) = pci_get_bar_ffi(iter_devind, 0)?;
                    if !ioflag || (base & 0xFFFF0000) != 0 {
                        return None; // BAR 0 must be I/O space in low 64K
                    }
                    dev.port = base as u16;

                    // Reset device
                    unsafe { dev.write_status(0); }

                    // Read IRQ line
                    dev.irq = pci_attr_r8_ffi(iter_devind, 0x3C) as c_int;

                    // Enable bus mastering
                    let cr = pci_attr_r16_ffi(iter_devind, 0x04);
                    if (cr & 0x0004) == 0 {
                        pci_attr_w16_ffi(iter_devind, 0x04, cr | 0x0004);
                    }

                    break;
                }
                skip -= 1;
            }

            let next = pci_next_dev_ffi();
            match next {
                Some(d) => { iter_devind = d; }
                None => { break; }
            }
        }

        if !found { return None; }

        // ACK the device
        unsafe { dev.write_status(STATUS_ACK); }

        Some((iter_devind, dev))
    }

    /// Negotiate features: host_features & guest_bits -> write guest_features.
    /// Returns the set of negotiated features.
    pub fn negotiate_features(&self, guest_bits: u32) -> u32 {
        unsafe {
            let host = self.read32(HOST_F_OFF);
            let negotiated = host & guest_bits;
            self.write32(GUEST_F_OFF, negotiated);
            negotiated
        }
    }

    /// Allocate and initialise `num` virtqueues.
    /// Must be called after `negotiate_features()`.
    pub fn alloc_queues(&mut self, num: usize) -> Result<(), c_int> {
        self.num_queues = num;

        // Allocate queue array
        let queue_size = core::mem::size_of::<VirtQueue>() * num;
        let ptr = alloc_queue_array(queue_size)?;
        self.queues = ptr as *mut VirtQueue;
        unsafe { core::ptr::write_bytes(ptr, 0, queue_size); }

        for i in 0..num {
            let q = unsafe { &mut *self.queues.add(i) };

            // Select queue
            unsafe { self.write16(QSEL_OFF, i as u16); }

            // Read queue size (must be power of 2)
            let qsz = unsafe { self.read16(QSIZE_OFF) };
            if qsz & (qsz - 1) != 0 || qsz == 0 {
                return Err(errno::EINVAL);
            }

            // Allocate the actual ring memory
            *q = match VirtQueue::allocate(qsz) {
                Some(q) => q,
                None => return Err(errno::ENOMEM),
            };

            // Tell the host about the queue (guest page number = phys / 4096)
            let page = (q.paddr / 4096) as u32;
            unsafe {
                self.write32(QADDR_OFF, page);
            }
        }

        Ok(())
    }

    /// Register IRQ and set DRV_OK (driver ready).
    pub fn ready(&mut self) -> Result<(), c_int> {
        let hook = match irq_setup(self.irq) {
            Some(h) => h,
            None => return Err(errno::EIO),
        };
        self.hook_id = hook;
        unsafe { self.write_status(STATUS_DRV_OK); }
        Ok(())
    }

    /// Reset the device.
    pub fn reset(&mut self) {
        unsafe { self.write_status(0); }
        if self.hook_id != 0 {
            irq_remove(&mut self.hook_id);
            self.hook_id = 0;
        }
    }

    /// Free all queue memory and clean up.
    pub fn cleanup(&mut self) {
        if !self.queues.is_null() {
            for i in 0..self.num_queues {
                let q = unsafe { &mut *self.queues.add(i) };
                q.free_resources();
            }
            let size = core::mem::size_of::<VirtQueue>() * self.num_queues;
            free_queue_array(self.queues as *mut core::ffi::c_void, size);
            self.queues = core::ptr::null_mut();
        }
        self.num_queues = 0;
    }

    /// Check if the device raised an interrupt (ISR bit 0).
    pub fn had_irq(&self) -> bool {
        unsafe { self.read8(ISR_STATUS_OFF) & 1 != 0 }
    }

    /// Re-enable IRQ after handling.
    pub fn irq_reenable(&self) {
        irq_reenable(&self.hook_id);
    }

    // ========================================================================
    // I/O port register access
    // ========================================================================

    pub unsafe fn read8(&self, offset: u16) -> u8 {
        port_inb(self.port + offset)
    }

    pub unsafe fn read16(&self, offset: u16) -> u16 {
        port_inw(self.port + offset)
    }

    pub unsafe fn read32(&self, offset: u16) -> u32 {
        port_inl(self.port + offset)
    }

    pub unsafe fn write8(&self, offset: u16, val: u8) {
        port_outb(self.port + offset, val);
    }

    pub unsafe fn write16(&self, offset: u16, val: u16) {
        port_outw(self.port + offset, val);
    }

    pub unsafe fn write32(&self, offset: u16, val: u32) {
        port_outl(self.port + offset, val);
    }

    /// Read device status register.
    pub unsafe fn read_status(&self) -> u8 {
        self.read8(DEV_STATUS_OFF)
    }

    /// Write device status register.
    pub unsafe fn write_status(&self, status: u8) {
        self.write8(DEV_STATUS_OFF, status);
    }

    /// Read device-specific config at `offset`.
    pub unsafe fn sread8(&self, offset: u16) -> u8 {
        self.read8(DEV_SPECIFIC_OFF + offset)
    }

    pub unsafe fn sread16(&self, offset: u16) -> u16 {
        self.read16(DEV_SPECIFIC_OFF + offset)
    }

    pub unsafe fn sread32(&self, offset: u16) -> u32 {
        self.read32(DEV_SPECIFIC_OFF + offset)
    }

    /// Kick queue `qidx`.
    pub unsafe fn kick(&self, qidx: u16) {
        self.write16(QNOTIFY_OFF, qidx);
    }
}

impl Drop for VirtioDevice {
    fn drop(&mut self) {
        self.reset();
        self.cleanup();
    }
}

// ============================================================================
// FFI wrappers
// ============================================================================

#[cfg(target_os = "minix")]
mod ffi_imports {
    use core::ffi::c_int;

    extern "C" {
        // PCI
        pub fn pci_init() -> c_int;
        pub fn pci_first_dev(devindp: *mut c_int, vidp: *mut u16, didp: *mut u16) -> c_int;
        pub fn pci_next_dev(devindp: *mut c_int, vidp: *mut u16, didp: *mut u16) -> c_int;
        pub fn pci_reserve(devind: c_int);
        pub fn pci_get_bar(devind: c_int, bar: c_int, base: *mut u32,
            size_: *mut u32, ioflag: *mut c_int) -> c_int;
        pub fn pci_attr_r8(devind: c_int, offset: c_int) -> u8;
        pub fn pci_attr_r16(devind: c_int, offset: c_int) -> u16;
        pub fn pci_attr_r32(devind: c_int, offset: c_int) -> u32;
        pub fn pci_attr_w16(devind: c_int, offset: c_int, value: u16);

        // Port I/O
        pub fn sys_inb(port: u16, value: *mut u32) -> c_int;
        pub fn sys_inw(port: u16, value: *mut u32) -> c_int;
        pub fn sys_inl(port: u16, value: *mut u32) -> c_int;
        pub fn sys_outb(port: u16, value: u8) -> c_int;
        pub fn sys_outw(port: u16, value: u16) -> c_int;
        pub fn sys_outl(port: u16, value: u32) -> c_int;

        // Physical memory
        pub fn alloc_contig(size: usize, flags: c_int, phys: *mut u64) -> *mut core::ffi::c_void;
        pub fn free_contig(addr: *mut core::ffi::c_void, size: usize);

        // IRQ
        pub fn sys_irqsetpolicy(irq: c_int, policy: c_int, hook_id: *mut c_int) -> c_int;
        pub fn sys_irqenable(hook_id: *mut c_int) -> c_int;
        pub fn sys_irqrmpolicy(hook_id: *mut c_int) -> c_int;
    }
}

#[cfg(not(target_os = "minix"))]
mod ffi_imports {
    // Stubs for host-side testing
    use core::ffi::c_int;

    pub unsafe fn pci_init() -> c_int { 0 }
    pub unsafe fn pci_first_dev(_: *mut c_int, _: *mut u16, _: *mut u16) -> c_int { 0 }
    pub unsafe fn pci_next_dev(_: *mut c_int, _: *mut u16, _: *mut u16) -> c_int { 0 }
    pub unsafe fn pci_reserve(_: c_int) {}
    pub unsafe fn pci_get_bar(_: c_int, _: c_int, base: *mut u32,
        size_: *mut u32, _: *mut c_int) -> c_int { unsafe { *base = 0; *size_ = 0x100; 0 } }
    pub unsafe fn pci_attr_r8(_: c_int, _: c_int) -> u8 { 0 }
    pub unsafe fn pci_attr_r16(_: c_int, _: c_int) -> u16 { 0 }
    pub unsafe fn pci_attr_r32(_: c_int, _: c_int) -> u32 { 0 }
    pub unsafe fn pci_attr_w16(_: c_int, _: c_int, _: u16) {}

    pub unsafe fn sys_inb(_: u16, value: *mut u32) -> c_int { unsafe { *value = 0xFF; 0 } }
    pub unsafe fn sys_inw(_: u16, value: *mut u32) -> c_int { unsafe { *value = 0xFFFF; 0 } }
    pub unsafe fn sys_inl(_: u16, value: *mut u32) -> c_int { unsafe { *value = 0xFFFFFFFF; 0 } }
    pub unsafe fn sys_outb(_: u16, _: u8) -> c_int { 0 }
    pub unsafe fn sys_outw(_: u16, _: u16) -> c_int { 0 }
    pub unsafe fn sys_outl(_: u16, _: u32) -> c_int { 0 }

    pub unsafe fn alloc_contig(_: usize, _: c_int, phys: *mut u64) -> *mut core::ffi::c_void {
        unsafe { *phys = 0x100000; }
        core::ptr::null_mut()
    }
    pub unsafe fn free_contig(_: *mut core::ffi::c_void, _: usize) {}

    pub unsafe fn sys_irqsetpolicy(_: c_int, _: c_int, _: *mut c_int) -> c_int { 0 }
    pub unsafe fn sys_irqenable(_: *mut c_int) -> c_int { 0 }
    pub unsafe fn sys_irqrmpolicy(_: *mut c_int) -> c_int { 0 }
}

use ffi_imports::*;

fn pci_init_ffi() -> bool { unsafe { pci_init() >= 0 } }
fn pci_first_dev_ffi() -> Option<c_int> {
    unsafe {
        let mut devind: c_int = 0;
        let mut vid: u16 = 0;
        let mut did: u16 = 0;
        let r = pci_first_dev(&mut devind, &mut vid, &mut did);
        if r <= 0 { None } else { Some(devind) }
    }
}
fn pci_next_dev_ffi() -> Option<c_int> {
    unsafe {
        let mut devind: c_int = 0;
        let mut vid: u16 = 0;
        let mut did: u16 = 0;
        let r = pci_next_dev(&mut devind, &mut vid, &mut did);
        if r <= 0 { None } else { Some(devind) }
    }
}
fn pci_reserve_ffi(devind: c_int) { unsafe { pci_reserve(devind) } }
fn pci_get_bar_ffi(devind: c_int, bar: c_int) -> Option<(u32, u32, bool)> {
    unsafe {
        let mut base: u32 = 0;
        let mut size_: u32 = 0;
        let mut ioflag: c_int = 0;
        let r = pci_get_bar(devind, bar, &mut base, &mut size_, &mut ioflag);
        if r != 0 { None } else { Some((base, size_, ioflag != 0)) }
    }
}
fn pci_attr_r8_ffi(devind: c_int, offset: c_int) -> u8 {
    unsafe { pci_attr_r8(devind, offset) }
}
fn pci_attr_r16_ffi(devind: c_int, offset: c_int) -> u16 {
    unsafe { pci_attr_r16(devind, offset) }
}
fn pci_attr_r32_ffi(devind: c_int, offset: c_int) -> u32 {
    unsafe { pci_attr_r32(devind, offset) }
}
fn pci_attr_w16_ffi(devind: c_int, offset: c_int, value: u16) {
    unsafe { pci_attr_w16(devind, offset, value) }
}

// Port I/O
unsafe fn port_inb(port: u16) -> u8 {
    #[cfg(target_os = "minix")]
    { let mut val: u32 = 0; let _ = sys_inb(port, &mut val); val as u8 }
    #[cfg(not(target_os = "minix"))]
    { let mut val: u32 = 0; let _ = sys_inb(port, &mut val); val as u8 }
}
unsafe fn port_inw(port: u16) -> u16 {
    #[cfg(target_os = "minix")]
    { let mut val: u32 = 0; let _ = sys_inw(port, &mut val); val as u16 }
    #[cfg(not(target_os = "minix"))]
    { let mut val: u32 = 0; let _ = sys_inw(port, &mut val); val as u16 }
}
unsafe fn port_inl(port: u16) -> u32 {
    #[cfg(target_os = "minix")]
    { let mut val: u32 = 0; let _ = sys_inl(port, &mut val); val }
    #[cfg(not(target_os = "minix"))]
    { let mut val: u32 = 0; let _ = sys_inl(port, &mut val); val }
}
unsafe fn port_outb(port: u16, val: u8) {
    #[cfg(target_os = "minix")]
    { let _ = sys_outb(port, val); }
    #[cfg(not(target_os = "minix"))]
    { let _ = sys_outb(port, val); }
}
unsafe fn port_outw(port: u16, val: u16) {
    #[cfg(target_os = "minix")]
    { let _ = sys_outw(port, val); }
    #[cfg(not(target_os = "minix"))]
    { let _ = sys_outw(port, val); }
}
unsafe fn port_outl(port: u16, val: u32) {
    #[cfg(target_os = "minix")]
    { let _ = sys_outl(port, val); }
    #[cfg(not(target_os = "minix"))]
    { let _ = sys_outl(port, val); }
}

fn irq_setup(irq: c_int) -> Option<c_int> {
    unsafe {
        let mut hook_id: c_int = 0;
        let r = sys_irqsetpolicy(irq, 0, &mut hook_id);
        if r != 0 { return None; }
        let r = sys_irqenable(&mut hook_id);
        if r != 0 { return None; }
        Some(hook_id)
    }
}

fn irq_reenable(hook_id: &c_int) -> c_int {
    unsafe { sys_irqenable(hook_id as *const c_int as *mut c_int) }
}

fn irq_remove(hook_id: &mut c_int) -> c_int {
    unsafe { sys_irqrmpolicy(hook_id) }
}

/// Allocate a fixed-size array for queue pointers.
fn alloc_queue_array(size: usize) -> Result<*mut core::ffi::c_void, c_int> {
    #[cfg(target_os = "minix")]
    {
        unsafe {
            const AC_ALIGN4K: c_int = 1;
            let mut phys: u64 = 0;
            let ptr = alloc_contig(size, AC_ALIGN4K, &mut phys);
            if ptr.is_null() { Err(errno::ENOMEM) } else { Ok(ptr) }
        }
    }
    #[cfg(not(target_os = "minix"))]
    {
        let _ = size;
        Err(errno::ENOMEM)
    }
}

fn free_queue_array(ptr: *mut core::ffi::c_void, size: usize) {
    #[cfg(target_os = "minix")]
    unsafe { free_contig(ptr, size); }
    #[cfg(not(target_os = "minix"))]
    { let _ = (ptr, size); }
}

// ============================================================================
// Common errno values
// ============================================================================

mod errno {
    #![allow(dead_code)]
    pub const OK: i32 = 0;
    pub const EIO: i32 = -5;
    pub const ENXIO: i32 = -6;
    pub const ENOMEM: i32 = -12;
    pub const EBUSY: i32 = -16;
    pub const EINVAL: i32 = -22;
}

// ============================================================================
// C FFI test verification functions
// ============================================================================

/// Return a version identifier: 0x00010000 (legacy virtio 0.9.5 / pre-1.0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn virtio_test_version() -> u32 {
    0x00010000
}

/// Return virtio register offset by register ID (0..=8).
/// Returns 0xFFFFFFFF for unknown IDs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn virtio_test_reg_offset(reg_id: u32) -> u32 {
    match reg_id {
        0 => HOST_F_OFF as u32,
        1 => GUEST_F_OFF as u32,
        2 => QADDR_OFF as u32,
        3 => QSIZE_OFF as u32,
        4 => QSEL_OFF as u32,
        5 => QNOTIFY_OFF as u32,
        6 => DEV_STATUS_OFF as u32,
        7 => ISR_STATUS_OFF as u32,
        8 => DEV_SPECIFIC_OFF as u32,
        _ => 0xFFFFFFFF,
    }
}

/// Return a bitfield/constant value by bitfield ID (0..=28).
/// Returns 0xFFFFFFFF for unknown IDs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn virtio_test_bitfield(bf_id: u32) -> u32 {
    match bf_id {
        // Status flags (0-3)
        0 => STATUS_ACK as u32,
        1 => STATUS_DRV as u32,
        2 => STATUS_DRV_OK as u32,
        3 => STATUS_FAIL as u32,
        // Feature flags (4-5)
        4 => F_INDIRECT_DESC,
        // Vring descriptor flags (5-7)
        5 => crate::queue::VRING_DESC_F_NEXT as u32,
        6 => crate::queue::VRING_DESC_F_WRITE as u32,
        7 => crate::queue::VRING_DESC_F_INDIRECT as u32,
        // Vring struct sizes (8-9)
        8 => core::mem::size_of::<crate::queue::VringDesc>() as u32,
        9 => core::mem::size_of::<crate::queue::VringUsedElem>() as u32,
        // Virtio-net feature bits (10-29)
        10 => crate::net::VIRTIO_NET_F_CSUM,
        11 => crate::net::VIRTIO_NET_F_GUEST_CSUM,
        12 => crate::net::VIRTIO_NET_F_MAC,
        13 => crate::net::VIRTIO_NET_F_GSO,
        14 => crate::net::VIRTIO_NET_F_STATUS,
        15 => crate::net::VIRTIO_NET_F_CTRL_VQ,
        16 => crate::net::VIRTIO_NET_F_MRG_RXBUF,
        // Net status flags (17-18)
        17 => crate::net::VIRTIO_NET_S_LINK_UP as u32,
        18 => crate::net::VIRTIO_NET_S_ANNOUNCE as u32,
        // Header struct sizes (19-20)
        19 => core::mem::size_of::<crate::net::VirtioNetHdr>() as u32,
        20 => core::mem::size_of::<crate::net::VirtioNetHdrMrgRxbuf>() as u32,
        // Header flags (21-22)
        21 => crate::net::VIRTIO_NET_HDR_F_NEEDS_CSUM as u32,
        22 => crate::net::VIRTIO_NET_HDR_F_DATA_VALID as u32,
        // GSO types (23-26)
        23 => crate::net::VIRTIO_NET_HDR_GSO_NONE as u32,
        24 => crate::net::VIRTIO_NET_HDR_GSO_TCPV4 as u32,
        25 => crate::net::VIRTIO_NET_HDR_GSO_TCPV6 as u32,
        26 => crate::net::VIRTIO_NET_HDR_GSO_ECN as u32,
        // Queue indices (27-29)
        27 => crate::net::RX_Q as u32,
        28 => crate::net::TX_Q as u32,
        29 => crate::net::CTRL_Q as u32,
        // Driver constants (30-31)
        30 => crate::driver::BUF_PACKETS as u32,
        31 => crate::driver::MAX_PACK_SIZE as u32,
        _ => 0xFFFFFFFF,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errno_constants() {
        assert_eq!(errno::EIO, -5);
        assert_eq!(errno::ENOMEM, -12);
        assert_eq!(errno::EINVAL, -22);
    }

    #[test]
    fn register_offsets() {
        assert_eq!(HOST_F_OFF, 0x0000);
        assert_eq!(GUEST_F_OFF, 0x0004);
        assert_eq!(QSEL_OFF, 0x000E);
        assert_eq!(DEV_STATUS_OFF, 0x0012);
        assert_eq!(DEV_SPECIFIC_OFF, 0x0014);
    }

    #[test]
    fn test_ffi_version() {
        let v = unsafe { virtio_test_version() };
        assert_eq!(v, 0x00010000);
    }

    #[test]
    fn test_ffi_reg_offsets() {
        assert_eq!(unsafe { virtio_test_reg_offset(0) }, 0x0000);
        assert_eq!(unsafe { virtio_test_reg_offset(6) }, 0x0012);
        assert_eq!(unsafe { virtio_test_reg_offset(8) }, 0x0014);
        assert_eq!(unsafe { virtio_test_reg_offset(99) }, 0xFFFFFFFF);
    }

    #[test]
    fn test_ffi_bitfields() {
        assert_eq!(unsafe { virtio_test_bitfield(0) }, 0x01); // STATUS_ACK
        assert_eq!(unsafe { virtio_test_bitfield(5) }, 1);    // VRING_DESC_F_NEXT
        assert_eq!(unsafe { virtio_test_bitfield(8) }, 16);   // VringDesc size
        assert_eq!(unsafe { virtio_test_bitfield(19) }, 10);  // VirtioNetHdr size
        assert_eq!(unsafe { virtio_test_bitfield(99) }, 0xFFFFFFFF);
    }
}
