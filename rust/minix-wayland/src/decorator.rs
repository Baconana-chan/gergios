//! # Window Decorations — Title Bars, Borders, Buttons
//!
//! Implements Phase 4.3 of the GUI architecture: software-rendered window
//! decorations. Each window gets a title bar with close/minimize/maximize
//! buttons and a thin border.
//!
//! ## Architecture
//!
//! ```text
//! Decorator
//!   ├── decorations: Vec<DecoInfo>  (per-window decoration state)
//!   └── style: DecorationStyle      (colors, dimensions)
//!
//! For each window, a separate compositor surface is created for the
//! decoration, positioned at (window.x, window.y) with height =
//! TITLE_BAR_HEIGHT and z_order just above the window content surface.
//! ```
//!
//! ## Integration
//!
//! - On window add: `decorator.create_deco(comp, window_info)`
//! - On window move/resize: `decorator.update_deco_position(comp, window_info)`
//! - On window title change: `decorator.update_deco(comp, window_info, active)`
//! - On window remove: `decorator.remove_deco(comp, xdg_id)`
//! - On focus change: `decorator.set_active(comp, shell, active_xdg_id)`
//! - Call `decorator.render_decorations()` on tick to update dirty decos

use alloc::string::String;
use alloc::vec::Vec;

use minix_compositor::compositor::Compositor;
use minix_compositor::pixel_buffer::PixelBuffer;
use minix_compositor::surface::Surface;

/// Title bar height in pixels.
pub const TITLE_BAR_HEIGHT: i32 = 28;

/// Border width (1px line around the title bar).
pub const BORDER_WIDTH: i32 = 1;

/// Button sizes.
pub const BUTTON_SIZE: i32 = 14;
pub const BUTTON_MARGIN: i32 = 6;

/// Z-order offset: decoration surfaces are rendered at window_z + DECO_Z_OFFSET.
/// This ensures the decoration (title bar) renders on top of the window content.
pub const DECO_Z_OFFSET: i32 = 1000;

/// Color scheme for window decorations.
#[derive(Debug, Clone)]
pub struct DecorationColors {
    /// Title bar background for the active (focused) window.
    pub active_bg_a: [u8; 4],   // gradient top
    pub active_bg_b: [u8; 4],   // gradient bottom
    /// Title bar background for inactive windows.
    pub inactive_bg: [u8; 4],
    /// Border color.
    pub border: [u8; 4],
    /// Title text color.
    pub title_color: [u8; 4],
    /// Close button colors (normal, hover).
    pub close_color: [u8; 4],
    pub close_hover: [u8; 4],
    /// Minimize button colors.
    pub minimize_color: [u8; 4],
    pub minimize_hover: [u8; 4],
    /// Maximize button colors.
    pub maximize_color: [u8; 4],
    pub maximize_hover: [u8; 4],
}

impl Default for DecorationColors {
    fn default() -> Self {
        Self {
            // Active title bar: blue-gray gradient
            active_bg_a: [0x40, 0x50, 0x60, 0xFF],
            active_bg_b: [0x30, 0x40, 0x50, 0xFF],
            // Inactive: dark gray
            inactive_bg: [0x35, 0x35, 0x35, 0xFF],
            // Border: slightly lighter than bg
            border: [0x50, 0x60, 0x70, 0xFF],
            // Title text: white
            title_color: [0xE0, 0xE0, 0xE0, 0xFF],
            // Close button: red
            close_color: [0xD0, 0x30, 0x30, 0xFF],
            close_hover: [0xFF, 0x40, 0x40, 0xFF],
            // Minimize button: yellow
            minimize_color: [0xD0, 0x90, 0x20, 0xFF],
            minimize_hover: [0xF0, 0xB0, 0x30, 0xFF],
            // Maximize button: green
            maximize_color: [0x30, 0x90, 0x40, 0xFF],
            maximize_hover: [0x40, 0xC0, 0x50, 0xFF],
        }
    }
}

/// Per-window decoration tracking state.
#[derive(Debug)]
pub struct DecoInfo {
    /// The xdg_toplevel_id of the window this decoration belongs to.
    pub xdg_toplevel_id: u32,
    /// The compositor surface ID for the decoration.
    pub surface_id: u64,
    /// Whether the decoration surface needs to be re-rendered.
    pub dirty: bool,
}

/// Manages window decorations: creates, updates, and removes compositor
/// surfaces for title bars and borders.
pub struct Decorator {
    /// Per-window decoration info.
    pub decorations: Vec<DecoInfo>,
    /// Color scheme.
    pub colors: DecorationColors,
}

