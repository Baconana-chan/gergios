//! # audio-buf — MINIX audio DMA ring buffer management
//!
//! This crate provides safe, `no_std` ring buffer abstractions for audio DMA
//! drivers. It manages fragment tracking, buffer positions, and overflow
//! prevention — replacing the manual ring buffer arithmetic in the C
//! `libaudiodriver`.
//!
//! ## Architecture
//!
//! The audio driver uses a **double DMA buffer** with an optional extra
//! software buffer:
//!
//! ```text
//!   User app  ──→  ExtraBuf  ──→  DmaBuf  ──→  DMA hardware
//!                  (circular)       (circular)
//! ```
//!
//! - `DmaBuf`: DMA buffer, divided into `nr_of_dma_fragments` fragments.
//! - `ExtraBuf`: software buffer, divided into `nr_of_extra_buffers` fragments.
//!
//! Both buffers are circular. When the DMA buffer is full, data spills to
//! ExtraBuf. On interrupt, data moves from ExtraBuf → DmaBuf → hardware.
//!
//! ## Safety
//!
//! This crate contains **zero `unsafe` code**. All buffer bounds are checked
//! at runtime via safe `check_*` methods before any indexing.

#![no_std]
#![deny(unsafe_code)]

use core::fmt;

// ---------------------------------------------------------------------------
// DMA direction
// ---------------------------------------------------------------------------

/// DMA transfer direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaMode {
    /// No DMA transfer active.
    None,
    /// Reading from hardware (capture/record).
    Read,
    /// Writing to hardware (playback).
    Write,
}

// ---------------------------------------------------------------------------
// Ring buffer position tracking
// ---------------------------------------------------------------------------

/// A position cursor in a circular buffer.
///
/// Tracks the read position, fill position, and current length for a
/// circular buffer with a fixed number of fragments.
#[derive(Debug, Clone, Copy)]
pub struct RingPos {
    /// Number of fragments in this buffer.
    nr_fragments: usize,
    /// Index of the next fragment to read.
    read_next: usize,
    /// Index of the next fragment to fill.
    fill_next: usize,
    /// Number of fragments currently occupied.
    length: usize,
}

impl RingPos {
    /// Create a new, empty ring position tracker.
    ///
    /// Returns `None` if `nr_fragments` is 0.
    pub fn new(nr_fragments: usize) -> Option<Self> {
        if nr_fragments == 0 {
            return None;
        }
        Some(Self {
            nr_fragments,
            read_next: 0,
            fill_next: 0,
            length: 0,
        })
    }

    /// Reset the ring to empty state.
    pub fn reset(&mut self) {
        self.read_next = 0;
        self.fill_next = 0;
        self.length = 0;
    }

    /// Returns `true` if the ring is empty (no data available).
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// Returns `true` if the ring is full (no space available).
    pub fn is_full(&self) -> bool {
        self.length == self.nr_fragments
    }

    /// Current number of occupied fragments.
    pub fn len(&self) -> usize {
        self.length
    }

    /// Total capacity in fragments.
    pub fn capacity(&self) -> usize {
        self.nr_fragments
    }

    /// Available space for writing (fragments).
    pub fn available(&self) -> usize {
        self.nr_fragments - self.length
    }

    /// Advance the fill (write) position by one fragment.
    ///
    /// Returns the index of the fragment to fill.
    /// Returns `None` if the ring is full.
    pub fn advance_fill(&mut self) -> Option<usize> {
        if self.length >= self.nr_fragments {
            return None;
        }
        let idx = self.fill_next;
        self.fill_next = (self.fill_next + 1) % self.nr_fragments;
        self.length += 1;
        Some(idx)
    }

    /// Advance the read position by one fragment.
    ///
    /// Returns the index of the fragment to read.
    /// Returns `None` if the ring is empty.
    pub fn advance_read(&mut self) -> Option<usize> {
        if self.length == 0 {
            return None;
        }
        let idx = self.read_next;
        self.read_next = (self.read_next + 1) % self.nr_fragments;
        self.length -= 1;
        Some(idx)
    }

    /// Peek at the current read index without consuming it.
    pub fn peek_read(&self) -> Option<usize> {
        if self.length == 0 {
            None
        } else {
            Some(self.read_next)
        }
    }

    /// Peek at the current fill (write) index.
    pub fn peek_fill(&self) -> usize {
        self.fill_next
    }

    /// Current read index.
    pub fn read_idx(&self) -> usize {
        self.read_next
    }

    /// Current fill index.
    pub fn fill_idx(&self) -> usize {
        self.fill_next
    }
}

// ---------------------------------------------------------------------------
// DMA buffer state
// ---------------------------------------------------------------------------

