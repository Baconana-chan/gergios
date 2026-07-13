//! # Compositor — Surface management and software compositing
//!
//! The compositor manages a set of surfaces, composites them in z-order
//! into an output buffer, and notifies a backend when the output is ready.

use alloc::vec::Vec;

use crate::pixel_buffer::PixelBuffer;
use crate::surface::Surface;
use crate::backend::Backend;
use crate::cursor::Cursor;

/// Statistics from the last composite cycle.
#[derive(Debug, Clone, Default)]
pub struct CompositeStats {
    pub surfaces_composited: u32,
    pub dirty_surfaces: u32,
    pub total_surfaces: u32,
    pub output_width: u32,
    pub output_height: u32,
    /// Whether the cursor was rendered on this frame.
    pub cursor_rendered: bool,
    /// Cursor position on this frame.
    pub cursor_x: i32,
    pub cursor_y: i32,
}

/// The main compositor: manages surfaces, composites into an output buffer.
pub struct Compositor {
    /// All managed surfaces.
    surfaces: Vec<Surface>,
    /// Z-order sorted indices into `surfaces`.
    z_order: Vec<usize>,
    /// The output buffer (composited result).
    pub output: PixelBuffer,
    /// Output width in pixels.
    pub output_width: u32,
    /// Output height in pixels.
    pub output_height: u32,
    /// Background color for the output (used when no surface covers a pixel).
    pub background_color: [u8; 4],
    /// Dirty flag: set when any surface is dirty, cleared after composite.
    pub needs_composite: bool,
    /// Software cursor rendered on top of all surfaces.
    pub cursor: Cursor,

}

