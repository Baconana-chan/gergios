//! # Widget Trait and Base Types
//!
//! Defines the core [`Widget`] trait that all widgets implement, plus
//! shared types for layout, events, rendering state, and styling.

use alloc::boxed::Box;
use alloc::vec::Vec;

use minix_compositor::font::FontSystem;
use minix_compositor::pixel_buffer::PixelBuffer;
use minix_input::InputEvent;

// ── Geometry ──────────────────────────────────────────────────────────────

/// A 2D rectangle (position + size).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// Check if point `(px, py)` is inside this rectangle.
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y
            && px < self.x + self.w as i32
            && py < self.y + self.h as i32
    }

    /// Return the intersection with another rectangle.
    pub fn intersect(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let r = (self.x + self.w as i32).min(other.x + other.w as i32);
        let b = (self.y + self.h as i32).min(other.y + other.h as i32);
        if x < r && y < b {
            Some(Rect::new(x, y, (r - x) as u32, (b - y) as u32))
        } else {
            None
        }
    }
}

// ── Widget state machine ─────────────────────────────────────────────────

/// Interactive state of a widget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetState {
    /// Normal, idle state.
    Normal,
    /// Pointer is hovering over the widget.
    Hover,
    /// Widget is being pressed/activated.
    Pressed,
    /// Widget is disabled (can't interact).
    Disabled,
}

impl WidgetState {
    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }
}

// ── Theme / style colours ────────────────────────────────────────────────

/// Colour palette for widgets. All values are RGBA `[u8; 4]`.
#[derive(Clone)]
pub struct Theme {
    /// Background colour (panel bg, window bg).
    pub bg: [u8; 4],
    /// Foreground / text colour.
    pub fg: [u8; 4],
    /// Accent colour (buttons, active elements).
    pub accent: [u8; 4],
    /// Hover colour (lighter accent).
    pub accent_hover: [u8; 4],
    /// Pressed colour (darker accent).
    pub accent_pressed: [u8; 4],
    /// Disabled colour (dimmed).
    pub disabled: [u8; 4],
    /// Border colour.
    pub border: [u8; 4],
    /// Text input background (TextBox).
    pub input_bg: [u8; 4],
    /// Selection colour (TextBox selected text).
    pub selection: [u8; 4],
    /// Cursor colour.
    pub cursor: [u8; 4],
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg: [0x1E, 0x1E, 0x2E, 0xFF],
            fg: [0xE0, 0xE0, 0xE0, 0xFF],
            accent: [0x40, 0x80, 0xD0, 0xFF],
            accent_hover: [0x50, 0x90, 0xE0, 0xFF],
            accent_pressed: [0x30, 0x70, 0xC0, 0xFF],
            disabled: [0x50, 0x50, 0x60, 0xFF],
            border: [0x40, 0x40, 0x50, 0xFF],
            input_bg: [0x14, 0x14, 0x20, 0xFF],
            selection: [0x40, 0x60, 0xA0, 0x40],
            cursor: [0xC0, 0xC0, 0xC0, 0xFF],
        }
    }
}

// ── Render state (per-frame) ─────────────────────────────────────────────

/// Per-frame rendering context passed to all widgets.
pub struct RenderState<'a> {
    /// Font system for text rendering.
    pub font: &'a mut FontSystem,
    /// Theme colours.
    pub theme: &'a Theme,
    /// Clip rectangle (widget should not draw outside this region).
    pub clip: Option<Rect>,
    /// Global tick count for animations.
    pub tick: u64,
}

// ── Widget trait ─────────────────────────────────────────────────────────

/// The core trait all UI widgets must implement.
///
/// Widgets are arranged in a tree hierarchy: a `Window` contains containers
/// which contain buttons, labels, etc.
pub trait Widget {
    /// Get the widget's bounding rectangle.
    fn rect(&self) -> Rect;

    /// Set the widget's position and size (layout pass).
    fn set_rect(&mut self, rect: Rect);

    /// Get the minimum size this widget needs.
    fn min_size(&self) -> (u32, u32);

    /// Handle an input event. Return `true` if the event was consumed.
    fn handle_event(&mut self, event: &InputEvent) -> bool;

    /// Render the widget into the pixel buffer.
    ///
    /// The buffer should be pre-clipped to the widget's rectangle.
    /// Uses `rs.font` for text rendering.
    fn render(&mut self, buf: &mut PixelBuffer, rs: &mut RenderState);

    /// Get the current interactive state.
    fn state(&self) -> WidgetState {
        WidgetState::Normal
    }

    /// Get child widgets (for event dispatch and layout traversal).
    fn children(&mut self) -> &mut [Box<dyn Widget>] {
        &mut []
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────



// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_contains() {
        let r = Rect::new(10, 10, 100, 50);
        assert!(r.contains(10, 10));
        assert!(r.contains(109, 59));
        assert!(!r.contains(110, 60));
        assert!(!r.contains(9, 10));
    }

    #[test]
    fn rect_intersect() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(50, 50, 100, 100);
        let i = a.intersect(&b).unwrap();
        assert_eq!(i, Rect::new(50, 50, 50, 50));

        let c = Rect::new(200, 200, 10, 10);
        assert!(a.intersect(&c).is_none());
    }

    #[test]
    fn theme_default() {
        let t = Theme::default();
        assert_eq!(t.fg, [0xE0, 0xE0, 0xE0, 0xFF]);
        assert_eq!(t.bg, [0x1E, 0x1E, 0x2E, 0xFF]);
    }
}
