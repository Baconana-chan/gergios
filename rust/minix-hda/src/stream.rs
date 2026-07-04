//! # Stream — Audio DMA Stream Management
//!
//! Implements: buffer descriptor list (BDL) DMA management, position tracking,
//! fragment transfer between DMA and software buffers, interrupt handling for
//! buffer completion events.

#![allow(dead_code)]

use core::ptr;

use crate::ffi;
use crate::registers::{self, BdlEntry, AUDIO_DMA_BUF_SIZE, BDL_ENTRIES};
use crate::registers::fmt as audio_fmt;
use crate::controller::HdaController;

use audio_buf::{RingPos, DmaMode};

/// Fragment size for audio data (matching typical ALSA period size).
pub const FRAGMENT_SIZE: u32 = 8192; // 8KB

/// Number of DMA fragments.
pub const NR_DMA_FRAGMENTS: usize = AUDIO_DMA_BUF_SIZE / FRAGMENT_SIZE as usize;

/// Number of extra (software) buffer fragments.
pub const NR_EXTRA_FRAGMENTS: usize = 4;

/// Audio stream state for a single direction (playback or capture).
pub struct AudioStream {
    /// Stream tag (SDI identifier).
    pub stream_tag: u8,
    /// Whether this is a capture stream.
    pub is_capture: bool,
    /// DMA ring buffer position tracker.
    pub dma_pos: RingPos,
    /// Extra (software) buffer position tracker.
    pub extra_pos: RingPos,
    /// Fragment size in bytes.
    pub frag_size: u32,
    /// DMA buffer physical address.
    pub dma_phys: u64,
    /// DMA buffer virtual address.
    pub dma_virt: *mut u8,
    /// Temporary copy buffer for this stream.
    pub copy_buf: [u8; FRAGMENT_SIZE as usize],
    /// Current DMA mode.
    pub mode: DmaMode,
    /// Number of bytes transferred since last reset.
    pub bytes_transferred: u64,
    /// Whether underrun occurred.
    pub underrun: bool,
    /// Whether overrun occurred (capture).
    pub overrun: bool,
    /// Whether the stream is draining (waiting for playback to finish).
    pub draining: bool,
}

impl AudioStream {
    /// Create a new audio stream.
    pub fn new(stream_tag: u8, is_capture: bool, dma_phys: u64, dma_virt: *mut u8) -> Option<Self> {
        Some(Self {
            stream_tag,
            is_capture,
            dma_pos: RingPos::new(NR_DMA_FRAGMENTS)?,
            extra_pos: RingPos::new(NR_EXTRA_FRAGMENTS)?,
            frag_size: FRAGMENT_SIZE,
            dma_phys,
            dma_virt,
            copy_buf: [0u8; FRAGMENT_SIZE as usize],
            mode: if is_capture { DmaMode::Read } else { DmaMode::Write },
            bytes_transferred: 0,
            underrun: false,
            overrun: false,
            draining: false,
        })
    }

    /// Reset the stream state.
    pub fn reset(&mut self) {
        self.dma_pos.reset();
        self.extra_pos.reset();
        self.bytes_transferred = 0;
        self.underrun = false;
        self.overrun = false;
        self.draining = false;
    }

    /// Write data from user space into the DMA buffer (playback).
    /// `data` is the data slice, `grant` is the MINIX safecopy grant.
    /// Returns number of bytes written.
    pub fn write_user_data(&mut self, grant: ffi::cp_grant_id_t, endpoint: ffi::endpoint_t,
        size: usize) -> isize
    {
        let total = core::cmp::min(size, self.frag_size as usize);

        // Copy from user space to copy buffer
        let r = ffi::sys_safecopyfrom_ffi(
            endpoint, grant, 0,
            self.copy_buf.as_mut_ptr() as *mut core::ffi::c_void,
            total as core::ffi::c_ulong,
        );
        if r != ffi::OK {
            return ffi::EIO as isize;
        }

        // Try to write to DMA or extra buffer
        if !self.dma_pos.is_full() {
            if let Some(idx) = self.dma_pos.advance_fill() {
                let offset = idx * self.frag_size as usize;
                if offset + total <= AUDIO_DMA_BUF_SIZE {
                    unsafe {
                        ptr::copy_nonoverlapping(
                            self.copy_buf.as_ptr(),
                            self.dma_virt.add(offset),
                            total,
                        );
                    }
                }
                self.bytes_transferred += total as u64;
            }
        } else if !self.extra_pos.is_full() {
            if let Some(_idx) = self.extra_pos.advance_fill() {
                // Extra buffer overflow in DMA buffer, spill to software buffer
                self.bytes_transferred += total as u64;
            }
        } else {
            self.underrun = true;
            return 0; // Buffer full, drop data
        }

        total as isize
    }

