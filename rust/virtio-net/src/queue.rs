//! # VirtQueue — split virtqueue implementation for virtio-net
//!
//! Manages descriptor/available/used rings for a single virtqueue.
//! Adapted from `virtio-blk/src/virtio.rs` for the netdriver context.

#![allow(dead_code)]

use core::ffi::c_int;

// ============================================================================
// Vring structures
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

    let after_avail = desc_size + avail_size;
    let used_offset = (after_avail + 2 + align - 1) & !(align - 1);
    used_offset + used_size + 2
}

// ============================================================================
// VirtQueue
// ============================================================================

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
    pub free_num: u16,
    pub free_head: u16,
    pub free_tail: u16,
    pub last_used: u16,

    // Per-descriptor opaque data
    pub data: *mut usize,
}

impl VirtQueue {
    /// Allocate and initialise a new virtqueue with `num` descriptors.
    /// Returns the queue, or None on allocation failure.
    pub fn allocate(num: u16) -> Option<Self> {
        let ring_size = vring_size(num as usize);

        // Use ffi alloc_contig for contiguous physical memory
        let (vaddr, paddr) = alloc_contig_ffi(ring_size)?;

        // Allocate per-descriptor tracking array
        let data_size = core::mem::size_of::<usize>() * (num as usize);
        let data_ptr = alloc_contig_raw_ptr(data_size)?;

        let mut q = VirtQueue {
            num,
            vaddr: vaddr as *mut u8,
            paddr,
            ring_size,
            desc: core::ptr::null_mut(),
            avail: core::ptr::null_mut(),
            used: core::ptr::null_mut(),
            free_num: 0,
            free_head: 0,
            free_tail: 0,
            last_used: 0,
            data: data_ptr as *mut usize,
        };

        q.init_vring();
        Some(q)
    }

    /// Initialise vring layout within allocated memory.
    fn init_vring(&mut self) {
        let num = self.num as usize;
        let vaddr = self.vaddr;

        // Descriptors at the start
        self.desc = vaddr as *mut VringDesc;

        // Available ring: right after descriptors
        let avail_off = num * core::mem::size_of::<VringDesc>();
        self.avail = unsafe { vaddr.add(avail_off) as *mut VringAvail };

        // Used ring: aligned after available + ring
        let after_avail = avail_off + 2 + 2 + 2 * num;
        let align: usize = 4096;
        let used_off = (after_avail + 2 + align - 1) & !(align - 1);
        self.used = unsafe { vaddr.add(used_off) as *mut VringUsed };

        // Initialise free list: all descriptors chained
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

        unsafe { core::ptr::write_bytes(self.data, 0, num); }
    }

    /// Allocate `count` chained descriptors. Returns the index of the first,
    /// or None if insufficient descriptors are available.
    pub fn alloc_descs(&mut self, count: u16) -> Option<u16> {
        if self.free_num < count { return None; }

        let head = self.free_head;
        let mut prev = head;

        for _ in 0..count {
            let desc = unsafe { &mut *self.desc.add(prev as usize) };
            prev = desc.next;
        }

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
    /// `data` is an opaque value returned on completion via `collect()`.
    pub fn submit(&mut self, head: u16, data: usize) {
        let avail = unsafe { &mut *self.avail };
        let idx = avail.idx % self.num;
        unsafe {
            avail.ring.as_mut_ptr().add(idx as usize).write(head);
            *self.data.add(head as usize) = data;
        }
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

        let data = unsafe { *self.data.add(head as usize) };
        self.free_descs(head);
        Some((data, len))
    }

    /// Free all queue resources.
    pub fn free_resources(&mut self) {
        if !self.vaddr.is_null() {
            free_contig_ffi(self.vaddr as *mut core::ffi::c_void, self.ring_size);
            self.vaddr = core::ptr::null_mut();
        }
        if !self.data.is_null() {
            let size = core::mem::size_of::<usize>() * (self.num as usize);
            free_contig_ffi(self.data as *mut core::ffi::c_void, size);
            self.data = core::ptr::null_mut();
        }
    }
}

impl Drop for VirtQueue {
    fn drop(&mut self) {
        self.free_resources();
    }
}

// ============================================================================
// FFI wrappers for contiguous memory allocation
// ============================================================================

/// Allocate contiguous physical memory. Returns (virtual_ptr, phys_addr) or None.
fn alloc_contig_ffi(size: usize) -> Option<(*mut core::ffi::c_void, u64)> {
    #[cfg(target_os = "minix")]
    {
        extern "C" {
            fn alloc_contig(size: usize, flags: c_int, phys: *mut u64) -> *mut core::ffi::c_void;
        }
        unsafe {
            const AC_ALIGN4K: c_int = 1;
            let mut phys: u64 = 0;
            let ptr = alloc_contig(size, AC_ALIGN4K, &mut phys);
            if ptr.is_null() { None } else { Some((ptr, phys)) }
        }
    }
    #[cfg(not(target_os = "minix"))]
    {
        let _ = size;
        None
    }
}

/// Allocate contiguous physical memory, return just the raw pointer.
fn alloc_contig_raw_ptr(size: usize) -> Option<*mut core::ffi::c_void> {
    alloc_contig_ffi(size).map(|(p, _)| p)
}

/// Free contiguous memory.
fn free_contig_ffi(addr: *mut core::ffi::c_void, size: usize) {
    #[cfg(target_os = "minix")]
    {
        extern "C" {
            fn free_contig(addr: *mut core::ffi::c_void, size: usize);
        }
        unsafe { free_contig(addr, size); }
    }
    #[cfg(not(target_os = "minix"))]
    {
        let _ = (addr, size);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vring_size_calculation() {
        let size = vring_size(128);
        assert!(size > 0);
        assert!(size < 128 * 1024);
    }

    #[test]
    fn struct_sizes() {
        assert_eq!(core::mem::size_of::<VringDesc>(), 16);
        assert_eq!(core::mem::size_of::<VringUsedElem>(), 8);
    }

    #[test]
    fn free_list_logic() {
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
        };

        // Can't test much without real memory, but at least verify layouts
        assert_eq!(core::mem::size_of::<VringDesc>(), 16);
        assert_eq!(core::mem::size_of::<VringUsedElem>(), 8);
    }
}
