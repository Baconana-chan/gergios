//! # minix-widgets — no_std GUI Widget Toolkit for MINIX
//!
//! A pure-Rust, software-rendered widget toolkit that runs on top of
//! `minix-compositor` (PixelBuffer + FontSystem) and `minix-input`
//! (InputEvent). Designed for `#![no_std]` environments.
//!
//! ## Architecture
//!
//! ```text
//! Window (top-level container + title bar + Wayland surface binding)
//!   └── Container (HBox / VBox / Padding / FixedSize)
//!         ├── Button (text, click, states)
//!         ├── Label (text, alignment, color)
//!         ├── Container (nesting)
//!         └── (future: TextBox, Slider, etc.)
//! ```
//!
//! Each widget implements the [`Widget`] trait:
//! - `render(buf, font, state)` — draw into PixelBuffer
//! - `handle_event(event) -> bool` — process InputEvent, return consumed
//! - `layout(rect)` — set position and size
//! - `min_size() -> (u32, u32)` — minimum dimensions
//!
//! ## Quick start
//!
//! ```no_run
//! use minix_widgets::*;
//!
//! // Create a window with a button
//! let mut win = Window::new("My App", 400, 300);
//! let mut btn = Button::new("Click me!");
//! btn.set_on_click(Box::new(|| { /* handle click */ }));
//! win.set_child(Box::new(btn));
//! ```

#![no_std]

extern crate alloc;

mod widget;
mod button;
mod label;
mod container;
mod window;

#[cfg(test)]
pub(crate) mod test_helpers;

// Re-export main types
pub use widget::*;
pub use button::*;
pub use label::*;
pub use container::*;
pub use window::*;