    /// Read data from DMA buffer to user space (capture).
    /// Returns number of bytes read.
    pub fn read_user_data(&mut self, grant: ffi::cp_grant_id_t, endpoint: ffi::endpoint_t,
        size: usize) -> isize
    {
        let total = core::cmp::min(size, self.frag_size as usize);

        // Try to read from DMA buffer
        if !self.dma_pos.is_empty() {
            if let Some(idx) = self.dma_pos.advance_read() {
                let offset = idx * self.frag_size as usize;
                if offset + total <= AUDIO_DMA_BUF_SIZE {
                    unsafe {
                        ptr::copy_nonoverlapping(
                            self.dma_virt.add(offset),
                            self.copy_buf.as_mut_ptr(),
                            total,
                        );
                    }
                }
                self.bytes_transferred += total as u64;
            }
        } else if !self.extra_pos.is_empty() {
            if let Some(_idx) = self.extra_pos.advance_read() {
                self.bytes_transferred += total as u64;
            }
        } else {
            self.overrun = true;
            return 0; // No data available
        }

        // Copy to user space
        let r = ffi::sys_safecopyto_ffi(
            endpoint, grant, 0,
            self.copy_buf.as_mut_ptr() as *const core::ffi::c_void,
            total as core::ffi::c_ulong,
        );
        if r != ffi::OK {
            return ffi::EIO as isize;
        }

        total as isize
    }

    /// Called from interrupt handler when a buffer completion occurs.
    /// Advances the DMA read/write position.
    pub fn on_buffer_complete(&mut self) {
        // For playback: the DMA engine has consumed one fragment from the DMA buffer.
        // Move one fragment from DMA position to "consumed" (advance read).
        if !self.is_capture {
            if !self.dma_pos.is_empty() {
                self.dma_pos.advance_read();
            }
            // If extra buffer has data, move it to DMA buffer
            audio_buf::try_transfer(&mut self.extra_pos, &mut self.dma_pos);
        } else {
            // For capture: the DMA engine has filled one fragment.
            if !self.dma_pos.is_full() {
                let _ = self.dma_pos.advance_fill();
            }
        }
    }

    /// Fill the initial DMA buffer for playback (before starting).
    /// Called with silence data.
    pub fn fill_dma_initial(&mut self) {
        unsafe {
            ptr::write_bytes(self.dma_virt, 0, AUDIO_DMA_BUF_SIZE);
        }
        // Mark all fragments as filled
        while !self.dma_pos.is_full() {
            let _ = self.dma_pos.advance_fill();
        }
    }

    /// Get the current playback/capture position in bytes.
    pub fn position(&self) -> u64 {
        self.bytes_transferred
    }

    /// Check if the stream can accept more data (for writes).
    pub fn can_accept(&self) -> bool {
        !self.dma_pos.is_full() || !self.extra_pos.is_full()
    }

    /// Check if the stream has data available (for reads).
    pub fn has_data(&self) -> bool {
        !self.dma_pos.is_empty() || !self.extra_pos.is_empty()
    }
}

// ============================================================================
// Stream helper: manage all active audio streams
// ============================================================================

/// Maximum number of concurrent audio streams.
pub const MAX_AUDIO_STREAMS: usize = 4;

/// Global audio stream manager.
pub struct StreamManager {
    /// Active audio streams.
    pub streams: [Option<AudioStream>; MAX_AUDIO_STREAMS],
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            streams: [None, None, None, None],
        }
    }

    /// Find a stream by stream tag.
    pub fn find_by_tag(&mut self, tag: u8) -> Option<&mut AudioStream> {
        for s in self.streams.iter_mut() {
            if let Some(stream) = s.as_mut() {
                if stream.stream_tag == tag {
                    return Some(stream);
                }
            }
        }
        None
    }

    /// Find a free slot.
    pub fn alloc_slot(&mut self) -> Option<&mut Option<AudioStream>> {
        for s in self.streams.iter_mut() {
            if s.is_none() {
                return Some(s);
            }
        }
        None
    }

    /// Free a stream slot by tag.
    pub fn free_by_tag(&mut self, tag: u8) {
        for s in self.streams.iter_mut() {
            if let Some(stream) = s {
                if stream.stream_tag == tag {
                    *s = None;
                    return;
                }
            }
        }
    }

    /// Handle buffer completion interrupts for all active streams.
    pub fn handle_buffer_completions(&mut self, completed_bitmap: u32) {
        for (idx, s) in self.streams.iter_mut().enumerate() {
            if let Some(stream) = s {
                if (completed_bitmap & (1u32 << idx)) != 0 {
                    stream.on_buffer_complete();
                }
            }
        }
    }

    /// Get current DMA position for all streams (for AUDIO_GETIOFFS/GETOOFFS).
    pub fn get_positions(&self, play_offset: &mut u32, rec_offset: &mut u32) {
        for s in self.streams.iter() {
            if let Some(stream) = s.as_ref() {
                if stream.is_capture {
                    *rec_offset = stream.dma_pos.len() as u32 * stream.frag_size;
                } else {
                    *play_offset = stream.dma_pos.len() as u32 * stream.frag_size;
                }
            }
        }
    }
}
