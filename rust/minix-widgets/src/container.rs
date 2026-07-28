//! # Container Widgets
//!
//! Layout containers for arranging child widgets:
//!
//! - **HBox**: Horizontal box — children laid out left to right.
//! - **VBox**: Vertical box — children laid out top to bottom.
//! - **Padding**: Adds empty space (insets) around a child.
//! - **Fixed**: Gives a child a fixed size.

use alloc::boxed::Box;
use alloc::vec::Vec;

use minix_compositor::pixel_buffer::PixelBuffer;
use minix_input::InputEvent;

use crate::widget::*;

// ── HBox ─────────────────────────────────────────────────────────────────

/// Horizontal layout container. Children are arranged left-to-right.
pub struct HBox {
    rect: Rect,
    children: Vec<Box<dyn Widget>>,
    spacing: u32,
}

impl HBox {
    pub fn new(spacing: u32) -> Self {
        Self {
            rect: Rect::new(0, 0, 0, 0),
            children: Vec::new(),
            spacing,
        }
    }

    pub fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(child);
    }

    pub fn set_spacing(&mut self, spacing: u32) {
        self.spacing = spacing;
    }
}

impl Widget for HBox {
    fn rect(&self) -> Rect { self.rect }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        // Layout children horizontally
        let count = self.children.len();
        if count == 0 { return; }

        // Compute total children min width
        let total_min_w: u32 = self.children.iter()
            .map(|c| c.min_size().0)
            .sum::<u32>() + self.spacing * (count as u32 - 1);

        let available = if rect.w > total_min_w {
            rect.w
        } else {
            total_min_w
        };

        // Distribute extra width equally
        let extra_per_child = if count > 0 {
            (available.saturating_sub(total_min_w)) / count as u32
        } else {
            0
        };

        let mut x = rect.x;
        for child in &mut self.children {
            let (min_w, min_h) = child.min_size();
            let w = min_w + extra_per_child;
            let h = rect.h.max(min_h);
            child.set_rect(Rect::new(x, rect.y, w, h));
            x += w as i32 + self.spacing as i32;
        }
    }

    fn min_size(&self) -> (u32, u32) {
        let mut w = 0u32;
        let mut h = 0u32;
        for child in &self.children {
            let (cw, ch) = child.min_size();
            w += cw;
            h = h.max(ch);
        }
        if self.children.len() > 1 {
            w += self.spacing * (self.children.len() as u32 - 1);
        }
        (w, h)
    }

    fn handle_event(&mut self, event: &InputEvent) -> bool {
        for child in &mut self.children {
            if child.handle_event(event) {
                return true;
            }
        }
        false
    }

    fn render(&mut self, buf: &mut PixelBuffer, rs: &mut RenderState) {
        for child in &mut self.children {
            child.render(buf, rs);
        }
    }

    fn children(&mut self) -> &mut [Box<dyn Widget>] {
        &mut self.children
    }
}

// ── VBox ─────────────────────────────────────────────────────────────────

/// Vertical layout container. Children are arranged top-to-bottom.
pub struct VBox {
    rect: Rect,
    children: Vec<Box<dyn Widget>>,
    spacing: u32,
}

impl VBox {
    pub fn new(spacing: u32) -> Self {
        Self {
            rect: Rect::new(0, 0, 0, 0),
            children: Vec::new(),
            spacing,
        }
    }

    pub fn add_child(&mut self, child: Box<dyn Widget>) {
        self.children.push(child);
    }

    pub fn set_spacing(&mut self, spacing: u32) {
        self.spacing = spacing;
    }
}

impl Widget for VBox {
    fn rect(&self) -> Rect { self.rect }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        let count = self.children.len();
        if count == 0 { return; }

        let total_min_h: u32 = self.children.iter()
            .map(|c| c.min_size().1)
            .sum::<u32>() + self.spacing * (count as u32 - 1);

        let available = if rect.h > total_min_h {
            rect.h
        } else {
            total_min_h
        };

        let extra_per_child = if count > 0 {
            (available.saturating_sub(total_min_h)) / count as u32
        } else {
            0
        };

        let mut y = rect.y;
        for child in &mut self.children {
            let (min_w, min_h) = child.min_size();
            let w = rect.w.max(min_w);
            let h = min_h + extra_per_child;
            child.set_rect(Rect::new(rect.x, y, w, h));
            y += h as i32 + self.spacing as i32;
        }
    }

    fn min_size(&self) -> (u32, u32) {
        let mut w = 0u32;
        let mut h = 0u32;
        for child in &self.children {
            let (cw, ch) = child.min_size();
            w = w.max(cw);
            h += ch;
        }
        if self.children.len() > 1 {
            h += self.spacing * (self.children.len() as u32 - 1);
        }
        (w, h)
    }

    fn handle_event(&mut self, event: &InputEvent) -> bool {
        for child in &mut self.children {
            if child.handle_event(event) {
                return true;
            }
        }
        false
    }

    fn render(&mut self, buf: &mut PixelBuffer, rs: &mut RenderState) {
        for child in &mut self.children {
            child.render(buf, rs);
        }
    }

    fn children(&mut self) -> &mut [Box<dyn Widget>] {
        &mut self.children
    }
}

// ── Padding ──────────────────────────────────────────────────────────────

/// Adds empty space (padding/insets) around a single child widget.
pub struct Padding {
    rect: Rect,
    child: Option<Box<dyn Widget>>,
    left: u32,
    right: u32,
    top: u32,
    bottom: u32,
}

