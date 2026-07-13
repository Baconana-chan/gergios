//! # PixelBuffer — Software RGBA framebuffer
//!
//! The fundamental rendering primitive. A 32-bit-per-pixel RGBA buffer
//! with methods for clearing, filling rectangles, blitting from other
//! buffers, and compositing with alpha blending.
//!
//! ## Phase 2.1 additions
//!
//! - `fill_rounded_rect` — rectangle with rounded corners
//! - `fill_linear_gradient` — horizontal/vertical color gradient
//! - `fill_radial_gradient` — circular color gradient
//! - `fill_triangle` — filled triangle via barycentric coordinates
//! - `draw_line` — Bresenham line algorithm (1 px wide)
//!
//! All coordinates are in pixels. The origin `(0, 0)` is at the top-left
//! corner. The `stride` is always `width * 4` (no row padding).

use core::cmp::{max, min};

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

    // ── Rounded rectangle ──────────────────────────────────────────────

    /// Fill a rectangle with rounded corners.
    ///
    /// `radius` is the corner radius in pixels. Clamped to half the
    /// shorter side to avoid overlap.
    pub fn fill_rounded_rect(&mut self, x: u32, y: u32, w: u32, h: u32,
        radius: u32, color: [u8; 4]) {
        if w == 0 || h == 0 {
            return;
        }

        // Clamp radius to avoid corner overlap
        let r = min(radius, min(w / 2, h / 2));

        // 1. Center body (full-width, full-height minus top and bottom corners)
        if h > 2 * r {
            self.fill_rect(x, y + r, w, h - 2 * r, color);
        }

        // 2. Top and bottom strips (full-width minus corners, r px high)
        if w > 2 * r && r > 0 {
            // Top strip
            self.fill_rect(x + r, y, w - 2 * r, r, color);
            // Bottom strip
            self.fill_rect(x + r, y + h - r, w - 2 * r, r, color);
        }

        // 3. The four corner quarter-circles
        if r > 0 {
            // Top-left
            self.fill_corner(x + r, y + r, r, -1, -1, color);
            // Top-right
            self.fill_corner(x + w - 1 - r, y + r, r, 1, -1, color);
            // Bottom-left
            self.fill_corner(x + r, y + h - 1 - r, r, -1, 1, color);
            // Bottom-right
            self.fill_corner(x + w - 1 - r, y + h - 1 - r, r, 1, 1, color);
        }
    }

    /// Fill a single corner quadrant of a circle centered at `(cx, cy)`.
    ///
    /// `sx` and `sy` are sign values (-1 or 1) indicating which quadrant:
    /// (-1, -1) = top-left, (1, -1) = top-right,
    /// (-1, 1)  = bottom-left, (1, 1)  = bottom-right
    fn fill_corner(&mut self, cx: u32, cy: u32, r: u32, sx: i32, sy: i32,
        color: [u8; 4]) {
        let rr = (r * r) as i64;

        for dy in 0..=r as i32 {
            let y2 = (dy * dy) as i64;
            if y2 > rr {
                continue;
            }

            // Compute y pixel position (handles both sy signs)
            let py = match (cy as i32).checked_add(dy * sy) {
                Some(y) if y >= 0 && (y as u32) < self.height => y as u32,
                _ => continue,
            };

            // Find max horizontal extent inside the circle at this y
            let max_x = (((rr - y2) as f64).sqrt().floor() as i32).min(r as i32);

            // Compute x range from cx - max_x ..= cx (sx < 0) or cx ..= cx + max_x (sx > 0)
            let (x_start, x_end) = if sx < 0 {
                ((cx as i32).saturating_sub(max_x), cx as i32)
            } else {
                (cx as i32, (cx as i32).saturating_add(max_x))
            };

            for px in x_start..=x_end {
                if px >= 0 && (px as u32) < self.width {
                    let old = self.get_pixel(px as u32, py);
                    self.set_pixel(px as u32, py, alpha_blend(color, old));
                }
            }
        }
    }

    // ── Gradient fills ─────────────────────────────────────────────────

    /// Fill a rectangle with a horizontal (left-to-right) linear gradient.

    /// Fill a rectangle with a horizontal (left-to-right) linear gradient.
    ///
    /// `stops` must have at least 2 entries. Colors are linearly
    /// interpolated between stops in the RGB + alpha space.
    pub fn fill_linear_gradient_h(&mut self, x: u32, y: u32, w: u32, h: u32,
        stops: &[ColorStop]) {
        if stops.len() < 2 || w == 0 || h == 0 {
            return;
        }

        let x_end = min(x + w, self.width);
        let y_end = min(y + h, self.height);
        let grad_width = x_end.saturating_sub(x);

        for py in y..y_end {
            for px in x..x_end {
                let t = if grad_width > 1 {
                    (px - x) as f32 / (grad_width - 1) as f32
                } else {
                    0.0
                };
                let color = interpolate_stops(stops, t);
                let old = self.get_pixel(px, py);
                self.set_pixel(px, py, alpha_blend(color, old));
            }
        }
    }

    /// Fill a rectangle with a vertical (top-to-bottom) linear gradient.
    pub fn fill_linear_gradient_v(&mut self, x: u32, y: u32, w: u32, h: u32,
        stops: &[ColorStop]) {
        if stops.len() < 2 || w == 0 || h == 0 {
            return;
        }

        let x_end = min(x + w, self.width);
        let y_end = min(y + h, self.height);
        let grad_height = y_end.saturating_sub(y);

        for py in y..y_end {
            let t = if grad_height > 1 {
                (py - y) as f32 / (grad_height - 1) as f32
            } else {
                0.0
            };
            let color = interpolate_stops(stops, t);
            for px in x..x_end {
                let old = self.get_pixel(px, py);
                self.set_pixel(px, py, alpha_blend(color, old));
            }
        }
    }

    /// Fill a rectangle with a radial (circular) gradient.
    ///
    /// The gradient radiates from center `(cx, cy)` out to `radius` pixels.
    pub fn fill_radial_gradient(&mut self, cx: f32, cy: f32, radius: f32,
        stops: &[ColorStop]) {
        if stops.len() < 2 || radius <= 0.0 {
            return;
        }

        let x_start = max(0i32, (cx - radius).floor() as i32) as u32;
        let y_start = max(0i32, (cy - radius).floor() as i32) as u32;
        let x_end = min(self.width - 1, (cx + radius).ceil() as u32);
        let y_end = min(self.height - 1, (cy + radius).ceil() as u32);

        for py in y_start..=y_end {
            for px in x_start..=x_end {
                let dx = px as f32 - cx;
                let dy = py as f32 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                let t = (dist / radius).clamp(0.0, 1.0);
                let color = interpolate_stops(stops, t);
                let old = self.get_pixel(px, py);
                self.set_pixel(px, py, alpha_blend(color, old));
            }
        }
    }

    // ── Triangle fill ──────────────────────────────────────────────────

    /// Fill a triangle defined by three vertices.
    ///
    /// Uses barycentric coordinates for per-pixel in/out testing.
    /// Vertices are given as `(x, y)` floating-point pairs.
    pub fn fill_triangle(&mut self, x1: f32, y1: f32, x2: f32, y2: f32,
        x3: f32, y3: f32, color: [u8; 4]) {
        // Compute bounding box
        let min_x = max(0i32, x1.min(x2).min(x3).floor() as i32) as u32;
        let min_y = max(0i32, y1.min(y2).min(y3).floor() as i32) as u32;
        let max_x = min(self.width - 1, x1.max(x2).max(x3).ceil() as u32);
        let max_y = min(self.height - 1, y1.max(y2).max(y3).ceil() as u32);

        if min_x > max_x || min_y > max_y {
            return;
        }

        // Barycentric helpers
        let denom = (y2 - y3) * (x1 - x3) + (x3 - x2) * (y1 - y3);
        if denom == 0.0 {
            return; // degenerate triangle (zero area)
        }

        let inv_denom = 1.0 / denom;

        for py in min_y..=max_y {
            for px in min_x..=max_x {
                let px_f = px as f32 + 0.5;
                let py_f = py as f32 + 0.5;

                // Convert to barycentric coordinates
                let w1 = ((y2 - y3) * (px_f - x3) + (x3 - x2) * (py_f - y3)) * inv_denom;
                let w2 = ((y3 - y1) * (px_f - x3) + (x1 - x3) * (py_f - y3)) * inv_denom;
                let w3 = 1.0 - w1 - w2;

                // Check if inside triangle (allow small epsilon for edge cases)
                if w1 >= -1e-6 && w2 >= -1e-6 && w3 >= -1e-6 {
                    let old = self.get_pixel(px, py);
                    self.set_pixel(px, py, alpha_blend(color, old));
                }
            }
        }
    }

    // ── Line drawing ───────────────────────────────────────────────────

    /// Draw a 1-pixel-wide line using Bresenham's line algorithm.
    ///
    /// Coordinates are inclusive on both ends. Clips to buffer bounds.
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32,
        color: [u8; 4]) {
        let mut x = x0;
        let mut y = y0;
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;

        loop {
            if x >= 0 && y >= 0 {
                let ux = x as u32;
                let uy = y as u32;
                if ux < self.width && uy < self.height {
                    let old = self.get_pixel(ux, uy);
                    self.set_pixel(ux, uy, alpha_blend(color, old));
                }
            }

            if x == x1 && y == y1 {
                break;
            }

            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
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
/// A single color stop in a gradient.
#[derive(Debug, Clone, Copy)]
pub struct ColorStop {
    /// Position along the gradient axis, in range `0.0 ..= 1.0`.
    pub position: f32,
    /// RGBA color at this stop.
    pub color: [u8; 4],
}

// ── Gradient helpers ────────────────────────────────────────────────────

/// Linearly interpolate `t` (0.0–1.0) across an array of color stops.
fn interpolate_stops(stops: &[ColorStop], t: f32) -> [u8; 4] {
    debug_assert!(stops.len() >= 2);

    // Clamp t to [0, 1]
    let t = t.clamp(0.0, 1.0);

    // Find the two stops we're between
    let mut lower = &stops[0];
    let mut upper = &stops[stops.len() - 1];

    for i in 0..stops.len() - 1 {
        if t >= stops[i].position && t <= stops[i + 1].position {
            lower = &stops[i];
            upper = &stops[i + 1];
            break;
        }
    }

    // Fraction within the segment [lower, upper]
    let segment_len = upper.position - lower.position;
    let frac = if segment_len > 0.0 {
        (t - lower.position) / segment_len
    } else {
        0.0
    };

    lerp_color(lower.color, upper.color, frac)
}

/// Linearly interpolate between two RGBA colors.
fn lerp_color(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let t = t.clamp(0.0, 1.0);
    let one_minus_t = 1.0 - t;
    [
        (a[0] as f32 * one_minus_t + b[0] as f32 * t).round() as u8,
        (a[1] as f32 * one_minus_t + b[1] as f32 * t).round() as u8,
        (a[2] as f32 * one_minus_t + b[2] as f32 * t).round() as u8,
        (a[3] as f32 * one_minus_t + b[3] as f32 * t).round() as u8,
    ]
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

    // ── Existing tests ─────────────────────────────────────────────────

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
        let buf = PixelBuffer::new_filled(4, 4, [0xFF, 0x00, 0x00, 0xFF]);
        let mut dst = PixelBuffer::new_filled(4, 4, [0x00, 0x00, 0x00, 0xFF]);

        // Blit with source region extending past buffer → should clamp
        dst.blit_from(&buf, 0, 0, 2, 2, 10, 10, false);
        assert_eq!(dst.get_pixel(0, 0), [0xFF, 0x00, 0x00, 0xFF]); // copied pixel
        assert_eq!(dst.get_pixel(1, 1), [0xFF, 0x00, 0x00, 0xFF]); // copied pixel
        assert_eq!(dst.get_pixel(2, 2), [0x00, 0x00, 0x00, 0xFF]); // past end of src
    }

    // ── Phase 2.1: Rounded rectangle tests ─────────────────────────────

    #[test]
    fn rounded_rect_full_radius() {
        let mut buf = PixelBuffer::new_filled(20, 20, [0x00, 0x00, 0x00, 0xFF]);
        // Rect at (2,2) w=16 h=16 r=5 → corners center at (7,7), (12,7), (7,12), (12,12)
        buf.fill_rounded_rect(2, 2, 16, 16, 5, [0xFF, 0x00, 0x00, 0xFF]);

        // Center pixel should be red
        assert_eq!(buf.get_pixel(10, 10), [0xFF, 0x00, 0x00, 0xFF]);
        // Outside corners — should remain black
        assert_eq!(buf.get_pixel(2, 2), [0x00, 0x00, 0x00, 0xFF]);
        // Top strip (x=7..=12, y=2..=6) — should be red
        assert_eq!(buf.get_pixel(8, 2), [0xFF, 0x00, 0x00, 0xFF]);
        // Center body (x=2..=17, y=7..=12) — left edge should be red
        assert_eq!(buf.get_pixel(2, 8), [0xFF, 0x00, 0x00, 0xFF]);
    }

    #[test]
    fn rounded_rect_zero_radius() {
        let mut buf = PixelBuffer::new_filled(10, 10, [0x00, 0x00, 0x00, 0xFF]);
        buf.fill_rounded_rect(2, 2, 6, 6, 0, [0xFF, 0x00, 0x00, 0xFF]);

        // With radius 0, should behave like a regular rect
        assert_eq!(buf.get_pixel(2, 2), [0xFF, 0x00, 0x00, 0xFF]);
        assert_eq!(buf.get_pixel(7, 7), [0xFF, 0x00, 0x00, 0xFF]);
        assert_eq!(buf.get_pixel(1, 1), [0x00, 0x00, 0x00, 0xFF]); // outside
    }

    #[test]
    fn rounded_rect_large_radius_clamped() {
        let mut buf = PixelBuffer::new_filled(10, 10, [0x00, 0x00, 0x00, 0xFF]);
        // Radius 10 should clamp to 5 (half of 10)
        buf.fill_rounded_rect(0, 0, 10, 10, 10, [0xFF, 0x00, 0x00, 0xFF]);

        // Center is red
        assert_eq!(buf.get_pixel(5, 5), [0xFF, 0x00, 0x00, 0xFF]);
    }

    // ── Phase 2.1: Gradient tests ──────────────────────────────────────

    #[test]
    fn linear_gradient_h_two_stops() {
        let mut buf = PixelBuffer::new_filled(10, 10, [0x00, 0x00, 0x00, 0xFF]);
        let stops = [
            ColorStop { position: 0.0, color: [0xFF, 0x00, 0x00, 0xFF] }, // red
            ColorStop { position: 1.0, color: [0x00, 0x00, 0xFF, 0xFF] }, // blue
        ];
        buf.fill_linear_gradient_h(0, 0, 10, 10, &stops);

        // Left edge should be red
        assert_eq!(buf.get_pixel(0, 0)[0], 0xFF);
        assert_eq!(buf.get_pixel(0, 0)[2], 0x00);
        // Right edge should be blue
        assert_eq!(buf.get_pixel(9, 0)[0], 0x00);
        assert_eq!(buf.get_pixel(9, 0)[2], 0xFF);
    }

    #[test]
    fn linear_gradient_v_two_stops() {
        let mut buf = PixelBuffer::new_filled(10, 10, [0x00, 0x00, 0x00, 0xFF]);
        let stops = [
            ColorStop { position: 0.0, color: [0x00, 0xFF, 0x00, 0xFF] }, // green
            ColorStop { position: 1.0, color: [0x00, 0x00, 0x00, 0xFF] }, // black
        ];
        buf.fill_linear_gradient_v(0, 0, 10, 10, &stops);

        // Top edge should be green
        assert_eq!(buf.get_pixel(0, 0)[1], 0xFF);
        // Bottom edge should be black
        assert_eq!(buf.get_pixel(0, 9), [0x00, 0x00, 0x00, 0xFF]);
    }

    #[test]
    fn radial_gradient_center_red() {
        let mut buf = PixelBuffer::new_filled(20, 20, [0x00, 0x00, 0x00, 0xFF]);
        let stops = [
            ColorStop { position: 0.0, color: [0xFF, 0xFF, 0xFF, 0xFF] }, // white
            ColorStop { position: 1.0, color: [0x00, 0x00, 0x00, 0xFF] }, // black
        ];
        buf.fill_radial_gradient(10.0, 10.0, 10.0, &stops);

        // Center should be white
        assert_eq!(buf.get_pixel(10, 10), [0xFF, 0xFF, 0xFF, 0xFF]);
        // Far corner should still be black (outside radius)
        assert_eq!(buf.get_pixel(0, 0), [0x00, 0x00, 0x00, 0xFF]);
    }

    // ── Phase 2.1: Triangle tests ──────────────────────────────────────

    #[test]
    fn fill_triangle_small() {
        let mut buf = PixelBuffer::new_filled(10, 10, [0x00, 0x00, 0x00, 0xFF]);
        // Small triangle in the center
        buf.fill_triangle(3.0, 7.0, 5.0, 2.0, 7.0, 7.0, [0xFF, 0x00, 0x00, 0xFF]);

        // Inside triangle (centroid area)
        assert_eq!(buf.get_pixel(5, 5), [0xFF, 0x00, 0x00, 0xFF]);
        // Outside triangle
        assert_eq!(buf.get_pixel(0, 0), [0x00, 0x00, 0x00, 0xFF]);
    }

    #[test]
    fn fill_triangle_degenerate() {
        let mut buf = PixelBuffer::new_filled(10, 10, [0x00, 0x00, 0x00, 0xFF]);
        // Degenerate triangle (all points on one line) — should do nothing
        buf.fill_triangle(1.0, 1.0, 5.0, 5.0, 9.0, 9.0, [0xFF, 0x00, 0x00, 0xFF]);

        // Nothing should be drawn (or at least no crash)
        assert_eq!(buf.get_pixel(5, 5), [0x00, 0x00, 0x00, 0xFF]);
    }

    // ── Phase 2.1: Line drawing tests ──────────────────────────────────

    #[test]
    fn draw_horizontal_line() {
        let mut buf = PixelBuffer::new_filled(10, 10, [0x00, 0x00, 0x00, 0xFF]);
        buf.draw_line(1, 5, 8, 5, [0xFF, 0x00, 0x00, 0xFF]);

        assert_eq!(buf.get_pixel(1, 5), [0xFF, 0x00, 0x00, 0xFF]); // start
        assert_eq!(buf.get_pixel(4, 5), [0xFF, 0x00, 0x00, 0xFF]); // middle
        assert_eq!(buf.get_pixel(8, 5), [0xFF, 0x00, 0x00, 0xFF]); // end
    }

    #[test]
    fn draw_vertical_line() {
        let mut buf = PixelBuffer::new_filled(10, 10, [0x00, 0x00, 0x00, 0xFF]);
        buf.draw_line(5, 1, 5, 8, [0x00, 0xFF, 0x00, 0xFF]);

        assert_eq!(buf.get_pixel(5, 1), [0x00, 0xFF, 0x00, 0xFF]);
        assert_eq!(buf.get_pixel(5, 5), [0x00, 0xFF, 0x00, 0xFF]);
        assert_eq!(buf.get_pixel(5, 8), [0x00, 0xFF, 0x00, 0xFF]);
    }

    #[test]
    fn draw_diagonal_line() {
        let mut buf = PixelBuffer::new_filled(10, 10, [0x00, 0x00, 0x00, 0xFF]);
        buf.draw_line(0, 0, 7, 7, [0x00, 0x00, 0xFF, 0xFF]);

        assert_eq!(buf.get_pixel(0, 0), [0x00, 0x00, 0xFF, 0xFF]);
        assert_eq!(buf.get_pixel(4, 4), [0x00, 0x00, 0xFF, 0xFF]);
        assert_eq!(buf.get_pixel(7, 7), [0x00, 0x00, 0xFF, 0xFF]);
    }

    #[test]
    fn draw_line_reverse() {
        let mut buf = PixelBuffer::new_filled(10, 10, [0x00, 0x00, 0x00, 0xFF]);
        // Reverse direction — should still draw the same line
        buf.draw_line(8, 2, 2, 2, [0xFF, 0xFF, 0xFF, 0xFF]);

        assert_eq!(buf.get_pixel(2, 2), [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(buf.get_pixel(5, 2), [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(buf.get_pixel(8, 2), [0xFF, 0xFF, 0xFF, 0xFF]);
    }

    // ── Phase 2.1: Helper tests ────────────────────────────────────────

    #[test]
    fn lerp_color_inbetween() {
        let result = lerp_color([0xFF, 0x00, 0x00, 0xFF], [0x00, 0x00, 0xFF, 0xFF], 0.5);
        assert_eq!(result[0], 0x80); // halfway between 0xFF and 0x00
        assert_eq!(result[2], 0x80); // halfway between 0x00 and 0xFF
        assert_eq!(result[3], 0xFF); // alpha stays
    }

    #[test]
    fn interpolate_stops_basic() {
        let stops = [
            ColorStop { position: 0.0, color: [0x00, 0xFF, 0x00, 0xFF] },
            ColorStop { position: 0.5, color: [0xFF, 0xFF, 0x00, 0xFF] },
            ColorStop { position: 1.0, color: [0xFF, 0x00, 0x00, 0xFF] },
        ];
        let result = interpolate_stops(&stops, 0.0);
        assert_eq!(result, [0x00, 0xFF, 0x00, 0xFF]); // first stop

        let result = interpolate_stops(&stops, 0.5);
        assert_eq!(result, [0xFF, 0xFF, 0x00, 0xFF]); // middle stop

        let result = interpolate_stops(&stops, 1.0);
        assert_eq!(result[0], 0xFF);
        assert_eq!(result[1], 0x00); // last stop
    }
}
