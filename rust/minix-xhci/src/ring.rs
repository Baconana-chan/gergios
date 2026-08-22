//! # Ring — TRB Ring Management for xHCI
//!
//! Manages three types of rings:
//! - **Command Ring** — submit commands to the xHC (one per controller)
//! - **Event Ring** — receive events from the xHC (one per interrupter)
//! - **Transfer Ring** — submit transfer TRBs for a specific endpoint

use crate::ffi;
use crate::registers::{Trb, ErstEntry};
use core::ffi::c_void;
use core::ptr;

/// Default number of TRBs in a ring (must be power of 2).
pub const RING_SIZE: usize = 256;

/// Default number of Event Ring segments.
pub const EVENT_RING_SEGMENTS: usize = 1;

/// Default Event Ring segment size (number of TRBs).
pub const EVENT_RING_SEG_SIZE: usize = 256;

/// DMA-mapped ring buffer containing TRBs.
///
/// Note: RingMem intentionally does NOT derive Clone.
/// DMA buffer ownership must be unique — cloning would create
/// dangling pointers if the clone is freed separately.
pub struct RingMem {
    /// Virtual address of the ring buffer.
    pub virt: *mut u8,
    /// Physical address of the ring buffer.
    pub phys: u64,
    /// Size of the ring buffer in bytes.
    pub size: usize,
}

impl RingMem {
    pub fn zeroed() -> Self {
        Self { virt: ptr::null_mut(), phys: 0, size: 0 }
    }

    /// Allocate a DMA ring buffer.
    pub fn alloc(num_trbs: usize) -> Option<Self> {
        let size = num_trbs * core::mem::size_of::<Trb>();
        let (virt, phys) = ffi::alloc_contig_ffi(size)?;
        unsafe { ptr::write_bytes(virt, 0, size); }
        Some(Self { virt: virt as *mut u8, phys, size })
    }

    /// Free the DMA ring buffer.
    pub fn free(&mut self) {
        if !self.virt.is_null() {
            ffi::free_contig_ffi(self.virt as *mut core::ffi::c_void, self.size);
        }
        *self = Self::zeroed();
    }

    /// Get a mutable pointer to a TRB at given index.
    pub fn trb_mut(&mut self, index: usize) -> *mut Trb {
        (self.virt as *mut Trb).wrapping_add(index)
    }

    /// Get a const pointer to a TRB at given index.
    pub fn trb(&self, index: usize) -> *const Trb {
        (self.virt as *const Trb).wrapping_add(index)
    }

    /// Get the number of TRBs in this ring.
    pub fn num_trbs(&self) -> usize {
        self.size / core::mem::size_of::<Trb>()
    }
}

// ============================================================================
// Generic Ring (used for Command and Transfer rings)
// ============================================================================

/// A generic producer-consumer ring for TRBs.
/// Producer: driver writes TRBs (advances enqueue ptr).
/// Consumer: xHC reads TRBs (advances dequeue ptr).
pub struct TrbRing {
    /// DMA buffer for the ring.
    pub mem: RingMem,
    /// Enqueue pointer index (where driver writes next TRB).
    pub enqueue_idx: usize,
    /// Dequeue pointer index (where xHC has consumed up to).
    pub dequeue_idx: usize,
    /// Current cycle bit (toggled on wrap).
    pub cycle: bool,
    /// Number of TRBs.
    pub num_trbs: usize,
    /// Physical address of the last TRB (for Link TRB target).
    pub last_trb_phys: u64,
}

impl TrbRing {
    /// Create a new TRB ring.
    pub fn new(num_trbs: usize, initial_cycle: bool) -> Option<Self> {
        let mem = RingMem::alloc(num_trbs)?;
        let last_phys = mem.phys + ((num_trbs - 1) * core::mem::size_of::<Trb>()) as u64;
        Some(Self {
            mem,
            enqueue_idx: 0,
            dequeue_idx: 0,
            cycle: initial_cycle,
            num_trbs,
            last_trb_phys: last_phys,
        })
    }