impl Decorator {
    /// Create a new decorator with default colors.
    pub fn new() -> Self {
        Self {
            decorations: Vec::new(),
            colors: DecorationColors::default(),
        }
    }

    /// Create a decoration surface for a window.
    ///
    /// The decoration surface is positioned at (window.x, window.y) with
    /// width = window.width and height = TITLE_BAR_HEIGHT.
    /// Its z_order is window.z_order + DECO_Z_OFFSET (above the window content).
    ///
    /// Returns the compositor surface ID of the decoration.
    pub fn create_deco(
        &mut self,
        comp: &mut Compositor,
        xdg_toplevel_id: u32,
        window_surface_id: u64,
        x: i32,
        y: i32,
        width: u32,
        z_order: i32,
        title: &str,
        is_active: bool,
    ) -> u64 {
        // Create a surface for the title bar decoration
        let mut deco_surface = Surface::new(width, TITLE_BAR_HEIGHT as u32, x, y);
        deco_surface.z_order = z_order + DECO_Z_OFFSET;

        // Render the title bar into the surface's pixel buffer
        Self::render_title_bar(
            &mut deco_surface.buffer,
            width,
            title,
            is_active,
            &self.colors,
        );

        let surface_id = comp.add_surface(deco_surface);

        self.decorations.push(DecoInfo {
            xdg_toplevel_id,
            surface_id,
            dirty: false,
        });

        surface_id
    }

    /// Update a decoration surface (re-render when title or active state changes).
    ///
    /// If `title` is None, keeps the existing title text.
    pub fn update_deco(
        &mut self,
        comp: &mut Compositor,
        xdg_toplevel_id: u32,
        title: Option<&str>,
        is_active: bool,
    ) {
        if let Some(surface_id) = self.find_surface(xdg_toplevel_id) {
            if let Some(s) = comp.get_surface(surface_id) {
                let width = s.buffer.width;
                let current_title = alloc::string::String::new(); // placeholder
                let title = title.unwrap_or(&current_title);
                Self::render_title_bar(&mut s.buffer, width, title, is_active, &self.colors);
                s.mark_dirty();
            }
        }
    }

    /// Update the position of a decoration surface (when window moves/resizes).
    pub fn update_deco_position(
        &mut self,
        comp: &mut Compositor,
        xdg_toplevel_id: u32,
        x: i32,
        y: i32,
        width: u32,
        z_order: i32,
    ) {
        if let Some(surface_id) = self.find_surface(xdg_toplevel_id) {
            if let Some(s) = comp.get_surface(surface_id) {
                s.x = x;
                s.y = y;
                s.z_order = z_order + DECO_Z_OFFSET;
                // If width changed, recreate the buffer
                if s.buffer.width != width {
                    s.buffer = PixelBuffer::new(width, TITLE_BAR_HEIGHT as u32);
                }
                s.mark_dirty();
            }
        }
    }

    /// Remove a decoration surface when its window is destroyed.
    pub fn remove_deco(&mut self, comp: &mut Compositor, xdg_toplevel_id: u32) {
        if let Some(surface_id) = self.find_surface(xdg_toplevel_id) {
            comp.remove_surface(surface_id);
        }
        self.decorations.retain(|d| d.xdg_toplevel_id != xdg_toplevel_id);
    }

    /// Update all decoration surfaces for the current shell state.
    ///
    /// Called on each tick. Re-renders dirty decorations (e.g., title changes).
    pub fn refresh(
        &mut self,
        comp: &mut Compositor,
        windows: &[crate::shell::WindowInfo],
        active_toplevel_id: Option<u32>,
    ) {
        for deco in &mut self.decorations {
            if !deco.dirty {
                continue;
            }
            // Find the window info for this decoration
            if let Some(win) = windows.iter().find(|w| w.xdg_toplevel_id == deco.xdg_toplevel_id) {
                if let Some(s) = comp.get_surface(deco.surface_id) {
                    let is_active = active_toplevel_id == Some(deco.xdg_toplevel_id);
                    Self::render_title_bar(
                        &mut s.buffer,
                        win.width as u32,
                        &win.title,
                        is_active,
                        &self.colors,
                    );
                    s.mark_dirty();
                }
            }
            deco.dirty = false;
        }
    }

    /// Find the compositor surface ID for a given xdg_toplevel_id.
    pub fn find_surface(&self, xdg_toplevel_id: u32) -> Option<u64> {
        self.decorations
            .iter()
            .find(|d| d.xdg_toplevel_id == xdg_toplevel_id)
            .map(|d| d.surface_id)
    }

