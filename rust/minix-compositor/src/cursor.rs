//! # Cursor — Software cursor bitmap and compositing
//!
//! Provides a software-rendered cursor that overlays the composited output.
//! The cursor has:
//! - A bitmap (small `PixelBuffer` for the cursor image)
//! - A hotspot (the "click point" offset within the bitmap)
//! - Position (x, y) in output coordinates
//! - Visibility flag
//!
//! **Phase 2.5**
//!
//! A 16×16 default arrow cursor is built-in (no external files needed).
//! Custom cursor images can be set via `set_image()`.

use crate::pixel_buffer::PixelBuffer;

/// Default cursor width in pixels.
const CURSOR_DEFAULT_W: u32 = 16;
/// Default cursor height in pixels.
const CURSOR_DEFAULT_H: u32 = 16;

// ── Cursor ──────────────────────────────────────────────────────────────

/// A software cursor with bitmap, hotspot, and position.
pub struct Cursor {
    /// Cursor bitmap (RGBA). Rendered on top of the composited output.
    pub image: PixelBuffer,
    /// Hotspot X: the "click point" X offset from the bitmap's left edge.
    pub hotspot_x: u32,
    /// Hotspot Y: the "click point" Y offset from the bitmap's top edge.
    pub hotspot_y: u32,
    /// Cursor X position in output coordinates.
    pub x: i32,
    /// Cursor Y position in output coordinates.
    pub y: i32,
    /// Whether the cursor is visible.
    pub visible: bool,
}

impl Cursor {
    /// Create a new cursor with the default arrow bitmap.
    pub fn new() -> Self {
        Self {
            image: build_default_arrow(),
            hotspot_x: 1,
            hotspot_y: 1,
            x: 0,
            y: 0,
            visible: true,
        }
    }

    /// Set a custom cursor image.
    ///
    /// `image` is a small RGBA bitmap. `hotspot_x` / `hotspot_y` specify
    /// the pixel within the bitmap that is the "pointer" location.
    pub fn set_image(&mut self, image: PixelBuffer, hotspot_x: u32, hotspot_y: u32) {
        let hx = hotspot_x.min(image.width.saturating_sub(1));
        let hy = hotspot_y.min(image.height.saturating_sub(1));
        self.image = image;
        self.hotspot_x = hx;
        self.hotspot_y = hy;
    }

    /// Set cursor position.
    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    /// Show or hide the cursor.
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Composite the cursor onto the output buffer at its current position.
    ///
    /// Clips the cursor bitmap to the output bounds (bounding box).
    /// The cursor is drawn with alpha blending.
    pub fn composite(&self, output: &mut PixelBuffer) {
        if !self.visible {
            return;
        }

        let cw = self.image.width;
        let ch = self.image.height;

        // Top-left of the cursor bitmap in output coordinates
        let dst_x = self.x.saturating_sub(self.hotspot_x as i32);
        let dst_y = self.y.saturating_sub(self.hotspot_y as i32);

        // Compute visible region clipped to output bounds
        let (sx, sy, sw, sh) = if dst_x < 0 {
            let crop_x = (-dst_x) as u32;
            let sw = cw.saturating_sub(crop_x);
            let sy = if dst_y < 0 { (-dst_y) as u32 } else { 0 };
            let sh = ch.saturating_sub(sy);
            (crop_x, sy, sw, sh)
        } else {
            (0, 0, cw, ch)
        };

        let (dx, dy) = (
            dst_x.max(0) as u32,
            dst_y.max(0) as u32,
        );

        let final_w = sw.min(output.width.saturating_sub(dx));
        let final_h = sh.min(output.height.saturating_sub(dy));

        if final_w == 0 || final_h == 0 {
            return;
        }

        // Blend cursor into output
        for oy in 0..final_h {
            for ox in 0..final_w {
                let src_px = self.image.get_pixel(sx + ox, sy + oy);
                if src_px[3] == 0 {
                    continue;
                }
                let dst_px = output.get_pixel(dx + ox, dy + oy);
                output.set_pixel(dx + ox, dy + oy,
                    crate::pixel_buffer::alpha_blend(src_px, dst_px));
            }
        }
    }
}