/// Complete state of a DMA sub-device buffer pair.
///
/// Models the `sub_dev_t` structure from `libaudiodriver` but with safe
/// Rust abstractions rather than raw C fields.
#[derive(Debug, Clone)]
pub struct DmaBuffer {
    /// DMA ring buffer position.
    pub dma: RingPos,
    /// Extra (software) ring buffer position.
    pub extra: RingPos,
    /// Current DMA mode.
    pub mode: DmaMode,
    /// Fragment size in bytes.
    pub frag_size: u32,
    /// Whether the device is busy with DMA transfer.
    pub busy: bool,
    /// Whether all buffers are empty (no more data to transfer).
    pub out_of_data: bool,
}

impl DmaBuffer {
    /// Create a new DMA buffer state.
    ///
    /// Returns `None` if either `nr_dma` or `nr_extra` is 0.
    pub fn new(nr_dma: usize, nr_extra: usize, frag_size: u32) -> Option<Self> {
        Some(Self {
            dma: RingPos::new(nr_dma)?,
            extra: RingPos::new(nr_extra)?,
            mode: DmaMode::None,
            frag_size,
            busy: false,
            out_of_data: true,
        })
    }

    /// Reset all buffer state.
    pub fn reset(&mut self) {
        self.dma.reset();
        self.extra.reset();
        self.mode = DmaMode::None;
        self.busy = false;
        self.out_of_data = true;
    }

    /// Returns `true` if there is space to accept new data from user space.
    /// Space exists if either DMA or extra buffer has room.
    pub fn can_accept_data(&self) -> bool {
        !self.dma.is_full() || !self.extra.is_full()
    }

    /// Returns `true` if there is data available for the user (for reads).
    pub fn has_data(&self) -> bool {
        !self.dma.is_empty() || !self.extra.is_empty()
    }

    /// Total fragments in flight (DMA + extra).
    pub fn total_fragments(&self) -> usize {
        self.dma.len() + self.extra.len()
    }
}

// ---------------------------------------------------------------------------
// Fragment transfer helpers
// ---------------------------------------------------------------------------

/// Attempt to transfer a fragment, only committing if both sides are ready.
///
/// This is the safe, transactional version:
/// 1. Check that `src` is not empty and `dst` is not full.
/// 2. Advance both positions.
/// 3. Return the source and destination indices.
///
/// Returns `None` if either condition fails.
pub fn try_transfer(src: &mut RingPos, dst: &mut RingPos) -> Option<(usize, usize)> {
    if src.is_empty() || dst.is_full() {
        return None;
    }
    let src_idx = src.advance_read()?;
    let dst_idx = dst.advance_fill()?;
    Some((src_idx, dst_idx))
}

// ---------------------------------------------------------------------------
// Fragment size validation
// ---------------------------------------------------------------------------

/// Check that a fragment size is valid for the given buffer parameters.
///
/// A valid fragment size:
/// - Is non-zero
/// - Divides evenly into `dma_size` (the total DMA buffer size)
/// - Is not larger than `dma_size`
pub fn validate_fragment_size(frag_size: u32, dma_size: u32) -> bool {
    if frag_size == 0 || dma_size == 0 {
        return false;
    }
    if frag_size > dma_size {
        return false;
    }
    dma_size % frag_size == 0
}