    /// Get the physical address of the ring (for programming CRCR or EP context).
    pub fn phys(&self) -> u64 {
        self.mem.phys
    }

    /// Get the current enqueue pointer physical address.
    pub fn enqueue_phys(&self) -> u64 {
        self.mem.phys + (self.enqueue_idx * core::mem::size_of::<Trb>()) as u64
    }

    /// Get the current dequeue pointer physical address.
    pub fn dequeue_phys(&self) -> u64 {
        self.mem.phys + (self.dequeue_idx * core::mem::size_of::<Trb>()) as u64
    }

    /// Check if ring is full (no room for more TRBs).
    pub fn is_full(&self) -> bool {
        ((self.enqueue_idx + 1) % self.num_trbs) == self.dequeue_idx
    }

    /// Check if ring is empty.
    pub fn is_empty(&self) -> bool {
        self.enqueue_idx == self.dequeue_idx
    }

    /// Reserve a slot in the ring and return a pointer to write the TRB.
    /// Returns None if full.
    pub fn reserve(&mut self) -> Option<*mut Trb> {
        if self.is_full() {
            return None;
        }
        let idx = self.enqueue_idx;
        let next_idx = (idx + 1) % self.num_trbs;

        // If we're about to wrap, write a Link TRB at the last position
        if next_idx == 0 {
            // Write Link TRB at last position
            let last_idx = self.num_trbs - 1;
            let link_trb = self.build_link_trb();
            unsafe {
                ptr::write_volatile(self.mem.trb_mut(last_idx), link_trb);
            }
        }

        self.enqueue_idx = next_idx;
        Some(self.mem.trb_mut(idx))
    }

    /// Commit a TRB that was written via `reserve()`.
    /// Performs a write barrier so the xHC sees the new TRB.
    pub fn commit(&self) {
        core::sync::atomic::fence(core::sync::atomic::Ordering::Release);
        // Ring the doorbell is done separately by the caller
    }

    /// Build a Link TRB for ring linking.
    fn build_link_trb(&self) -> Trb {
        let next_phys = self.mem.phys;
        // Toggle cycle bit on wrap (TC = 1)
        crate::registers::build_link_trb(self.cycle, next_phys, true, false)
    }

    /// Advance the dequeue pointer (called when xHC consumes TRBs).
    pub fn advance_dequeue(&mut self, count: usize) {
        let mut idx = self.dequeue_idx;
        for _ in 0..count {
            idx = (idx + 1) % self.num_trbs;
            if idx == 0 {
                // Wrapped — toggle cycle (handled by Link TRB TC bit)
                self.cycle = !self.cycle;
            }
        }
        self.dequeue_idx = idx;
    }

    /// Free the ring's DMA memory.
    pub fn free(&mut self) {
        self.mem.free();
        self.enqueue_idx = 0;
        self.dequeue_idx = 0;
        self.cycle = false;
    }
}

// ============================================================================
// Event Ring (consumer-only: xHC writes, driver reads)
// ============================================================================

/// Event Ring — the xHC writes event TRBs, the driver consumes them.
pub struct EventRing {
    /// DMA buffer for the event ring segment.
    pub seg_mem: RingMem,
    /// ERST entry (written to controller).
    pub erst_entry: ErstEntry,
    /// ERST entry physical address (for programming ERSTBA).
    pub erst_entry_phys: u64,
    /// ERST entry virtual address (for free).
    pub erst_virt: *mut c_void,
    /// Dequeue pointer index (where the driver has consumed up to).
    pub dequeue_idx: usize,
    /// Current cycle bit (from xHC — toggled by controller on wrap).
    pub cycle: bool,
    /// Number of TRBs in the segment.
    pub num_trbs: usize,
}