    /// Mark a decoration as dirty (will be re-rendered on next refresh).
    pub fn mark_dirty(&mut self, xdg_toplevel_id: u32) {
        if let Some(d) = self.decorations.iter_mut().find(|d| d.xdg_toplevel_id == xdg_toplevel_id) {
            d.dirty = true;
        }
    }

    /// Mark all decorations as dirty (e.g., after theme change).
    pub fn mark_all_dirty(&mut self) {
        for d in &mut self.decorations {
            d.dirty = true;
        }
    }

    // ── Hit-testing ──────────────────────────────────────────────

    /// Check if a click position is on the title bar of a window.
    pub fn is_on_title_bar(click_y: i32, window_y: i32) -> bool {
        click_y >= window_y && click_y < window_y + TITLE_BAR_HEIGHT
    }

    /// Check if a click position is on the close button of a window.
    pub fn is_on_close_button(click_x: i32, click_y: i32, window_x: i32, window_y: i32, window_width: i32) -> bool {
        let btn_x = window_x + window_width - (BUTTON_MARGIN + BUTTON_SIZE);
        let btn_y = window_y + (TITLE_BAR_HEIGHT - BUTTON_SIZE) / 2;
        click_x >= btn_x && click_x < btn_x + BUTTON_SIZE
            && click_y >= btn_y && click_y < btn_y + BUTTON_SIZE
    }

    /// Check if a click position is on the minimize button.
    pub fn is_on_minimize_button(click_x: i32, click_y: i32, window_x: i32, window_y: i32, window_width: i32) -> bool {
        let btn_x = window_x + window_width - 2 * (BUTTON_MARGIN + BUTTON_SIZE) + BUTTON_MARGIN;
        let btn_y = window_y + (TITLE_BAR_HEIGHT - BUTTON_SIZE) / 2;
        click_x >= btn_x && click_x < btn_x + BUTTON_SIZE
            && click_y >= btn_y && click_y < btn_y + BUTTON_SIZE
    }

    /// Check if a click position is on the maximize button.
    pub fn is_on_maximize_button(click_x: i32, click_y: i32, window_x: i32, window_y: i32, window_width: i32) -> bool {
        let btn_x = window_x + window_width - 3 * (BUTTON_MARGIN + BUTTON_SIZE) + 2 * BUTTON_MARGIN;
        let btn_y = window_y + (TITLE_BAR_HEIGHT - BUTTON_SIZE) / 2;
        click_x >= btn_x && click_x < btn_x + BUTTON_SIZE
            && click_y >= btn_y && click_y < btn_y + BUTTON_SIZE
    }

    // ── Rendering ────────────────────────────────────────────────

    /// Render a title bar into a PixelBuffer.
    ///
    /// Draws:
    /// 1. Background gradient (active: blue-gray, inactive: dark gray)
    /// 2. Bottom border line (1px)
    /// 3. Close button (red filled rect with [X] pattern)
    /// 4. Minimize button (yellow filled rect with [−] pattern)
    /// 5. Maximize button (green filled rect with [+] pattern)
    ///
    /// Text rendering is omitted for MVP (font system complexity).
    fn render_title_bar(
        buf: &mut PixelBuffer,
        width: u32,
        title: &str,
        is_active: bool,
        colors: &DecorationColors,
    ) {
        let h = TITLE_BAR_HEIGHT as u32;

        // 1. Background
        if is_active {
            // Gradient: top to bottom
            let _ = title; // unused for MVP
            let stops = [
                minix_compositor::pixel_buffer::ColorStop {
                    position: 0.0,
                    color: colors.active_bg_a,
                },
                minix_compositor::pixel_buffer::ColorStop {
                    position: 1.0,
                    color: colors.active_bg_b,
                },
            ];
            buf.fill_linear_gradient_v(0, 0, width, h, &stops);
        } else {
            // Solid color for inactive
            buf.fill_rect(0, 0, width, h, colors.inactive_bg);
        }

        // 2. Bottom border line (1px)
        let border_y = (h - 1) as i32;
        buf.draw_line(0, border_y, width as i32 - 1, border_y, colors.border);

        // 3. Buttons (right side)
        let btn_y = (h - BUTTON_SIZE as u32) / 2;

        // Close button (rightmost)
        let close_x = width as i32 - BUTTON_MARGIN - BUTTON_SIZE;
        buf.fill_rect(
            close_x as u32, btn_y,
            BUTTON_SIZE as u32, BUTTON_SIZE as u32,
            colors.close_color,
        );
        // Draw [X] pattern on close button
        Self::draw_x_pattern(buf, close_x as u32, btn_y, BUTTON_SIZE as u32, [0xFF; 4]);

        // Minimize button (middle)
        let min_x = close_x - BUTTON_MARGIN - BUTTON_SIZE;
        buf.fill_rect(
            min_x as u32, btn_y,
            BUTTON_SIZE as u32, BUTTON_SIZE as u32,
            colors.minimize_color,
        );
        // Draw [−] pattern on minimize button
        let mid_y = btn_y + BUTTON_SIZE as u32 / 2;
        buf.draw_line(min_x + 2, mid_y as i32, min_x + BUTTON_SIZE - 2, mid_y as i32, [0xFF; 4]);

        // Maximize button (left of minimize)
        let max_x = min_x - BUTTON_MARGIN - BUTTON_SIZE;
        buf.fill_rect(
            max_x as u32, btn_y,
            BUTTON_SIZE as u32, BUTTON_SIZE as u32,
            colors.maximize_color,
        );
        // Draw [+] pattern on maximize button
        let mid_x = max_x + BUTTON_SIZE / 2;
        let mid_y = btn_y + BUTTON_SIZE as u32 / 2;
        buf.draw_line(mid_x, btn_y as i32 + 2, mid_x, (btn_y + BUTTON_SIZE as u32 - 2) as i32, [0xFF; 4]);
        buf.draw_line(max_x + 2, mid_y as i32, max_x + BUTTON_SIZE - 2, mid_y as i32, [0xFF; 4]);
    }