/// Calculate the number of DMA fragments from the buffer and fragment sizes.
pub fn nr_dma_fragments(frag_size: u32, dma_size: u32) -> Option<usize> {
    if validate_fragment_size(frag_size, dma_size) {
        Some((dma_size / frag_size) as usize)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during buffer operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferError {
    /// Buffer is full (no space to write).
    Full,
    /// Buffer is empty (no data to read).
    Empty,
    /// Invalid fragment size (does not divide evenly).
    InvalidFragmentSize,
    /// Invalid buffer capacity (must be > 0).
    InvalidCapacity,
    /// Out of bounds access.
    OutOfBounds,
}

impl fmt::Display for BufferError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BufferError::Full => write!(f, "buffer full"),
            BufferError::Empty => write!(f, "buffer empty"),
            BufferError::InvalidFragmentSize => write!(f, "invalid fragment size"),
            BufferError::InvalidCapacity => write!(f, "invalid buffer capacity"),
            BufferError::OutOfBounds => write!(f, "out of bounds access"),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_pos_creation() {
        let pos = RingPos::new(8).unwrap();
        assert_eq!(pos.capacity(), 8);
        assert!(pos.is_empty());
        assert!(!pos.is_full());
        assert_eq!(pos.len(), 0);
        assert_eq!(pos.available(), 8);
    }

    #[test]
    fn ring_pos_zero_capacity() {
        assert!(RingPos::new(0).is_none());
    }

    #[test]
    fn ring_pos_advance_fill_and_read() {
        let mut pos = RingPos::new(4).unwrap();
        assert_eq!(pos.advance_fill(), Some(0));
        assert_eq!(pos.advance_fill(), Some(1));
        assert_eq!(pos.len(), 2);
        assert!(!pos.is_empty());
        assert!(!pos.is_full());
        assert_eq!(pos.advance_read(), Some(0));
        assert_eq!(pos.len(), 1);
        assert_eq!(pos.advance_read(), Some(1));
        assert!(pos.is_empty());
    }

    #[test]
    fn ring_pos_full() {
        let mut pos = RingPos::new(3).unwrap();
        assert_eq!(pos.advance_fill(), Some(0));
        assert_eq!(pos.advance_fill(), Some(1));
        assert_eq!(pos.advance_fill(), Some(2));
        assert!(pos.is_full());
        assert_eq!(pos.advance_fill(), None);
    }

    #[test]
    fn ring_pos_empty_read() {
        let mut pos = RingPos::new(4).unwrap();
        assert_eq!(pos.advance_read(), None);
    }

    #[test]
    fn ring_pos_wraparound() {
        let mut pos = RingPos::new(3).unwrap();
        // Fill 3, read 3 → ring is empty, positions should wrap
        pos.advance_fill(); pos.advance_fill(); pos.advance_fill();
        pos.advance_read(); pos.advance_read(); pos.advance_read();
        assert!(pos.is_empty());
        assert_eq!(pos.read_idx(), 0);
        assert_eq!(pos.fill_idx(), 0);
        assert_eq!(pos.advance_fill(), Some(0)); // wraps around
    }

    #[test]
    fn transfer_normal() {
        let mut src = RingPos::new(4).unwrap();
        src.advance_fill(); // put one fragment in src
        let mut dst = RingPos::new(4).unwrap();
        let result = try_transfer(&mut src, &mut dst);
        assert_eq!(result, Some((0, 0)));
        assert!(src.is_empty());
        assert_eq!(dst.len(), 1);
    }

    #[test]
    fn transfer_empty_src() {
        let mut src = RingPos::new(4).unwrap();
        let mut dst = RingPos::new(4).unwrap();
        assert_eq!(try_transfer(&mut src, &mut dst), None);
    }

    #[test]
    fn transfer_full_dst() {
        let mut src = RingPos::new(4).unwrap();
        src.advance_fill();
        let mut dst = RingPos::new(1).unwrap();
        dst.advance_fill(); // dst is full
        assert_eq!(try_transfer(&mut src, &mut dst), None);
        // src should be unchanged
        assert_eq!(src.len(), 1);
    }

    #[test]
    fn dma_buffer_creation() {
        let buf = DmaBuffer::new(8, 4, 256).unwrap();
        assert_eq!(buf.dma.capacity(), 8);
        assert_eq!(buf.extra.capacity(), 4);
        assert_eq!(buf.frag_size, 256);
        assert_eq!(buf.mode, DmaMode::None);
        assert!(!buf.busy);
        assert!(buf.out_of_data);
    }

    #[test]
    fn validate_fragment_size_ok() {
        assert!(validate_fragment_size(256, 8192));
        assert!(validate_fragment_size(1024, 8192));
        assert!(validate_fragment_size(8192, 8192));
    }

    #[test]
    fn validate_fragment_size_invalid() {
        assert!(!validate_fragment_size(0, 8192));
        assert!(!validate_fragment_size(256, 0));
        assert!(!validate_fragment_size(300, 8192)); // not divisible
        assert!(!validate_fragment_size(16384, 8192)); // larger than dma
    }

    #[test]
    fn dma_buffer_can_accept() {
        let mut buf = DmaBuffer::new(2, 2, 256).unwrap();
        assert!(buf.can_accept_data());
        buf.dma.advance_fill();
        buf.dma.advance_fill(); // dma full
        assert!(buf.can_accept_data()); // extra still has room
        buf.extra.advance_fill();
        buf.extra.advance_fill(); // both full
        assert!(!buf.can_accept_data());
    }



    // Helper to test Display without std::format!
    struct TestBuf<const N: usize>([u8; N], usize);
    impl<const N: usize> TestBuf<N> {
        fn new() -> Self { Self([0u8; N], 0) }
        fn as_str(&self) -> &str { core::str::from_utf8(&self.0[..self.1]).unwrap() }
    }
    impl<const N: usize> core::fmt::Write for TestBuf<N> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let b = s.as_bytes();
            if self.1 + b.len() > N { return Err(core::fmt::Error); }
            self.0[self.1..self.1 + b.len()].copy_from_slice(b);
            self.1 += b.len();
            Ok(())
        }
    }

    #[test]
    fn error_display() {
        use core::fmt::Write;
        let mut buf = TestBuf::<64>::new();
        write!(buf, "{}", BufferError::Full).unwrap();
        assert_eq!(buf.as_str(), "buffer full");
        let mut buf = TestBuf::<64>::new();
        write!(buf, "{}", BufferError::Empty).unwrap();
        assert_eq!(buf.as_str(), "buffer empty");
    }
    

}