impl Compositor {
    /// Create a new compositor with the given output dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            surfaces: Vec::new(),
            z_order: Vec::new(),
            output: PixelBuffer::new(width, height),
            output_width: width,
            output_height: height,
            background_color: [0x20, 0x30, 0x40, 0xFF],
            needs_composite: true,
            cursor: Cursor::new(),
        }
    }

    /// Add a surface to the compositor.
    ///
    /// Returns the surface's ID (for later reference).
    pub fn add_surface(&mut self, surface: Surface) -> u64 {
        let id = surface.id;
        self.surfaces.push(surface);
        self.sort_z_order();
        self.needs_composite = true;
        id
    }

    /// Remove a surface by ID.
    pub fn remove_surface(&mut self, id: u64) {
        self.surfaces.retain(|s| s.id != id);
        self.sort_z_order();
        self.needs_composite = true;
    }

    /// Get a mutable reference to a surface by ID.
    pub fn get_surface(&mut self, id: u64) -> Option<&mut Surface> {
        self.surfaces.iter_mut().find(|s| s.id == id)
    }

    /// Set the z-order of a surface.
    pub fn set_z_order(&mut self, id: u64, z_order: i32) {
        if let Some(s) = self.get_surface(id) {
            s.z_order = z_order;
            self.sort_z_order();
            self.needs_composite = true;
        }
    }

    /// Mark all surfaces as dirty (force full re-composite).
    pub fn mark_all_dirty(&mut self) {
        for s in &mut self.surfaces {
            s.dirty = true;
        }
        self.needs_composite = true;
    }

    /// Composite all surfaces into the output buffer and return stats.
    ///
    /// If `backend` is provided, calls `backend.present()` with the result.
    pub fn composite(&mut self, backend: Option<&mut dyn Backend>) -> CompositeStats {
        let mut stats = CompositeStats {
            total_surfaces: self.surfaces.len() as u32,
            output_width: self.output_width,
            output_height: self.output_height,
            ..Default::default()
        };

        // Only composite if something changed, or if cursor is visible (may have moved)
        if !self.needs_composite && !self.cursor.visible {
            return stats;
        }

        // Clear output with background color
        self.output.clear(self.background_color);

        // Composite surfaces in z-order (bottom to top)
        for &idx in &self.z_order {
            let surface = &self.surfaces[idx];
            if !surface.visible {
                continue;
            }

            stats.surfaces_composited += 1;
            if surface.dirty {
                stats.dirty_surfaces += 1;
            }

            // Determine the visible region of this surface
            let src_w = surface.buffer.width;
            let src_h = surface.buffer.height;

            // Calculate overlap with output
            let (sx, sy, sw, sh) = if surface.x < 0 {
                // Surface starts left of output — crop
                let crop_x = (-surface.x) as u32;
                (crop_x, 0u32, src_w.saturating_sub(crop_x), src_h)
            } else {
                (0, 0, src_w, src_h)
            };

            let (dx, dy) = if surface.x < 0 {
                (0u32, surface.y.max(0) as u32)
            } else {
                (surface.x as u32, surface.y.max(0) as u32)
            };

            // Final crop to output dimensions
            let final_w = sw.min(self.output_width.saturating_sub(dx));
            let final_h = sh.min(self.output_height.saturating_sub(dy));

            if final_w == 0 || final_h == 0 {
                continue;
            }

            // Blend the surface into the output
            let blend = surface.opacity < 255;

            if blend {
                // Apply surface opacity: blend with opacity
                let sx_local = sx;
                let sy_local = sy;
                for dy_off in 0..final_h {
                    for dx_off in 0..final_w {
                        let src_px = surface.buffer.get_pixel(sx_local + dx_off, sy_local + dy_off);
                        // Apply surface opacity to the source pixel's alpha
                        if src_px[3] == 0 {
                            continue;
                        }
                        let scaled_alpha = (src_px[3] as u32 * surface.opacity as u32 / 255) as u8;
                        let final_src = [src_px[0], src_px[1], src_px[2], scaled_alpha];
                        let dst_px = self.output.get_pixel(dx + dx_off, dy + dy_off);
                        self.output.set_pixel(dx + dx_off, dy + dy_off,
                            crate::pixel_buffer::alpha_blend(final_src, dst_px));
                    }
                }
            } else {
                // Fast path: no opacity adjustment
                self.output.blit_from(&surface.buffer, dx, dy, sx, sy, final_w, final_h, true);
            }
        }

        // Composite the software cursor on top of all surfaces
        if self.cursor.visible {
            self.cursor.composite(&mut self.output);
            stats.cursor_rendered = true;
            stats.cursor_x = self.cursor.x;
            stats.cursor_y = self.cursor.y;
        }

        // Clear all dirty flags
        for s in &mut self.surfaces {
            s.dirty = false;
        }
        self.needs_composite = false;

        // Present to backend if provided
        if let Some(b) = backend {
            b.present(&self.output);
        }

        stats
    }

    /// Resize the compositor's output.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.output = PixelBuffer::new(width, height);
        self.output_width = width;
        self.output_height = height;
        self.needs_composite = true;
    }

    // ── Internal ───────────────────────────────────────────────────────

    /// Sort surfaces by z-order (ascending).
    fn sort_z_order(&mut self) {
        // Sort indices directly by z-order (O(n log n), no O(n²) lookup)
        let mut indices: Vec<usize> = (0..self.surfaces.len()).collect();
        indices.sort_by_key(|&i| self.surfaces[i].z_order);
        self.z_order = indices;
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compositor_creation() {
        let comp = Compositor::new(800, 600);
        assert_eq!(comp.output_width, 800);
        assert_eq!(comp.output_height, 600);
        assert_eq!(comp.surfaces.len(), 0);
    }

    #[test]
    fn add_and_remove_surface() {
        let mut comp = Compositor::new(800, 600);
        let id = comp.add_surface(Surface::new(100, 100, 10, 10));
        assert_eq!(comp.surfaces.len(), 1);

        comp.remove_surface(id);
        assert_eq!(comp.surfaces.len(), 0);
    }

    #[test]
    fn composite_produces_output() {
        let mut comp = Compositor::new(100, 100);
        let mut surface = Surface::new_filled(50, 50, 10, 10, [0xFF, 0x00, 0x00, 0xFF]);
        surface.z_order = 1;
        comp.add_surface(surface);

        let stats = comp.composite(None);
        assert_eq!(stats.surfaces_composited, 1);

        // Check a pixel inside the surface
        let pixel = comp.output.get_pixel(10, 10);
        assert_eq!(pixel, [0xFF, 0x00, 0x00, 0xFF]);
    }

    #[test]
    fn hidden_surface_not_composited() {
        let mut comp = Compositor::new(100, 100);
        let mut surface = Surface::new(50, 50, 0, 0);
        surface.visible = false;
        comp.add_surface(surface);

        let stats = comp.composite(None);
        assert_eq!(stats.surfaces_composited, 0);
    }
}
