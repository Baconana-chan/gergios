//! # MemBackend — In-memory display backend
//!
//! Stores rendered frames in an internal buffer that can be copied out
//! or exported as PNG. Useful for testing and host-side development
//! without a physical MINIX framebuffer.

use crate::pixel_buffer::PixelBuffer;
use crate::backend::Backend;

/// An in-memory display backend.
///
/// Stores the latest presented frame in `frame`. The frame can be read
/// via `as_bytes()` or exported as PNG (with the `png` feature).
pub struct MemBackend {
    pub width: u32,
    pub height: u32,
    frame: PixelBuffer,
    frame_count: u64,
}

impl MemBackend {
    /// Create a new in-memory backend with the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            frame: PixelBuffer::new(width, height),
            frame_count: 0,
        }
    }

    /// Get the latest frame as raw RGBA bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.frame.as_bytes()
    }

    /// Get the total number of frames presented so far.
    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    /// Export the current frame as a PNG file (requires `png` feature).
    #[cfg(feature = "png")]
    pub fn save_png(&self, path: &str) -> Result<(), Box<dyn core::fmt::Debug>> {
        use image::ImageBuffer;
        let img = ImageBuffer::from_raw(self.width, self.height, self.frame.as_bytes().to_vec())
            .ok_or("failed to create image buffer")?;
        img.save(path).map_err(|e| Box::new(e) as Box<dyn core::fmt::Debug>)
    }
}

impl Backend for MemBackend {
    fn present(&mut self, buffer: &PixelBuffer) {
        let w = self.width.min(buffer.width);
        let h = self.height.min(buffer.height);

        // Copy the buffer into our frame
        self.frame.blit_from(buffer, 0, 0, 0, 0, w, h, false);
        self.frame_count += 1;
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mem_backend_stores_frame() {
        let mut backend = MemBackend::new(10, 10);
        let mut buf = PixelBuffer::new_filled(10, 10, [0xFF, 0x00, 0x00, 0xFF]);

        backend.present(&buf);
        assert_eq!(backend.frame_count(), 1);

        // Check that the frame stored the red pixels
        assert_eq!(backend.frame.get_pixel(0, 0), [0xFF, 0x00, 0x00, 0xFF]);
    }

    #[test]
    fn mem_backend_clips_larger_buffer() {
        let mut backend = MemBackend::new(5, 5);
        let mut buf = PixelBuffer::new_filled(10, 10, [0xFF, 0x00, 0x00, 0xFF]);

        backend.present(&buf);
        assert_eq!(backend.frame.get_pixel(4, 4), [0xFF, 0x00, 0x00, 0xFF]);
    }
}