impl EventRing {
    /// Create a new Event Ring with a single segment.
    pub fn new(seg_size: usize) -> Option<Self> {
        let seg_mem = RingMem::alloc(seg_size)?;

        // Allocate ERST entry (needs to be DMA-able for the controller to read it)
        let (erst_virt, erst_phys) = ffi::alloc_contig_ffi(core::mem::size_of::<ErstEntry>())?;
        unsafe { ptr::write_bytes(erst_virt, 0, core::mem::size_of::<ErstEntry>()); }

        let mut erst_entry = ErstEntry::zeroed();
        erst_entry.set_base_addr(seg_mem.phys);
        erst_entry.seg_size = seg_size as u16;

        // Write the ERST entry to DMA memory (controller reads from here)
        unsafe {
            ptr::write_volatile(erst_virt as *mut ErstEntry, erst_entry);
        }

        Some(Self {
            seg_mem,
            erst_entry,
            erst_entry_phys: erst_phys,
            erst_virt,
            dequeue_idx: 0,
            cycle: true, // Initial cycle = 1 (per xHCI spec)
            num_trbs: seg_size,
        })
    }

    /// Get a pointer to a TRB at given index.
    pub fn trb(&self, index: usize) -> *const Trb {
        self.seg_mem.trb(index)
    }

    /// Get the physical address of the ERST entry.
    pub fn erst_phys(&self) -> u64 {
        self.erst_entry_phys
    }

    /// Get the ERST base address to program into ERSTBA reg.
    pub fn erstba(&self) -> u64 {
        self.erst_entry_phys
    }

    /// Get the ERST segment size (number of entries in the table).
    pub fn erst_size(&self) -> u32 {
        1 // Single segment
    }

    /// Read the next event TRB (if available).
    /// Returns Some(Trb) if there's a new event, None otherwise.
    pub fn next_event(&mut self) -> Option<Trb> {
        let idx = self.dequeue_idx;
        let trb = unsafe { &*self.seg_mem.trb(idx) };

        // Check if this TRB has the expected cycle bit
        if trb.cycle() != self.cycle {
            return None; // No new events
        }

        let event = unsafe { ptr::read_volatile(trb) };

        // Advance dequeue pointer
        let next_idx = (idx + 1) % self.num_trbs;
        self.dequeue_idx = next_idx;

        // If we wrapped, the controller toggles the cycle bit
        if next_idx == 0 {
            self.cycle = !self.cycle;
        }

        Some(event)
    }

    /// Get the current dequeue pointer (for writing ERDP register).
    pub fn dequeue_phys(&self) -> u64 {
        self.seg_mem.phys + (self.dequeue_idx * core::mem::size_of::<Trb>()) as u64
    }

    /// Free all DMA memory.
    pub fn free(&mut self) {
        self.seg_mem.free();
        if !self.erst_virt.is_null() {
            ffi::free_contig_ffi(self.erst_virt, core::mem::size_of::<ErstEntry>());
        }
        *self = Self {
            seg_mem: RingMem::zeroed(),
            erst_entry: ErstEntry::zeroed(),
            erst_entry_phys: 0,
            erst_virt: core::ptr::null_mut(),
            dequeue_idx: 0,
            cycle: true,
            num_trbs: 0,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_alloc_free() {
        let mut ring = RingMem::alloc(256);
        assert!(ring.is_some());
        ring.as_mut().unwrap().free();
    }

    #[test]
    fn trb_ring_basic() {
        let mut ring = TrbRing::new(64, true).unwrap();
        assert!(!ring.is_full());
        assert!(ring.is_empty());

        let slot = ring.reserve();
        assert!(slot.is_some());
        assert!(!ring.is_full());
        assert!(!ring.is_empty());
    }

    #[test]
    fn event_ring_basic() {
        let er = EventRing::new(256);
        assert!(er.is_some());
    }

    #[test]
    fn ring_wrap() {
        let mut ring = TrbRing::new(8, true).unwrap();
        // Fill ring. With dequeue parked at 0 the ring accepts num_trbs - 1
        // = 7 TRBs; the last slot stays empty so enqueue never meets dequeue.
        for i in 0..7 {
            let slot = ring.reserve();
            assert!(slot.is_some(), "Failed at index {}", i);
        }
        let slot = ring.reserve();
        assert!(slot.is_none(), "8th reserve should fail (ring full)");
    }
}
