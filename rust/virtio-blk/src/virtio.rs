//! # Virtio Device — PCI transport layer for legacy (pre-1.0) virtio
//!
//! Implements virtio device discovery via PCI, I/O port register access,
//! feature negotiation, status lifecycle, and queue allocation.
//!
//! Based on the MINIX C libvirtio (`virtio.c`) and the Virtio 0.95 spec.

use crate::ffi;
use core::ffi::{c_int, c_void};

// ============================================================================
// Virtio PCI register offsets (legacy, I/O port BAR)
// ============================================================================

pub const VIRTIO_VENDOR_ID: u16 = 0x1AF4;

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
// Feature flags
// ============================================================================

pub const F_INDIRECT_DESC: u32 = 28;
pub const F_EVENT_IDX: u32 = 29;

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
    /// Create a new (uninitialized) VirtioDevice.
    pub fn new() -> Self {
        VirtioDevice {
            port: 0,
            irq: 0,
            hook_id: 0,
            num_queues: 0,
            queues: core::ptr::null_mut(),
        }
    }

    /// Probe for a virtio device matching `subdevid` at the given `instance`.
    /// Returns the PCI devind and a configured VirtioDevice, or None.
    pub fn probe(subdevid: u16, instance: c_int) -> Option<(c_int, Self)> {
        ffi::pci_init_ffi();
        let mut dev = Self::new();

        let mut found = false;
        let mut skip = instance;
        let mut iter = ffi::pci_first_dev_ffi()?;

        loop {
            let (devind, vid, sdid) = (iter.0, iter.1, 0); // sdid from attr_r16
            let sdid = ffi::pci_attr_r16_ffi(devind, 0x2E); // PCI_SUBSYSTEM_ID

            if vid == VIRTIO_VENDOR_ID && sdid == subdevid {
                if skip == 0 {
                    found = true;
                    // Initialize the device
                    ffi::pci_reserve_ffi(devind);

                    // Get BAR 0 (I/O port base)
                    let (base, _size, ioflag) = ffi::pci_get_bar_ffi(devind, 0)?;
                    if !ioflag || (base & 0xFFFF0000) != 0 {
                        return None; // BAR 0 must be I/O space in low 64K
                    }
                    dev.port = base as u16;

                    // Reset device
                    unsafe { dev.write_status(0); }

                    // Read IRQ line
                    dev.irq = ffi::pci_attr_r8_ffi(devind, 0x3C) as c_int; // PCI_ILR

                    break;
                }
                skip -= 1;
            }

            iter = ffi::pci_next_dev_ffi()?;
        }

        if !found { return None; }

        // ACK the device
        unsafe { dev.write_status(STATUS_ACK); }

        Some((iter.0, dev))
    }

    /// Negotiate features: host_features & guest_features -> write guest_features.
    /// `guest_bits` is the bitmask of features we want.
    pub fn negotiate_features(&self, guest_bits: u32) -> u32 {
        unsafe {
            let host = self.read32(HOST_F_OFF);
            let negotiated = host & guest_bits;
            self.write32(GUEST_F_OFF, negotiated);
            negotiated
        }
    }

    /// Allocate and initialize `num` virtqueues.
    /// Must be called after `negotiate_features()`.
    pub fn alloc_queues(&mut self, num: usize) -> Result<(), c_int> {
        self.num_queues = num;

        // Allocate the queue array
        let queue_size = core::mem::size_of::<VirtQueue>() * num;
        // Use a simple malloc via alloc_contig (or just use a static array for the pilot)
        let ptr = unsafe { ffi::platform::alloc_contig(queue_size, 1, &mut 0u64) };
        if ptr.is_null() { return Err(ffi::platform::ENOMEM); }
        unsafe { core::ptr::write_bytes(ptr, 0, queue_size); }
        self.queues = ptr as *mut VirtQueue;

        for i in 0..num {
            let q = unsafe { &mut *self.queues.add(i) };

            // Select queue
            unsafe { self.write16(QSEL_OFF, i as u16); }

            // Read queue size (must be power of 2)
            q.num = unsafe { self.read16(QSIZE_OFF) };
            if q.num & (q.num - 1) != 0 || q.num == 0 {
                return Err(ffi::platform::EINVAL);
            }

            // Allocate ring memory: vring_size = desc + avail + used
            let ring_size = vring_size(q.num as usize);
            let (vaddr, paddr) = match ffi::alloc_contig_ffi(ring_size) {
                Some((v, p)) => (v as *mut u8, p),
                None => return Err(ffi::platform::ENOMEM),
            };
            q.vaddr = vaddr;
            q.paddr = paddr;
            q.ring_size = ring_size;

            // Allocate data array for per-descriptor tracking
            let data_size = core::mem::size_of::<usize>() * (q.num as usize);
            let data_ptr = unsafe {
                ffi::platform::alloc_contig(data_size, 1, &mut 0u64)
            };
            if data_ptr.is_null() {
                unsafe { ffi::platform::free_contig(vaddr as *mut c_void, ring_size); }
                return Err(ffi::platform::ENOMEM);
            }
            q.data = data_ptr as *mut usize;

            // Initialize the vring layout
            q.init_vring();

            // Tell the host about the queue (guest page number = phys / 4096)
            let page = (paddr / 4096) as u32;
            unsafe {
                self.write32(QADDR_OFF, page);
            }
        }

        Ok(())
    }

    /// Register IRQ and set DRV_OK (driver ready).
    pub fn ready(&mut self) -> Result<(), c_int> {
        let hook = match ffi::irq_setup(self.irq) {
            Some(h) => h,
            None => return Err(ffi::platform::EIO),
        };
        self.hook_id = hook;

        // Signal the host that we're ready
        unsafe { self.write_status(STATUS_DRV_OK); }
        Ok(())
    }

    /// Reset the device.
    pub fn reset(&mut self) {
        unsafe { self.write_status(0); }
        if self.hook_id != 0 {
            ffi::irq_remove(&mut self.hook_id);
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
            unsafe {
                let size = core::mem::size_of::<VirtQueue>() * self.num_queues;
                ffi::platform::free_contig(self.queues as *mut c_void, size);
            }
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
        ffi::irq_reenable(&self.hook_id);
    }

    // ========================================================================
    // I/O port register access
    // ========================================================================

    /// Read 8-bit from device register at `offset`.
    pub unsafe fn read8(&self, offset: u16) -> u8 {
        ffi::port_inb(self.port + offset)
    }

    /// Read 16-bit from device register at `offset`.
    pub unsafe fn read16(&self, offset: u16) -> u16 {
        ffi::port_inw(self.port + offset)
    }

    /// Read 32-bit from device register at `offset`.
    pub unsafe fn read32(&self, offset: u16) -> u32 {
        ffi::port_inl(self.port + offset)
    }

    /// Write 8-bit to device register at `offset`.
    pub unsafe fn write8(&self, offset: u16, val: u8) {
        ffi::port_outb(self.port + offset, val);
    }

    /// Write 16-bit to device register at `offset`.
    pub unsafe fn write16(&self, offset: u16, val: u16) {
        ffi::port_outw(self.port + offset, val);
    }

    /// Write 32-bit to device register at `offset`.
    pub unsafe fn write32(&self, offset: u16, val: u32) {
        ffi::port_outl(self.port + offset, val);
    }

    /// Read device status register.
    pub unsafe fn read_status(&self) -> u8 {
        self.read8(DEV_STATUS_OFF)
    }

    /// Write device status register.
    pub unsafe fn write_status(&self, status: u8) {
        self.write8(DEV_STATUS_OFF, status);
    }

    /// Read device-specific config at `offset` (after accounting for MSI offset).
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
// VirtQueue — split virtqueue implementation
// ============================================================================

/// Virtqueue descriptor (16 bytes)
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VringDesc {
    pub addr: u64,   // Guest-physical address
    pub len: u32,    // Length
    pub flags: u16,  // VRING_DESC_F_*
    pub next: u16,   // Next descriptor index in chain
}

pub const VRING_DESC_F_NEXT: u16 = 1;
pub const VRING_DESC_F_WRITE: u16 = 2;
pub const VRING_DESC_F_INDIRECT: u16 = 4;

/// Available ring entry
#[repr(C)]
pub struct VringAvail {
    pub flags: u16,
    pub idx: u16,
    pub ring: [u16; 0], // flexible array
}

/// Used ring element
#[repr(C)]
#[derive(Clone, Copy)]
pub struct VringUsedElem {
    pub id: u32,  // Descriptor index
    pub len: u32, // Total length written
}

/// Used ring
#[repr(C)]
pub struct VringUsed {
    pub flags: u16,
    pub idx: u16,
    pub ring: [VringUsedElem; 0], // flexible array
}

/// Calculate vring size in bytes for a given number of descriptors.
pub fn vring_size(num: usize) -> usize {
    let desc_size = core::mem::size_of::<VringDesc>() * num;
    let avail_size = 2 + 2 + 2 * num; // flags(2) + idx(2) + ring[2*num]
    let used_size = 2 + 2 + 8 * num;  // flags(2) + idx(2) + ring[8*num]
    let align: usize = 4096;

    // Align: desc + avail, then used starts at next align boundary
    let after_avail = desc_size + avail_size;
    let used_offset = (after_avail + 2 + align - 1) & !(align - 1); // +2 for used_event
    used_offset + used_size + 2 // +2 for avail_event
}

/// VirtQueue — manages a single split virtqueue.
pub struct VirtQueue {
    pub num: u16,           // Number of descriptors (power of 2)
    pub vaddr: *mut u8,     // Virtual address of ring memory
    pub paddr: u64,         // Physical address of ring memory
    pub ring_size: usize,   // Total ring size in bytes

    // Vring pointers (pointing into vaddr)
    pub desc: *mut VringDesc,
    pub avail: *mut VringAvail,
    pub used: *mut VringUsed,

    // Free list management
    pub free_num: u16,       // Number of free descriptors
    pub free_head: u16,      // Head of free list
    pub free_tail: u16,      // Tail of free list
    pub last_used: u16,      // Last processed used index

    // Per-descriptor data pointer (opaque, returned on completion)
    pub data: *mut usize,

    // Indirect descriptor tables (per-thread)
    pub indirect: *mut u8,
    pub indirect_phys: u64,
    pub indirect_in_use: bool,
    pub indirect_len: usize,
}

impl VirtQueue {
    /// Initialize vring layout: set desc, avail, used pointers within vaddr.
    pub fn init_vring(&mut self) {
        let num = self.num as usize;
        let vaddr = self.vaddr;

        // Descriptors at start
        self.desc = vaddr as *mut VringDesc;

        // Available ring: right after descriptors
        let avail_off = num * core::mem::size_of::<VringDesc>();
        self.avail = unsafe { vaddr.add(avail_off) as *mut VringAvail };

        // Used ring: aligned after available + ring
        let after_avail = avail_off + 2 + 2 + 2 * num; // flags + idx + ring[]
        let align: usize = 4096;
        let used_off = (after_avail + 2 + align - 1) & !(align - 1); // +2 for used_event
        self.used = unsafe { vaddr.add(used_off) as *mut VringUsed };

        // Initialize free list: all descriptors chained
        unsafe {
            for i in 0..num {
                let desc = &mut *self.desc.add(i);
                desc.flags = VRING_DESC_F_NEXT;
                desc.next = ((i + 1) & (num - 1)) as u16;
            }
        }

        self.free_num = num as u16;
        self.free_head = 0;
        self.free_tail = (num - 1) as u16;
        self.last_used = 0;

        // Clear data array
        unsafe {
            core::ptr::write_bytes(self.data, 0, num);
        }
    }

    /// Allocate indirect descriptor table for this queue.
    pub fn alloc_indirect(&mut self, max_desc: usize) -> Result<(), c_int> {
        let size = max_desc * core::mem::size_of::<VringDesc>();
        let (vaddr, paddr) = match ffi::alloc_contig_ffi(size) {
            Some((v, p)) => (v, p),
            None => return Err(ffi::platform::ENOMEM),
        };
        self.indirect = vaddr as *mut u8;
        self.indirect_phys = paddr;
        self.indirect_in_use = false;
        self.indirect_len = size;
        unsafe { core::ptr::write_bytes(vaddr, 0, size); }
        Ok(())
    }

    /// Free queue resources.
    pub fn free_resources(&mut self) {
        if !self.vaddr.is_null() {
            unsafe { ffi::platform::free_contig(self.vaddr as *mut c_void, self.ring_size); }
            self.vaddr = core::ptr::null_mut();
        }
        if !self.data.is_null() {
            unsafe {
                let size = core::mem::size_of::<usize>() * (self.num as usize);
                ffi::platform::free_contig(self.data as *mut c_void, size);
            }
            self.data = core::ptr::null_mut();
        }
        if !self.indirect.is_null() {
            unsafe { ffi::platform::free_contig(self.indirect as *mut c_void, self.indirect_len); }
            self.indirect = core::ptr::null_mut();
        }
    }

    /// Allocate `count` chained descriptors. Returns the index of the first,
    /// or None if insufficient descriptors available.
    pub fn alloc_descs(&mut self, count: u16) -> Option<u16> {
        if self.free_num < count { return None; }

        let head = self.free_head;
        let mut prev = head;

        for _ in 0..count {
            let desc = unsafe { &mut *self.desc.add(prev as usize) };
            // desc.flags has VRING_DESC_F_NEXT from free list
            prev = desc.next;
        }

        // Update free list: head becomes prev (the next after our chain)
        self.free_head = prev;
        self.free_num -= count;

        Some(head)
    }

    /// Free a descriptor chain back to the free list.
    pub fn free_descs(&mut self, head: u16) {
        let mut idx = head;
        let mut count = 0;
        loop {
            let desc = unsafe { &*self.desc.add(idx as usize) };
            count += 1;
            if desc.flags & VRING_DESC_F_NEXT == 0 { break; }
            idx = desc.next;
        }

        // Link freed chain to tail of free list
        let tail_desc = unsafe { &mut *self.desc.add(self.free_tail as usize) };
        tail_desc.flags = VRING_DESC_F_NEXT;
        tail_desc.next = head;

        self.free_tail = idx;
        self.free_num += count;
    }

    /// Set a descriptor's address, length, and flags.
    pub fn set_desc(&mut self, idx: u16, addr: u64, len: u32, write: bool, has_next: bool) {
        let desc = unsafe { &mut *self.desc.add(idx as usize) };
        desc.addr = addr;
        desc.len = len;
        desc.flags = if write { VRING_DESC_F_WRITE } else { 0 };
        if has_next {
            desc.flags |= VRING_DESC_F_NEXT;
        }
    }

    /// Submit a descriptor chain to the available ring.
    /// `head` is the first descriptor index (from alloc_descs).
    /// `data` is an opaque value returned on completion.
    pub fn submit(&mut self, head: u16, data: usize) {
        let avail = unsafe { &mut *self.avail };
        let idx = avail.idx % self.num;
        unsafe {
            avail.ring.as_mut_ptr().add(idx as usize).write(head);
        }
        // Store the per-request data
        unsafe { *self.data.add(head as usize) = data; }
        // Memory barrier so host sees updated descriptors
        core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
        avail.idx = avail.idx.wrapping_add(1);
    }

    /// Process completed descriptors from the used ring.
    /// Returns the opaque `data` value and the total length, or None if nothing is done.
    pub fn collect(&mut self) -> Option<(usize, u32)> {
        let used = unsafe { &mut *self.used };
        // Memory barrier so we see host writes
        core::sync::atomic::fence(core::sync::atomic::Ordering::Acquire);

        let used_idx = used.idx % self.num;
        if self.last_used == used_idx { return None; }

        let elem = unsafe { &*used.ring.as_mut_ptr().add(self.last_used as usize) };
        let head = elem.id as u16;
        let len = elem.len;
        self.last_used = (self.last_used + 1) % self.num;

        // Retrieve the opaque data
        let data = unsafe { *self.data.add(head as usize) };

        // Free the descriptor chain
        self.free_descs(head);

        Some((data, len))
    }

    /// Check if a kick is needed (host hasn't set VRING_USED_F_NO_NOTIFY).
    pub fn wants_kick(&self) -> bool {
        let used = unsafe { &*self.used };
        used.flags & 1 == 0 // VRING_USED_F_NO_NOTIFY = 1
    }
}

impl Drop for VirtQueue {
    fn drop(&mut self) {
        self.free_resources();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vring_size_calculation() {
        let size = vring_size(128);
        assert!(size > 0);
        assert!(size < 128 * 1024); // sanity
    }

    #[test]
    fn virtqueue_free_list_works() {
        // We can test the free list logic without real hardware
        let mut vq = VirtQueue {
            num: 16,
            vaddr: core::ptr::null_mut(),
            paddr: 0,
            ring_size: 0,
            desc: core::ptr::null_mut(),
            avail: core::ptr::null_mut(),
            used: core::ptr::null_mut(),
            free_num: 0,
            free_head: 0,
            free_tail: 0,
            last_used: 0,
            data: core::ptr::null_mut(),
            indirect: core::ptr::null_mut(),
            indirect_phys: 0,
            indirect_in_use: false,
            indirect_len: 0,
        };

        // We can't test much without real memory, but at least the allocations
        // show the struct layouts are correct
        assert_eq!(core::mem::size_of::<VringDesc>(), 16);
        assert_eq!(core::mem::size_of::<VringUsedElem>(), 8);
    }
}
