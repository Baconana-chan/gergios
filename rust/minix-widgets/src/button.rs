//! # Button Widget
//!
//! A clickable button with text label. Supports four visual states:
//! `Normal`, `Hover`, `Pressed`, `Disabled`.

use alloc::boxed::Box;
use alloc::string::String;

use minix_compositor::pixel_buffer::PixelBuffer;

use minix_input::{InputEvent, MouseButton};

use crate::widget::*;

/// A clickable button with a text label.
pub struct Button {
    rect: Rect,
    text: String,
    state: WidgetState,
    theme: Option<Theme>,
    on_click: Option<Box<dyn FnMut()>>,
}

impl Button {
    /// Create a new button with the given label text.
    pub fn new(text: &str) -> Self {
        Self {
            rect: Rect::new(0, 0, 100, 28),
            text: text.into(),
            state: WidgetState::Normal,
            theme: None,
            on_click: None,
        }
    }

    /// Set the button's label text.
    pub fn set_text(&mut self, text: &str) {
        self.text = text.into();
    }

    /// Get the button's label text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Set a custom theme for this button.
    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = Some(theme);
    }

    /// Set the click callback.
    pub fn set_on_click(&mut self, f: Box<dyn FnMut()>) {
        self.on_click = Some(f);
    }

    /// Set whether the button is enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        if !enabled {
            self.state = WidgetState::Disabled;
        } else if self.state == WidgetState::Disabled {
            self.state = WidgetState::Normal;
        }
    }

    fn get_colors(&self, theme: &Theme) -> ([u8; 4], [u8; 4]) {
        let t = self.theme.as_ref().unwrap_or(theme);
        match self.state {
            WidgetState::Normal => (t.accent, t.fg),
            WidgetState::Hover => (t.accent_hover, t.fg),
            WidgetState::Pressed => (t.accent_pressed, t.fg),
            WidgetState::Disabled => (t.disabled, t.disabled),
        }
    }
}

impl Widget for Button {
    fn rect(&self) -> Rect { self.rect }

    fn set_rect(&mut self, rect: Rect) { self.rect = rect; }

    fn min_size(&self) -> (u32, u32) {
        // Rough text size estimate: ~8px per char, plus padding
        let text_w = self.text.len() as u32 * 8 + 16;
        (text_w.max(48), 28)
    }

    fn handle_event(&mut self, event: &InputEvent) -> bool {
        if self.state == WidgetState::Disabled {
            return false;
        }

        match event {
            InputEvent::MouseMotion { x, y, .. } => {
                let was_hover = self.state == WidgetState::Hover || self.state == WidgetState::Pressed;
                let is_hover = self.rect.contains(*x, *y);
                if is_hover && !was_hover {
                    self.state = WidgetState::Hover;
                } else if !is_hover && was_hover {
                    self.state = WidgetState::Normal;
                }
                true
            }
            InputEvent::MouseButton { button: MouseButton::Left, pressed: true, x, y, .. } => {
                if self.rect.contains(*x, *y) {
                    self.state = WidgetState::Pressed;
                    true
                } else {
                    false
                }
            }
            InputEvent::MouseButton { button: MouseButton::Left, pressed: false, x, y, .. } => {
                if self.state == WidgetState::Pressed {
                    if self.rect.contains(*x, *y) {
                        if let Some(ref mut cb) = self.on_click {
                            cb();
                        }
                    }
                    self.state = WidgetState::Hover;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn render(&mut self, buf: &mut PixelBuffer, rs: &mut RenderState) {
        let theme = rs.theme;
        let (bg, fg) = self.get_colors(theme);

        // Draw rounded rect background
        let r = 4;
        buf.fill_rounded_rect(
            self.rect.x as u32, self.rect.y as u32,
            self.rect.w, self.rect.h, r, bg,
        );

        // Draw border
        let border_color = if self.state == WidgetState::Disabled {
            theme.disabled
        } else {
            theme.border
        };
        buf.draw_line(self.rect.x, self.rect.y,
            self.rect.x + self.rect.w as i32 - 1, self.rect.y, border_color);
        buf.draw_line(self.rect.x, self.rect.y + self.rect.h as i32 - 1,
            self.rect.x + self.rect.w as i32 - 1, self.rect.y + self.rect.h as i32 - 1, border_color);

        // Draw text centered
        let text_w = rs.font.text_width(&self.text);
        let text_x = self.rect.x + (self.rect.w as i32 - text_w as i32) / 2;
        let text_y = self.rect.y + ((self.rect.h as i32).saturating_sub(rs.font.line_height as i32)) / 2;

        if self.state == WidgetState::Pressed {
            // Slight shift on press
            rs.font.render_text(buf, &self.text, text_x + 1, text_y + 1, fg);
        } else {
            rs.font.render_text(buf, &self.text, text_x, text_y, fg);
        }
    }

    fn state(&self) -> WidgetState { self.state }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use minix_compositor::font::FontSystem;
    use minix_compositor::pixel_buffer::PixelBuffer;

    fn make_font() -> FontSystem {
        // Use the minimal TTF built in the font module tests
        let ttf = crate::test_helpers::minimal_ttf();
        FontSystem::from_data(&ttf, 14).unwrap()
    }

    #[test]
    fn button_creation() {
        let btn = Button::new("OK");
        assert_eq!(btn.text(), "OK");
        assert_eq!(btn.state(), WidgetState::Normal);
    }

    #[test]
    fn button_hover_change() {
        let mut btn = Button::new("Test");
        btn.set_rect(Rect::new(0, 0, 100, 28));

        // Mouse enter
        let ev = InputEvent::MouseMotion { x: 50, y: 14, dx: 0, dy: 0, modifiers: Default::default() };
        assert!(btn.handle_event(&ev));
        assert_eq!(btn.state(), WidgetState::Hover);

        // Mouse leave
        let ev = InputEvent::MouseMotion { x: 200, y: 14, dx: 0, dy: 0, modifiers: Default::default() };
        assert!(btn.handle_event(&ev));
        assert_eq!(btn.state(), WidgetState::Normal);
    }

    #[test]
    fn button_click() {
        use alloc::rc::Rc;

        let mut btn = Button::new("Click");
        btn.set_rect(Rect::new(0, 0, 100, 28));

        let clicked = Rc::new(core::cell::Cell::new(false));
        let c = clicked.clone();
        btn.set_on_click(Box::new(move || { c.set(true); }));

        // Press
        let ev = InputEvent::MouseButton {
            button: MouseButton::Left, pressed: true,
            x: 50, y: 14, modifiers: Default::default(),
        };
        assert!(btn.handle_event(&ev));
        assert_eq!(btn.state(), WidgetState::Pressed);

        // Release inside
        let ev = InputEvent::MouseButton {
            button: MouseButton::Left, pressed: false,
            x: 50, y: 14, modifiers: Default::default(),
        };
        assert!(btn.handle_event(&ev));
        assert!(clicked.get());
    }

    #[test]
    fn button_disabled() {
        let mut btn = Button::new("Disabled");
        btn.set_enabled(false);
        assert_eq!(btn.state(), WidgetState::Disabled);

        let ev = InputEvent::MouseMotion { x: 50, y: 14, dx: 0, dy: 0, modifiers: Default::default() };
        assert!(!btn.handle_event(&ev)); // not consumed
    }

    #[test]
    fn button_min_size() {
        let btn = Button::new("Hello");
        let (w, h) = btn.min_size();
        assert!(w >= 48);
        assert_eq!(h, 28);
    }
}
