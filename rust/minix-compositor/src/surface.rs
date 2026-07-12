//! # Surface — A drawable region in the compositor
//!
//! A surface represents a rectangular region with an attached pixel buffer,
//! position, z-order, and dirty tracking. Surfaces are the building blocks
//! of the compositor — each window (or window decoration, or cursor, etc.)
//! is represented as a surface.

use core::sync::atomic::{AtomicU64, Ordering};

use crate::pixel_buffer::PixelBuffer;

/// A drawable region managed by the compositor.
///
/// Each surface has:
/// - A pixel buffer (the content to draw)
/// - A position on the output (x, y)
/// - A z-order (higher = on top)
/// - Opacity for the entire surface
/// - A dirty flag (set when content changes, cleared after compositing)
pub struct Surface {
    /// Unique identifier for this surface.
    pub id: u64,
    /// The pixel buffer containing the surface's content.
    pub buffer: PixelBuffer,
    /// Horizontal position on the output (in pixels).
    pub x: i32,
    /// Vertical position on the output (in pixels).
    pub y: i32,
    /// Z-order: higher values are composited on top.
    pub z_order: i32,
    /// Overall surface opacity (0 = transparent, 255 = opaque).
    pub opacity: u8,
    /// Whether the surface content has changed since last composite.
    pub dirty: bool,
    /// Whether this surface is visible.
    pub visible: bool,
}

/// Next available surface ID (monotonically increasing).
static NEXT_SURFACE_ID: AtomicU64 = AtomicU64::new(1);

impl Surface {
    /// Create a new surface with the given dimensions and position.
    pub fn new(width: u32, height: u32, x: i32, y: i32) -> Self {
        let id = NEXT_SURFACE_ID.fetch_add(1, Ordering::Relaxed);

        Self {
            id,
            buffer: PixelBuffer::new(width, height),
            x,
            y,
            z_order: 0,
            opacity: 255,
            dirty: true,
            visible: true,
        }
    }

    /// Create a new surface filled with a color.
    pub fn new_filled(width: u32, height: u32, x: i32, y: i32, color: [u8; 4]) -> Self {
        let mut surface = Self::new(width, height, x, y);
        surface.buffer.clear(color);
        surface
    }

    /// Mark the surface as needing re-compositing.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Get the right edge of this surface.
    pub fn right(&self) -> i32 {
        self.x + self.buffer.width as i32
    }

    /// Get the bottom edge of this surface.
    pub fn bottom(&self) -> i32 {
        self.y + self.buffer.height as i32
    }

    /// Check if this surface intersects a given rectangle.
    pub fn intersects(&self, x: i32, y: i32, w: u32, h: u32) -> bool {
        let r = x + w as i32;
        let b = y + h as i32;
        self.x < r && self.right() > x && self.y < b && self.bottom() > y
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_creation() {
        let s = Surface::new_filled(100, 50, 10, 20, [0xFF, 0x00, 0x00, 0xFF]);
        assert!(s.id > 0);
        assert_eq!(s.buffer.width, 100);
        assert_eq!(s.buffer.height, 50);
        assert_eq!(s.x, 10);
        assert_eq!(s.y, 20);
        assert!(s.dirty);
        assert!(s.visible);
    }

    #[test]
    fn surface_intersects() {
        let s = Surface::new(100, 100, 0, 0);
        assert!(s.intersects(50, 50, 10, 10));
        assert!(!s.intersects(200, 200, 10, 10));
        assert!(s.intersects(0, 0, 100, 100)); // exactly overlaps
        assert!(s.intersects(99, 99, 1, 1));    // last pixel
        assert!(!s.intersects(100, 100, 1, 1)); // just past the edge
    }
}
