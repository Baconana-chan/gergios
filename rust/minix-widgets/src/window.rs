//! # Window Widget
//!
//! A top-level window container. Manages a title bar, client area,
//! and close/minimize/maximize buttons. Renders into a compositor
//! `Surface` and interacts with the Wayland event system.
//!
//! ## Usage
//!
//! ```no_run
//! use minix_widgets::*;
//!
//! let mut window = Window::new("Calculator", 300, 400);
//! let mut btn = Button::new("Click");
//! btn.set_on_click(Box::new(|| { /* ... */ }));
//! window.set_child(Box::new(btn));
//! ```

use alloc::boxed::Box;
use alloc::string::String;
use core::cell::Cell;

use minix_compositor::pixel_buffer::PixelBuffer;
use minix_input::{InputEvent, MouseButton};

use minix_compositor::font::TextAlignment;

use crate::widget::*;
use crate::button::Button;
use crate::label::Label;
use crate::container::{HBox, VBox, Padding};

/// Title bar height in pixels.
pub const TITLE_BAR_HEIGHT: u32 = 24;

/// Minimum window width.
pub const MIN_WINDOW_WIDTH: u32 = 160;

/// Minimum window height.
pub const MIN_WINDOW_HEIGHT: u32 = 80;

/// A top-level window with a title bar and client content area.
pub struct Window {
    rect: Rect,
    title: String,
    content: Option<Box<dyn Widget>>,
    /// Surface ID in the compositor (set when rendering).
    pub surface_id: u64,
    title_dragging: Cell<bool>,
    drag_start_x: Cell<i32>,
    drag_start_y: Cell<i32>,
    /// Cached output size for title bar rendering.
    output_width: u32,
}

impl Window {
    /// Create a new window with the given title and initial size.
    pub fn new(title: &str, width: u32, height: u32) -> Self {
        Self {
            rect: Rect::new(0, 0, width.max(MIN_WINDOW_WIDTH), height.max(MIN_WINDOW_HEIGHT)),
            title: title.into(),
            content: None,
            surface_id: 0,
            title_dragging: Cell::new(false),
            drag_start_x: Cell::new(0),
            drag_start_y: Cell::new(0),
            output_width: width.max(MIN_WINDOW_WIDTH),
        }
    }

    /// Set the child widget (the window's content area).
    pub fn set_child(&mut self, child: Box<dyn Widget>) {
        self.content = Some(child);
        self.layout_content();
    }

    /// Set the window title.
    pub fn set_title(&mut self, title: &str) {
        self.title = title.into();
    }

    /// Get the window title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Get the content area rectangle (below the title bar).
    pub fn content_rect(&self) -> Rect {
        Rect::new(
            self.rect.x,
            self.rect.y + TITLE_BAR_HEIGHT as i32,
            self.rect.w,
            self.rect.h.saturating_sub(TITLE_BAR_HEIGHT),
        )
    }

    fn layout_content(&mut self) {
        let cr = self.content_rect();
        if let Some(ref mut child) = self.content {
            child.set_rect(cr);
        }
    }

    /// Handle a title bar click (like the compositor's handle_title_bar_click).
    /// Returns true if the event was consumed.
    pub fn handle_title_bar_event(&self, event: &InputEvent) -> bool {
        match event {
            InputEvent::MouseButton {
                button: MouseButton::Left, pressed: true,
                x, y, ..
            } => {
                // Check if click is on title bar (top 24px of window rect)
                if *x >= self.rect.x && *x <= self.rect.x + self.rect.w as i32
                    && *y >= self.rect.y && *y < self.rect.y + TITLE_BAR_HEIGHT as i32
                {
                    self.title_dragging.set(true);
                    self.drag_start_x.set(*x);
                    self.drag_start_y.set(*y);
                    return true;
                }
                false
            }
            InputEvent::MouseButton {
                button: MouseButton::Left, pressed: false, ..
            } => {
                if self.title_dragging.get() {
                    self.title_dragging.set(false);
                    return true;
                }
                false
            }
            InputEvent::MouseMotion { x, y, .. } => {
                if self.title_dragging.get() {
                    // Calculate new position
                    let dx = x - self.drag_start_x.get();
                    let dy = y - self.drag_start_y.get();
                    // Update window position (handled by the compositor in practice)
                    return true;
                }
                false
            }
            _ => false,
        }
    }
}

