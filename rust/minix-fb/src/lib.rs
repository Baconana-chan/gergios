//! # minix-fb — Framebuffer backend for MINIX
//!
//! Provides framebuffer detection and display output for the MINIX
//! compositor (`minix-compositor`). Supports:
//!
//! - **SoftwareFb**: In-memory framebuffer that works on any system.
//!   Useful for development, testing, and as a fallback when no
//!   hardware framebuffer driver is available.
//! - **VesaFb**: Hardware framebuffer backed by the VESA/UEFI GOP
//!   linear framebuffer set up by the bootloader. Not yet implemented.
//!
//! Each framebuffer implements the `Framebuffer` trait and can be
//! plugged into `minix_compositor::Compositor` as a `Backend`.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;

/// Information about a framebuffer: resolution, format, layout.
#[derive(Debug, Clone)]
pub struct FbInfo {
    /// Visible width in pixels.
    pub width: u32,
    /// Visible height in pixels.
    pub height: u32,
    /// Bytes per scanline (may be larger than width * bytes_per_pixel).
    pub stride: u32,
    /// Bits per pixel (typically 32 for RGBA/XRGB).
    pub bpp: u8,
    /// Red channel bit offset within a pixel.
    pub red_offset: u8,
    /// Green channel bit offset within a pixel.
    pub green_offset: u8,
    /// Blue channel bit offset within a pixel.
    pub blue_offset: u8,
    /// Alpha channel bit offset (0 if no alpha).
    pub alpha_offset: u8,
}

impl FbInfo {
    /// Create a default FbInfo for 32-bit RGBA (common framebuffer format).
    pub const fn rgba32(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            stride: width * 4,
            bpp: 32,
            red_offset: 0,
            green_offset: 8,
            blue_offset: 16,
            alpha_offset: 24,
        }
    }

    /// Convert an RGBA pixel `[r, g, b, a]` to the native framebuffer
    /// pixel format as `u32`.
    pub fn pack_rgba(&self, rgba: [u8; 4]) -> u32 {
        let r = rgba[0] as u32;
        let g = rgba[1] as u32;
        let b = rgba[2] as u32;
        let a = rgba[3] as u32;

        (r << self.red_offset)
            | (g << self.green_offset)
            | (b << self.blue_offset)
            | (a << self.alpha_offset)
    }
}

/// Trait for framebuffer backends.
///
/// Provides methods to query display parameters, write pixels, and
/// present a rendered frame.
pub trait Framebuffer {
    /// Get information about this framebuffer (resolution, format, etc.).
    fn info(&self) -> &FbInfo;

    /// Write a single RGBA pixel at `(x, y)`.
    fn write_pixel(&mut self, x: u32, y: u32, pixel: [u8; 4]);

    /// Fill a rectangular region with a solid RGBA color.
    fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: [u8; 4]);

    /// Blit raw RGBA pixel data into the framebuffer at `(x, y)`.
    /// `data` is `[R, G, B, A, R, G, B, A, ...]` with `pitch` bytes per row.
    fn blit(&mut self, x: u32, y: u32, w: u32, h: u32, data: &[u8], pitch: u32);

    /// Present the current framebuffer contents to the display.
    ///
    /// For a hardware framebuffer, this may be a no-op (pixels are
    /// already visible). For a software framebuffer, this may trigger
    /// a copy to the actual display hardware.
    fn present(&mut self);

    /// Clear the entire framebuffer with a solid color.
    fn clear(&mut self, color: [u8; 4]) {
        self.fill_rect(0, 0, self.info().width, self.info().height, color);
    }
}

// ── SoftwareFb ──────────────────────────────────────────────────────────

/// In-memory software framebuffer.
///
/// Stores pixel data in a `Vec<u8>` in RGBA32 format. Useful for
/// development, testing, and as a fallback when no hardware framebuffer
/// is available.
pub struct SoftwareFb {
    info: FbInfo,
    data: Vec<u8>,
}

impl SoftwareFb {
    /// Create a new software framebuffer with the given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        let info = FbInfo::rgba32(width, height);
        let size = (info.stride * height) as usize;
        Self {
            info,
            data: alloc::vec![0u8; size],
        }
    }

    /// Get a reference to the raw pixel data.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get a mutable reference to the raw pixel data.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }
}

impl Framebuffer for SoftwareFb {
    fn info(&self) -> &FbInfo {
        &self.info
    }

    fn write_pixel(&mut self, x: u32, y: u32, pixel: [u8; 4]) {
        if x >= self.info.width || y >= self.info.height {
            return;
        }
        let idx = (y * self.info.stride + x * 4) as usize;
        self.data[idx] = pixel[0];
        self.data[idx + 1] = pixel[1];
        self.data[idx + 2] = pixel[2];
        self.data[idx + 3] = pixel[3];
    }

    fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: [u8; 4]) {
        let x_end = core::cmp::min(x + w, self.info.width);
        let y_end = core::cmp::min(y + h, self.info.height);

        for py in y..y_end {
            for px in x..x_end {
                let idx = (py * self.info.stride + px * 4) as usize;
                self.data[idx] = color[0];
                self.data[idx + 1] = color[1];
                self.data[idx + 2] = color[2];
                self.data[idx + 3] = color[3];
            }
        }
    }

    fn blit(&mut self, x: u32, y: u32, w: u32, h: u32, data: &[u8], pitch: u32) {
        let copy_w = core::cmp::min(w, self.info.width - x);
        let copy_h = core::cmp::min(h, self.info.height - y);

        for dy in 0..copy_h {
            let src_off = (dy * pitch) as usize;
            let dst_off = ((y + dy) * self.info.stride + x * 4) as usize;
            let len = (copy_w * 4) as usize;
            if src_off + len <= data.len() && dst_off + len <= self.data.len() {
                self.data[dst_off..dst_off + len]
                    .copy_from_slice(&data[src_off..src_off + len]);
            }
        }
    }

    fn present(&mut self) {
        // No-op: software framebuffer is always up to date.
    }
}

// ── Backend implementation for minix-compositor ─────────────────────────

/// Wraps any `Framebuffer` as a `minix_compositor::backend::Backend`.
///
/// This allows using a `SoftwareFb` (or future `VesaFb`) directly
/// with the compositor's `composite()` method.
pub struct FbBackend<F: Framebuffer> {
    fb: F,
}

impl<F: Framebuffer> FbBackend<F> {
    /// Create a new backend wrapping the given framebuffer.
    pub fn new(fb: F) -> Self {
        Self { fb }
    }

    /// Get a mutable reference to the underlying framebuffer.
    pub fn framebuffer_mut(&mut self) -> &mut F {
        &mut self.fb
    }
}

impl<F: Framebuffer> minix_compositor::backend::Backend for FbBackend<F> {
    fn present(&mut self, buffer: &minix_compositor::pixel_buffer::PixelBuffer) {
        // Blit the compositor's output buffer into the framebuffer
        let w = core::cmp::min(buffer.width, self.fb.info().width);
        let h = core::cmp::min(buffer.height, self.fb.info().height);

        self.fb.blit(0, 0, w, h, buffer.as_bytes(), buffer.stride);
        self.fb.present();
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.fb.info().width, self.fb.info().height)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_fb_creation() {
        let fb = SoftwareFb::new(800, 600);
        assert_eq!(fb.info().width, 800);
        assert_eq!(fb.info().height, 600);
        assert_eq!(fb.info().stride, 800 * 4);
    }

    #[test]
    fn write_and_read_pixel() {
        let mut fb = SoftwareFb::new(10, 10);
        fb.write_pixel(5, 5, [0xFF, 0x00, 0x00, 0xFF]);

        let idx = (5 * fb.info().stride + 5 * 4) as usize;
        assert_eq!(fb.data[idx], 0xFF);
        assert_eq!(fb.data[idx + 1], 0x00);
        assert_eq!(fb.data[idx + 2], 0x00);
        assert_eq!(fb.data[idx + 3], 0xFF);
    }

    #[test]
    fn fill_rect() {
        let mut fb = SoftwareFb::new(100, 100);
        fb.fill_rect(10, 10, 20, 20, [0x00, 0xFF, 0x00, 0xFF]);

        // Inside rect
        let idx = (15 * fb.info().stride + 15 * 4) as usize;
        assert_eq!(fb.data[idx], 0x00);
        assert_eq!(fb.data[idx + 1], 0xFF);

        // Outside rect
        let idx = (5 * fb.info().stride + 5 * 4) as usize;
        assert_eq!(fb.data[idx], 0x00);
    }

    #[test]
    fn blit_operation() {
        let mut src = SoftwareFb::new(4, 4);
        src.fill_rect(0, 0, 4, 4, [0xFF, 0x00, 0x00, 0xFF]);

        let mut fb = SoftwareFb::new(10, 10);
        fb.blit(2, 2, 4, 4, src.as_bytes(), src.info().stride);

        // Check a pixel in the blitted region
        let idx = (3 * fb.info().stride + 3 * 4) as usize;
        assert_eq!(fb.data[idx], 0xFF);
    }

    #[test]
    fn fb_backend_present() {
        use minix_compositor::pixel_buffer::PixelBuffer;
        use minix_compositor::backend::Backend;

        let sf = SoftwareFb::new(10, 10);
        let mut backend = FbBackend::new(sf);

        let mut buf = PixelBuffer::new_filled(10, 10, [0xFF, 0x00, 0x00, 0xFF]);
        backend.present(&buf);

        let dims = backend.dimensions();
        assert_eq!(dims, (10, 10));

        // Check the framebuffer has the red pixels
        let fb = backend.framebuffer_mut();
        let idx = (0 * fb.info().stride + 0 * 4) as usize;
        assert_eq!(fb.as_bytes()[idx], 0xFF);
    }

    #[test]
    fn pack_rgba_default() {
        let info = FbInfo::rgba32(100, 100);
        let pixel = info.pack_rgba([0xFF, 0x00, 0x80, 0xFF]);
        // R at offset 0, G at 8, B at 16, A at 24
        // pixel = R | (G << 8) | (B << 16) | (A << 24)
        //       = 0xFF | 0x0000 | 0x0080_0000 | 0xFF00_0000
        //       = 0xFF80_00FF
        // Byte map: [0]R=FF, [1]G=00, [2]B=80, [3]A=FF
        assert_eq!(pixel, 0xFF8000FFu32);
    }
}