    /// Draw an [X] pattern on a button area.
    fn draw_x_pattern(buf: &mut PixelBuffer, x: u32, y: u32, size: u32, color: [u8; 4]) {
        let margin = 3;
        let x0 = (x + margin) as i32;
        let y0 = (y + margin) as i32;
        let x1 = (x + size - margin - 1) as i32;
        let y1 = (y + size - margin - 1) as i32;
        buf.draw_line(x0, y0, x1, y1, color);
        buf.draw_line(x1, y0, x0, y1, color);
    }
}

impl Default for Decorator {
    fn default() -> Self {
        Self::new()
    }
}

// ════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    fn make_decorator() -> Decorator {
        Decorator::new()
    }

    #[test]
    fn decorator_creation() {
        let d = make_decorator();
        assert!(d.decorations.is_empty());
    }

    #[test]
    fn create_and_remove_deco() {
        let mut d = make_decorator();
        let mut comp = Compositor::new(800, 600);

        let sid = d.create_deco(&mut comp, 1, 100, 0, 0, 200, 1, "Test", true);
        assert!(sid > 0);
        assert_eq!(d.decorations.len(), 1);
        assert_eq!(d.find_surface(1), Some(sid));

        d.remove_deco(&mut comp, 1);
        assert!(d.decorations.is_empty());
        assert!(d.find_surface(1).is_none());
    }

    #[test]
    fn title_bar_dimensions() {
        let mut d = make_decorator();
        let mut comp = Compositor::new(800, 600);

        let sid = d.create_deco(&mut comp, 1, 100, 10, 20, 400, 5, "My Window", true);
        let s = comp.get_surface(sid).unwrap();

        // Position matches window
        assert_eq!(s.x, 10);
        assert_eq!(s.y, 20);
        // Width matches window
        assert_eq!(s.buffer.width, 400);
        // Height is title bar height
        assert_eq!(s.buffer.height, TITLE_BAR_HEIGHT as u32);
        // z_order is window z + offset
        assert_eq!(s.z_order, 5 + DECO_Z_OFFSET);
        // Buffer should have data (non-zero since we rendered into it)
        assert!(s.buffer.data.iter().any(|&b| b != 0));
    }

    #[test]
    fn update_position() {
        let mut d = make_decorator();
        let mut comp = Compositor::new(800, 600);

        let sid = d.create_deco(&mut comp, 1, 100, 0, 0, 200, 1, "Test", true);
        d.update_deco_position(&mut comp, 1, 50, 60, 300, 10);

        let s = comp.get_surface(sid).unwrap();
        assert_eq!(s.x, 50);
        assert_eq!(s.y, 60);
        assert_eq!(s.buffer.width, 300);
        assert_eq!(s.z_order, 10 + DECO_Z_OFFSET);
    }

    #[test]
    fn mark_dirty_and_refresh() {
        let mut d = make_decorator();
        let mut comp = Compositor::new(800, 600);

        // Create a simple window info for refresh
        let win = crate::shell::WindowInfo {
            xdg_toplevel_id: 1,
            surface_id: 100,
            conn_idx: 0,
            title: alloc::string::String::from("Updated"),
            app_id: alloc::string::String::new(),
            x: 0, y: 0,
            width: 200, height: 150,
            z_order: 1,
            minimized: false,
            maximized: false,
            fullscreen: false,
            visible: true,
        };

        d.create_deco(&mut comp, 1, 100, 0, 0, 200, 1, "Original", true);
        assert!(!d.decorations[0].dirty);

        d.mark_dirty(1);
        assert!(d.decorations[0].dirty);

        d.refresh(&mut comp, &[win], Some(1));
        assert!(!d.decorations[0].dirty);
    }

    // ── Hit-testing tests ───────────────────────────────────────

    #[test]
    fn on_title_bar() {
        // Window at (100, 200), title bar is y=200..228
        assert!(Decorator::is_on_title_bar(210, 200));
        assert!(Decorator::is_on_title_bar(200, 200));
        assert!(Decorator::is_on_title_bar(227, 200));
        // Below title bar
        assert!(!Decorator::is_on_title_bar(228, 200));
        assert!(!Decorator::is_on_title_bar(300, 200));
        // Above window
        assert!(!Decorator::is_on_title_bar(199, 200));
    }

    #[test]
    fn on_close_button() {
        // Window at (100, 200), width=300
        // Close button is at (100 + 300 - 6 - 14, 200 + (28-14)/2) = (380, 207)
        assert!(Decorator::is_on_close_button(383, 210, 100, 200, 300));
        // Not on button
        assert!(!Decorator::is_on_close_button(100, 210, 100, 200, 300));
        assert!(!Decorator::is_on_close_button(390, 300, 100, 200, 300));
    }

    #[test]
    fn on_minimize_button() {
        // Window at (100, 200), width=300
        // Buttons right-aligned. B_M=6, B_S=14.
        // Close: x = 100+300-6-14 = 380. Range: [380, 394)
        // Min:   x = 100+300-6-2*14 = 366. Range: [366, 380)
        // Max:   x = 100+300+12-3*20 = 352. Range: [352, 366)
        assert!(Decorator::is_on_minimize_button(373, 210, 100, 200, 300));
        assert!(!Decorator::is_on_minimize_button(380, 210, 100, 200, 300)); // close, not min
        assert!(!Decorator::is_on_minimize_button(350, 210, 100, 200, 300)); // max, not min
    }

    #[test]
    fn on_maximize_button() {
        // Max button range: [352, 366) for wx=100, width=300
        assert!(Decorator::is_on_maximize_button(358, 210, 100, 200, 300));
        assert!(!Decorator::is_on_maximize_button(100, 210, 100, 200, 300));
        assert!(!Decorator::is_on_maximize_button(370, 210, 100, 200, 300)); // min, not max
    }

    // ── Rendering tests ─────────────────────────────────────────

    #[test]
    fn render_title_bar_active() {
        let mut buf = PixelBuffer::new(200, TITLE_BAR_HEIGHT as u32);
        let colors = DecorationColors::default();

        Decorator::render_title_bar(&mut buf, 200, "Active", true, &colors);

        // Background should be non-zero (active gradient)
        assert!(buf.get_pixel(10, 10) != [0; 4]);
        // Close button area should be red-ish
        let close_x = 200 - BUTTON_MARGIN as u32 - BUTTON_SIZE as u32;
        assert!(buf.get_pixel(close_x + 2, 10) != [0; 4]);
    }

    #[test]
    fn render_title_bar_inactive() {
        let mut buf = PixelBuffer::new(200, TITLE_BAR_HEIGHT as u32);
        let colors = DecorationColors::default();

        Decorator::render_title_bar(&mut buf, 200, "Inactive", false, &colors);

        // Should have data
        assert!(buf.data.iter().any(|&b| b != 0));
        // Inactive should be darker than active
        let mut active_buf = PixelBuffer::new(200, TITLE_BAR_HEIGHT as u32);
        Decorator::render_title_bar(&mut active_buf, 200, "Active", true, &colors);

        // The inactive bg is darker, so active pixels should be brighter at center
        let active_pixel = active_buf.get_pixel(20, 14);
        let inactive_pixel = buf.get_pixel(20, 14);
        assert!(active_pixel[0] >= inactive_pixel[0]); // red channel should be brighter
    }
}
