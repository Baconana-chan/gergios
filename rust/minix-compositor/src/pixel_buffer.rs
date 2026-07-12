//! # PixelBuffer — Software RGBA framebuffer
//!
//! The fundamental rendering primitive. A 32-bit-per-pixel RGBA buffer
//! with methods for clearing, filling rectangles, blitting from other
//! buffers, and compositing with alpha blending.
//!
//! All coordinates are in pixels. The origin `(0, 0)` is at the top-left
//! corner. The `stride` is always `width * 4` (no row padding).

use core::cmp::min;

/// A 32-bit RGBA pixel buffer (8 bits per channel, non-premultiplied).
///
/// The pixel format is `[R, G, B, A]` where each component is `u8`.
/// Alpha blending uses the standard over operator:
///   `out = src * src.a + dst * (1 - src.a)`
#[derive(Clone)]
pub struct PixelBuffer {
    pub width: u32,
    pub height: u32,
    pub stride: u32,          // always width * 4
    pub data: alloc::vec::Vec<u8>,
}

impl PixelBuffer {
    /// Create a new pixel buffer of the given dimensions, zero-filled.
    pub fn new(width: u32, height: u32) -> Self {
        let stride = width * 4;
        let size = (stride * height) as usize;
        Self {
            width,
            height,
            stride,
            data: alloc::vec![0u8; size],
        }
    }

    /// Create a new pixel buffer filled with the given color.
    pub fn new_filled(width: u32, height: u32, color: [u8; 4]) -> Self {
        let mut buf = Self::new(width, height);
        buf.clear(color);
        buf
    }

    // ── Accessors ──────────────────────────────────────────────────────

    /// Get a reference to the raw pixel data.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get a mutable reference to the raw pixel data.
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.data
    }

    // ── Pixel access ───────────────────────────────────────────────────

    /// Get the RGBA value of the pixel at `(x, y)`.
    /// Returns `[0, 0, 0, 0]` if out of bounds.
    pub fn get_pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0];
        }
        let idx = (y * self.stride + x * 4) as usize;
        [
            self.data[idx],
            self.data[idx + 1],
            self.data[idx + 2],
            self.data[idx + 3],
        ]
    }

    /// Set the RGBA value of the pixel at `(x, y)`.
    /// Does nothing if out of bounds.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = (y * self.stride + x * 4) as usize;
        self.data[idx] = color[0];
        self.data[idx + 1] = color[1];
        self.data[idx + 2] = color[2];
        self.data[idx + 3] = color[3];
    }

    // ── Fill operations ────────────────────────────────────────────────

    /// Fill the entire buffer with a solid color.
    /// Alpha-blends if the color has alpha < 255.
    pub fn clear(&mut self, color: [u8; 4]) {
        if color[3] == 0xFF {
            // Fast path: opaque fill via memset
            let pixel = u32::from_ne_bytes(color);
            let pixel_bytes = pixel.to_ne_bytes();
            for chunk in self.data.chunks_exact_mut(4) {
                chunk.copy_from_slice(&pixel_bytes);
            }
        } else {
            // Alpha-blended clear
            for y in 0..self.height {
                for x in 0..self.width {
                    let old = self.get_pixel(x, y);
                    self.set_pixel(x, y, alpha_blend(color, old));
                }
            }
        }
    }

    /// Fill a rectangular region with a solid color (alpha-blended).
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: [u8; 4]) {
        let x_end = min(x + w, self.width);
        let y_end = min(y + h, self.height);

        for py in y..y_end {
            for px in x..x_end {
                let old = self.get_pixel(px, py);
                self.set_pixel(px, py, alpha_blend(color, old));
            }
        }
    }

    // ── Blit operations ────────────────────────────────────────────────

    /// Copy pixels from `src` at position `(src_x, src_y)` to this buffer
    /// at `(dst_x, dst_y)`, with optional alpha blending.
    ///
    /// If `blend` is true, uses over-compositing. If false, replaces
    /// the destination pixels (no blending, for performance).
    pub fn blit_from(&mut self, src: &PixelBuffer, dst_x: u32, dst_y: u32,
        src_x: u32, src_y: u32, w: u32, h: u32, blend: bool) {
        let copy_w = min(w, min(src.width.saturating_sub(src_x), self.width.saturating_sub(dst_x)));
        let copy_h = min(h, min(src.height.saturating_sub(src_y), self.height.saturating_sub(dst_y)));

        if copy_w == 0 || copy_h == 0 {
            return;
        }

        if blend {
            for dy in 0..copy_h {
                for dx in 0..copy_w {
                    let src_px = src.get_pixel(src_x + dx, src_y + dy);
                    let dst_px = self.get_pixel(dst_x + dx, dst_y + dy);
                    self.set_pixel(dst_x + dx, dst_y + dy, alpha_blend(src_px, dst_px));
                }
            }
        } else {
            for dy in 0..copy_h {
                let src_off = ((src_y + dy) * src.stride + (src_x * 4)) as usize;
                let dst_off = ((dst_y + dy) * self.stride + (dst_x * 4)) as usize;
                let len = (copy_w * 4) as usize;
                self.data[dst_off..dst_off + len].copy_from_slice(&src.data[src_off..src_off + len]);
            }
        }
    }

    /// Blit the entire source buffer into this buffer at `(dst_x, dst_y)`.
    pub fn blit_all_from(&mut self, src: &PixelBuffer, dst_x: u32, dst_y: u32, blend: bool) {
        self.blit_from(src, dst_x, dst_y, 0, 0, src.width, src.height, blend);
    }
}