impl Default for Cursor {
    fn default() -> Self {
        Self::new()
    }
}

// ── Default arrow cursor bitmap ─────────────────────────────────────────

/// Build the default 16×16 arrow cursor bitmap.
///
/// The arrow shape:
/// ```text
/// ██░░░░░░░░░░░░░
/// ███░░░░░░░░░░░░
/// ████░░░░░░░░░░░
/// █████░░░░░░░░░░
/// ██████░░░░░░░░░
/// ███████░░░░░░░░
/// ████████░░░░░░░
/// █████████░░░░░░
/// ██████████░░░░░
/// ███████████░░░░
/// ████████████░░░
/// ███░░░████░░░░░
/// ██ ██░░███░░░░░
/// █  ███░░██░░░░░
/// ░░░░░██░░██░░░░
/// ░░░░░░██░░██░░░
/// ```
/// Hotspot is at (1, 1) — the top-left corner of the arrow tip.
fn build_default_arrow() -> PixelBuffer {
    let w = CURSOR_DEFAULT_W;
    let h = CURSOR_DEFAULT_H;
    let mut buf = PixelBuffer::new(w, h);

    // Arrow body: black pixels with full opacity
    let black = [0x00u8, 0x00, 0x00, 0xFF];
    // Anti-aliased edge: dark gray with partial opacity
    let edge = [0x20u8, 0x20, 0x20, 0x80];

    // Draw the arrow outline row by row
    // Each row: fill from left edge to `end_x`
    let arrow_rows: [(u32, u32); 16] = [
        // (y, end_x_exclusive)
        (0, 2),   // ██
        (1, 3),   // ███
        (2, 4),   // ████
        (3, 5),   // █████
        (4, 6),   // ██████
        (5, 7),   // ███████
        (6, 8),   // ████████
        (7, 9),   // █████████
        (8, 10),  // ██████████
        (9, 11),  // ███████████
        (10, 12), // ████████████
        // Row 11: break in the arrow shaft (███ ████)
        (11, 3),  // ███
        // Row 12: ██ ██ ███
        (12, 2),  // ██
        // Row 13: █ ███ ██
        (13, 1),  // █
        // Rows 14-15: empty (only tail pixels)
        (14, 0),  // (no main body — tail handled separately)
        (15, 0),
    ];

    for &(y, end_x) in &arrow_rows {
        for x in 0..end_x {
            buf.set_pixel(x, y, black);
        }
    }

    // Draw the "tail" of the arrow (lower-right diagonal part)
    // Row 11: shaft continues after gap — pixels at x=7..11
    for x in 7..11 {
        buf.set_pixel(x, 11, black);
    }
    // Row 12: left part at x=4, diagonal tail starts at x=7..10
    buf.set_pixel(4, 12, black);
    for x in 7..10 {
        buf.set_pixel(x, 12, black);
    }
    // Row 13: left part at x=4..6, diagonal tail at x=7..9
    for x in 4..7 {
        buf.set_pixel(x, 13, black);
    }
    for x in 7..9 {
        buf.set_pixel(x, 13, black);
    }
    // Row 14: diagonal tail at x=5..8
    for x in 5..8 {
        buf.set_pixel(x, 14, black);
    }
    // Row 15: diagonal tail at x=6..7
    for x in 6..8 {
        buf.set_pixel(x, 15, black);
    }

    // Anti-aliased edges on the right side of the arrow
    // Row 0: anti-alias at x=2
    buf.set_pixel(2, 0, edge);
    // Row 1: anti-alias at x=3
    buf.set_pixel(3, 1, edge);
    // Row 2: anti-alias at x=4
    buf.set_pixel(4, 2, edge);
    // Row 11: anti-alias around the gap — x=3
    buf.set_pixel(3, 11, edge);
    // Row 12: anti-alias at x=5
    buf.set_pixel(5, 12, edge);
    // Row 14: anti-alias at x=4
    buf.set_pixel(4, 14, edge);

    buf
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_default_has_bitmap() {
        let c = Cursor::new();
        assert_eq!(c.image.width, CURSOR_DEFAULT_W);
        assert_eq!(c.image.height, CURSOR_DEFAULT_H);
        assert!(c.visible);
    }

    #[test]
    fn cursor_hotspot_default() {
        let c = Cursor::new();
        assert_eq!(c.hotspot_x, 1);
        assert_eq!(c.hotspot_y, 1);
    }

    #[test]
    fn cursor_default_bitmap_not_empty() {
        let c = Cursor::new();
        let has_non_transparent = c.image.as_bytes().iter().any(|&b| b != 0);
        assert!(has_non_transparent, "default cursor bitmap should have visible pixels");
    }

    #[test]
    fn cursor_invisible_composite_no_effect() {
        let mut output = PixelBuffer::new_filled(100, 100, [0x00; 4]);
        let mut cursor = Cursor::new();
        cursor.set_visible(false);
        cursor.set_position(50, 50);

        cursor.composite(&mut output);

        // All pixels should still be zero (cursor was invisible)
        assert!(output.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn cursor_visible_composite_has_effect() {
        let mut output = PixelBuffer::new_filled(100, 100, [0x00; 4]);
        let cursor = Cursor::new();
        cursor.composite(&mut output);

        let has_non_zero = output.as_bytes().iter().any(|&b| b != 0);
        assert!(has_non_zero, "visible cursor should draw pixels");
    }

    #[test]
    fn cursor_composite_clips_at_output_edges() {
        // Cursor at (0, 0) — top-left corner
        let mut buf = PixelBuffer::new_filled(20, 20, [0x00; 4]);
        let c = Cursor::new();
        c.composite(&mut buf);
        // No crash, and cursor pixels visible in top-left
        let has_pixels = buf.as_bytes().iter().any(|&b| b != 0);
        assert!(has_pixels, "cursor at origin should render");

        // Cursor far off-screen (negative position)
        let mut buf2 = PixelBuffer::new_filled(20, 20, [0x00; 4]);
        let mut c2 = Cursor::new();
        c2.set_position(-100, -100);
        c2.composite(&mut buf2);
        // All pixels should remain zero (cursor entirely off-screen)
        assert!(buf2.as_bytes().iter().all(|&b| b == 0),
            "off-screen cursor should not render");

        // Cursor at bottom-right edge (partially visible)
        let mut buf3 = PixelBuffer::new_filled(20, 20, [0x00; 4]);
        let mut c3 = Cursor::new();
        c3.set_position(19, 19); // hotspot (1,1) → bitmap at (18,18), partially visible
        c3.composite(&mut buf3);
        // No crash, some pixels may be visible
    }

    #[test]
    fn cursor_set_custom_image() {
        let mut buf = PixelBuffer::new_filled(8, 8, [0xFF, 0x00, 0x00, 0xFF]);
        let mut cursor = Cursor::new();
        cursor.set_image(buf, 4, 4);

        assert_eq!(cursor.image.width, 8);
        assert_eq!(cursor.hotspot_x, 4);
        assert_eq!(cursor.hotspot_y, 4);
    }

    #[test]
    fn cursor_hotspot_clamped_to_bitmap() {
        let buf = PixelBuffer::new_filled(4, 4, [0xFF, 0x00, 0x00, 0xFF]);
        let mut cursor = Cursor::new();
        // Hotspot outside bitmap → should be clamped
        cursor.set_image(buf, 100, 100);
        assert_eq!(cursor.hotspot_x, 3); // max = width - 1
        assert_eq!(cursor.hotspot_y, 3);
    }

    #[test]
    fn cursor_set_position_and_visible() {
        let mut cursor = Cursor::new();
        cursor.set_position(100, 200);
        assert_eq!(cursor.x, 100);
        assert_eq!(cursor.y, 200);

        cursor.set_visible(false);
        assert!(!cursor.visible);

        cursor.set_visible(true);
        assert!(cursor.visible);
    }
}
