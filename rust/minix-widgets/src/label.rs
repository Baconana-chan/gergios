//! # Label Widget
//!
//! A static text label with configurable alignment, colour, and
//! optional word-wrapping.

use alloc::string::String;

use minix_compositor::font::TextAlignment;
use minix_compositor::pixel_buffer::PixelBuffer;
use minix_input::InputEvent;

use crate::widget::*;

/// A static text label.
pub struct Label {
    rect: Rect,
    text: String,
    color: [u8; 4],
    alignment: TextAlignment,
    /// If `None`, disables wrapping (single line).
    max_width: Option<u32>,
    disabled: bool,
}

impl Label {
    /// Create a new label with the given text.
    pub fn new(text: &str) -> Self {
        Self {
            rect: Rect::new(0, 0, 100, 20),
            text: text.into(),
            color: [0xE0, 0xE0, 0xE0, 0xFF],
            alignment: TextAlignment::Left,
            max_width: None,
            disabled: false,
        }
    }

    /// Set the label text.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.into();
    }

    /// Get the label text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Set the text colour.
    pub fn set_color(&mut self, color: [u8; 4]) {
        self.color = color;
    }

    /// Set the text alignment.
    pub fn set_alignment(&mut self, alignment: TextAlignment) {
        self.alignment = alignment;
    }

    /// Enable word-wrapping at the given pixel width.
    pub fn set_max_width(&mut self, width: u32) {
        self.max_width = Some(width);
    }

    /// Disable word-wrapping (single line).
    pub fn disable_wrap(&mut self) {
        self.max_width = None;
    }

    /// Set disabled state (text dimmed).
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }
}

impl Widget for Label {
    fn rect(&self) -> Rect { self.rect }

    fn set_rect(&mut self, rect: Rect) { self.rect = rect; }

    fn min_size(&self) -> (u32, u32) {
        if self.text.is_empty() {
            return (0, 20);
        }
        // Rough estimate
        let w = self.text.len() as u32 * 8;
        (w, 20)
    }

    fn handle_event(&mut self, _event: &InputEvent) -> bool {
        false // Labels don't consume events
    }

    fn render(&mut self, buf: &mut PixelBuffer, rs: &mut RenderState) {
        if self.text.is_empty() {
            return;
        }

        let color = if self.disabled {
            [0x50, 0x50, 0x60, 0xFF]
        } else {
            self.color
        };

        let max_w = self.max_width.unwrap_or(0);
        if max_w > 0 {
            // Multi-line with wrapping
            rs.font.render_text_rect(
                buf, &self.text,
                self.rect.x, self.rect.y,
                max_w, color,
                self.alignment, false,
            );
        } else {
            // Single line
            let text_w = rs.font.text_width(&self.text);
            let x = match self.alignment {
                TextAlignment::Left => self.rect.x,
                TextAlignment::Center => self.rect.x + (self.rect.w as i32 - text_w as i32) / 2,
                TextAlignment::Right => self.rect.x + self.rect.w as i32 - text_w as i32,
            };
            rs.font.render_text(buf, &self.text, x, self.rect.y, color);
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_creation() {
        let lbl = Label::new("Hello, world!");
        assert_eq!(lbl.text(), "Hello, world!");
    }

    #[test]
    fn label_does_not_consume_events() {
        let mut lbl = Label::new("Test");
        let ev = InputEvent::MouseMotion { x: 0, y: 0, dx: 0, dy: 0, modifiers: Default::default() };
        assert!(!lbl.handle_event(&ev));
    }
}