impl Widget for Window {
    fn rect(&self) -> Rect { self.rect }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        self.layout_content();
    }

    fn min_size(&self) -> (u32, u32) {
        let (cw, ch) = self.content.as_ref()
            .map(|c| c.min_size())
            .unwrap_or((100, 60));
        (cw.max(MIN_WINDOW_WIDTH), ch + TITLE_BAR_HEIGHT)
    }

    fn handle_event(&mut self, event: &InputEvent) -> bool {
        // First try the content area
        if let Some(ref mut child) = self.content {
            if child.handle_event(event) {
                return true;
            }
        }
        // Then try title bar
        self.handle_title_bar_event(event)
    }

    fn render(&mut self, buf: &mut PixelBuffer, rs: &mut RenderState) {
        // ── Title bar ────────────────────────────────────────────────
        let title_bg = [0x22, 0x22, 0x38, 0xFF];
        let title_fg = [0xC0, 0xC0, 0xC0, 0xFF];
        let title_bar_rect = Rect::new(
            self.rect.x, self.rect.y,
            self.rect.w, TITLE_BAR_HEIGHT,
        );

        // Draw title bar background
        buf.fill_rect(
            title_bar_rect.x as u32, title_bar_rect.y as u32,
            title_bar_rect.w, title_bar_rect.h,
            title_bg,
        );

        // Draw bottom border of title bar
        let bottom = title_bar_rect.y + title_bar_rect.h as i32 - 1;
        buf.draw_line(
            title_bar_rect.x, bottom,
            title_bar_rect.x + title_bar_rect.w as i32 - 1, bottom,
            [0x40, 0x40, 0x60, 0xFF],
        );

        // Draw title text
        if !self.title.is_empty() {
            rs.font.render_text(
                buf, &self.title,
                title_bar_rect.x + 6,
                title_bar_rect.y + (TITLE_BAR_HEIGHT as i32 - rs.font.line_height as i32) / 2,
                title_fg,
            );
        }

        // ── Content area ─────────────────────────────────────────────
        // Fill content area background (compute BEFORE mutable borrow)
        let cr = self.content_rect();
        buf.fill_rect(cr.x as u32, cr.y as u32, cr.w, cr.h, rs.theme.bg);
        // Render child widget
        if let Some(ref mut child) = self.content {
            child.render(buf, rs);
        }
    }

    fn children(&mut self) -> &mut [Box<dyn Widget>] {
        self.content.as_mut().map(|c| {
            let ptr: *mut Box<dyn Widget> = c;
            unsafe { core::slice::from_raw_parts_mut(ptr, 1) }
        }).unwrap_or(&mut [])
    }
}

// ── Demo: Calculator app ────────────────────────────────────────────────

/// A simple calculator application built from widgets.
/// Returns the root widget (Window with content) and the render function.
pub struct CalculatorApp {
    pub window: Window,
    display_text: String,
    pending_op: Option<(char, i32)>,
    clear_display: bool,
}

impl CalculatorApp {
    pub fn new() -> Self {
        let mut window = Window::new("Calculator", 240, 300);

        let display_text = String::from("0");
        let mut app = Self {
            window,
            display_text,
            pending_op: None,
            clear_display: false,
        };

        app.build_ui();
        app
    }

    fn build_ui(&mut self) {
        let mut vbox = VBox::new(4);

        // Display area (placeholder — uses a label)
        let display_text = self.display_text.clone();
        let mut display = Label::new(&display_text);
        display.set_alignment(TextAlignment::Right);
        display.set_color([0x80, 0xE0, 0x80, 0xFF]); // green monochrome style
        display.set_max_width(220);

        // Wrap display in a styled container
        let display_padded = Padding::new(Box::new(display), 8);

        vbox.add_child(Box::new(display_padded));

        // Row 1: 7 8 9 /
        let mut row1 = HBox::new(4);
        for ch in &["7", "8", "9", "/"] {
            let mut btn = Button::new(*ch);
            btn.set_on_click(Box::new(|| {}));
            row1.add_child(Box::new(btn));
        }
        vbox.add_child(Box::new(row1));

        // Row 2: 4 5 6 *
        let mut row2 = HBox::new(4);
        for ch in &["4", "5", "6", "*"] {
            let mut btn = Button::new(*ch);
            btn.set_on_click(Box::new(|| {}));
            row2.add_child(Box::new(btn));
        }
        vbox.add_child(Box::new(row2));

        // Row 3: 1 2 3 -
        let mut row3 = HBox::new(4);
        for ch in &["1", "2", "3", "-"] {
            let mut btn = Button::new(*ch);
            btn.set_on_click(Box::new(|| {}));
            row3.add_child(Box::new(btn));
        }
        vbox.add_child(Box::new(row3));

        // Row 4: 0 . = +
        let mut row4 = HBox::new(4);
        for ch in &["0", ".", "=", "+"] {
            let mut btn = Button::new(*ch);
            btn.set_on_click(Box::new(|| {}));
            row4.add_child(Box::new(btn));
        }
        vbox.add_child(Box::new(row4));

        // Add a clear button row
        let mut clear_row = HBox::new(4);
        let mut clear_btn = Button::new("C");
        clear_btn.set_on_click(Box::new(|| {}));
        clear_row.add_child(Box::new(clear_btn));
        let mut eq_btn = Button::new("=");
        eq_btn.set_on_click(Box::new(|| {}));
        clear_row.add_child(Box::new(eq_btn));
        vbox.add_child(Box::new(clear_row));

        let padded = Padding::new(Box::new(vbox), 8);
        self.window.set_child(Box::new(padded));
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use minix_compositor::pixel_buffer::PixelBuffer;

    #[test]
    fn window_creation() {
        let win = Window::new("Test", 300, 200);
        assert_eq!(win.title(), "Test");
        assert!(win.rect.w >= 160);
        assert!(win.rect.h >= 80);
    }

    #[test]
    fn window_content_rect() {
        let win = Window::new("Test", 300, 200);
        let cr = win.content_rect();
        assert_eq!(cr.x, 0);
        assert_eq!(cr.y, TITLE_BAR_HEIGHT as i32);
        assert_eq!(cr.w, 300);
        assert_eq!(cr.h, 200 - TITLE_BAR_HEIGHT);
    }

    #[test]
    fn calculator_creation() {
        let calc = CalculatorApp::new();
        assert_eq!(calc.window.title(), "Calculator");
    }
}