// ── Alpha blending helpers ──────────────────────────────────────────────

/// Standard over-compositing: `out = src * a_src + dst * (1 - a_src)`
pub fn alpha_blend(src: [u8; 4], dst: [u8; 4]) -> [u8; 4] {
    if src[3] == 0xFF {
        return src; // Fully opaque
    }
    if src[3] == 0 {
        return dst; // Fully transparent
    }

    let a_src = src[3] as u32;
    let a_dst = dst[3] as u32;
    let a_out = a_src + a_dst * (255 - a_src) / 255;

    if a_out == 0 {
        return [0, 0, 0, 0];
    }

    let blend_channel = |s: u8, d: u8| -> u8 {
        let s = s as u32;
        let d = d as u32;
        ((s * a_src + d * a_dst * (255 - a_src) / 255) / a_out) as u8
    };

    [
        blend_channel(src[0], dst[0]),
        blend_channel(src[1], dst[1]),
        blend_channel(src[2], dst[2]),
        a_out as u8,
    ]
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_buffer_is_zeroed() {
        let buf = PixelBuffer::new(10, 10);
        assert_eq!(buf.width, 10);
        assert_eq!(buf.height, 10);
        assert!(buf.data.iter().all(|&b| b == 0));
    }

    #[test]
    fn fill_opaque_memset_path() {
        let mut buf = PixelBuffer::new(4, 4);
        buf.clear([0xFF, 0x00, 0x00, 0xFF]); // red
        assert_eq!(buf.get_pixel(0, 0), [0xFF, 0x00, 0x00, 0xFF]);
        assert_eq!(buf.get_pixel(3, 3), [0xFF, 0x00, 0x00, 0xFF]);
    }

    #[test]
    fn fill_rect_partial() {
        let mut buf = PixelBuffer::new_filled(10, 10, [0x00, 0x00, 0x00, 0xFF]);
        buf.fill_rect(2, 2, 3, 3, [0xFF, 0x00, 0x00, 0xFF]);
        // Inside rect
        assert_eq!(buf.get_pixel(3, 3), [0xFF, 0x00, 0x00, 0xFF]);
        // Outside rect
        assert_eq!(buf.get_pixel(0, 0), [0x00, 0x00, 0x00, 0xFF]);
        assert_eq!(buf.get_pixel(9, 9), [0x00, 0x00, 0x00, 0xFF]);
    }

    #[test]
    fn alpha_blend_opaque() {
        let result = alpha_blend([0xFF, 0x00, 0x00, 0xFF], [0x00, 0xFF, 0x00, 0xFF]);
        assert_eq!(result, [0xFF, 0x00, 0x00, 0xFF]); // src is opaque → src wins
    }

    #[test]
    fn alpha_blend_transparent() {
        let result = alpha_blend([0xFF, 0x00, 0x00, 0x00], [0x00, 0xFF, 0x00, 0xFF]);
        assert_eq!(result, [0x00, 0xFF, 0x00, 0xFF]); // src is fully transparent → dst wins
    }

    #[test]
    fn blit_no_blend_copies_data() {
        let mut src = PixelBuffer::new_filled(4, 4, [0xFF, 0x00, 0x00, 0xFF]);
        src.fill_rect(0, 0, 2, 2, [0x00, 0xFF, 0x00, 0xFF]); // green square at origin
        let mut dst = PixelBuffer::new_filled(8, 8, [0x00, 0x00, 0x00, 0xFF]);

        dst.blit_from(&src, 2, 2, 0, 0, 4, 4, false);

        assert_eq!(dst.get_pixel(2, 2), [0x00, 0xFF, 0x00, 0xFF]); // copied green pixel
        assert_eq!(dst.get_pixel(0, 0), [0x00, 0x00, 0x00, 0xFF]); // untouched
    }

    #[test]
    fn blit_clips_at_edges() {
        let mut buf = PixelBuffer::new_filled(4, 4, [0xFF, 0x00, 0x00, 0xFF]);
        let mut dst = PixelBuffer::new_filled(4, 4, [0x00, 0x00, 0x00, 0xFF]);

        // Blit with source region extending past buffer → should clamp
        dst.blit_from(&buf, 0, 0, 2, 2, 10, 10, false);
        assert_eq!(dst.get_pixel(0, 0), [0xFF, 0x00, 0x00, 0xFF]); // copied pixel
        assert_eq!(dst.get_pixel(1, 1), [0xFF, 0x00, 0x00, 0xFF]); // copied pixel
        assert_eq!(dst.get_pixel(2, 2), [0x00, 0x00, 0x00, 0xFF]); // past end of src
    }
}