impl Padding {
    pub fn new(child: Box<dyn Widget>, pad: u32) -> Self {
        Self::with_insets(child, pad, pad, pad, pad)
    }

    pub fn with_insets(child: Box<dyn Widget>, left: u32, right: u32, top: u32, bottom: u32) -> Self {
        Self {
            rect: Rect::new(0, 0, 0, 0),
            child: Some(child),
            left, right, top, bottom,
        }
    }
}

impl Widget for Padding {
    fn rect(&self) -> Rect { self.rect }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        if let Some(ref mut child) = self.child {
            child.set_rect(Rect::new(
                rect.x + self.left as i32,
                rect.y + self.top as i32,
                rect.w.saturating_sub(self.left + self.right),
                rect.h.saturating_sub(self.top + self.bottom),
            ));
        }
    }

    fn min_size(&self) -> (u32, u32) {
        let (cw, ch) = self.child.as_ref()
            .map(|c| c.min_size())
            .unwrap_or((0, 0));
        (cw + self.left + self.right, ch + self.top + self.bottom)
    }

    fn handle_event(&mut self, event: &InputEvent) -> bool {
        self.child.as_mut().map(|c| c.handle_event(event)).unwrap_or(false)
    }

    fn render(&mut self, buf: &mut PixelBuffer, rs: &mut RenderState) {
        if let Some(ref mut child) = self.child {
            child.render(buf, rs);
        }
    }

    fn children(&mut self) -> &mut [Box<dyn Widget>] {
        self.child.as_mut().map(|c| {
            // Return a mutable slice of one element
            let ptr: *mut Box<dyn Widget> = c;
            unsafe { core::slice::from_raw_parts_mut(ptr, 1) }
        }).unwrap_or(&mut [])
    }
}

// ── FixedSize ────────────────────────────────────────────────────────────

/// Forces a child widget to have a fixed width and height.
pub struct FixedSize {
    rect: Rect,
    child: Option<Box<dyn Widget>>,
    fixed_w: u32,
    fixed_h: u32,
}

impl FixedSize {
    pub fn new(child: Box<dyn Widget>, w: u32, h: u32) -> Self {
        Self {
            rect: Rect::new(0, 0, w, h),
            child: Some(child),
            fixed_w: w,
            fixed_h: h,
        }
    }
}

impl Widget for FixedSize {
    fn rect(&self) -> Rect { self.rect }

    fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
        if let Some(ref mut child) = self.child {
            child.set_rect(Rect::new(
                rect.x + (rect.w as i32 - self.fixed_w as i32) / 2,
                rect.y + (rect.h as i32 - self.fixed_h as i32) / 2,
                self.fixed_w, self.fixed_h,
            ));
        }
    }

    fn min_size(&self) -> (u32, u32) { (self.fixed_w, self.fixed_h) }

    fn handle_event(&mut self, event: &InputEvent) -> bool {
        self.child.as_mut().map(|c| c.handle_event(event)).unwrap_or(false)
    }

    fn render(&mut self, buf: &mut PixelBuffer, rs: &mut RenderState) {
        if let Some(ref mut child) = self.child {
            child.render(buf, rs);
        }
    }

    fn children(&mut self) -> &mut [Box<dyn Widget>] {
        self.child.as_mut().map(|c| {
            let ptr: *mut Box<dyn Widget> = c;
            unsafe { core::slice::from_raw_parts_mut(ptr, 1) }
        }).unwrap_or(&mut [])
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // A simple test widget for layout tests
    struct DummyWidget {
        rect: Rect,
        min_w: u32,
        min_h: u32,
    }

    impl Widget for DummyWidget {
        fn rect(&self) -> Rect { self.rect }
        fn set_rect(&mut self, r: Rect) { self.rect = r; }
        fn min_size(&self) -> (u32, u32) { (self.min_w, self.min_h) }
        fn handle_event(&mut self, _: &InputEvent) -> bool { false }
        fn render(&mut self, _: &mut PixelBuffer, _: &mut RenderState) {}
    }

    fn dummy(w: u32, h: u32) -> Box<dyn Widget> {
        Box::new(DummyWidget { rect: Rect::new(0, 0, 0, 0), min_w: w, min_h: h })
    }

    #[test]
    fn hbox_layout() {
        let mut hbox = HBox::new(4);
        hbox.add_child(dummy(50, 20));
        hbox.add_child(dummy(80, 30));
        let (w, h) = hbox.min_size();
        assert_eq!(w, 50 + 4 + 80);
        assert_eq!(h, 30);
    }

    #[test]
    fn vbox_layout() {
        let mut vbox = VBox::new(4);
        vbox.add_child(dummy(50, 20));
        vbox.add_child(dummy(80, 30));
        let (w, h) = vbox.min_size();
        assert_eq!(w, 80);
        assert_eq!(h, 20 + 4 + 30);
    }

    #[test]
    fn padding_layout() {
        let p = Padding::new(dummy(50, 20), 10);
        let (w, h) = p.min_size();
        assert_eq!(w, 50 + 10 + 10); // left + right
        assert_eq!(h, 20 + 10 + 10); // top + bottom
    }

    #[test]
    fn fixed_size_layout() {
        let f = FixedSize::new(dummy(30, 15), 100, 50);
        let (w, h) = f.min_size();
        assert_eq!(w, 100);
        assert_eq!(h, 50);
    }
}
