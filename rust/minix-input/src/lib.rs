//! # minix-input — Input subsystem for the MINIX compositor
//!
//! Provides keyboard and mouse input processing for the MINIX GUI stack.
//!
//! ## Architecture
//!
//! ```text
//! Hardware (USB HID) → xHCI driver → /dev/kbd0, /dev/mouse0 (chardev)
//!     ↓
//! InputSource trait (poll for events)
//!     ↓
//! KeyboardState / MouseState trackers (delta → absolute, press/release)
//!     ↓
//! InputEvent enum (high-level: key symbol, mouse motion, button, wheel)
//!     ↓
//! EventLoop (polls sources, composites, presents)
//!     ↓
//! Compositor (dispatches events to focused surface)
//! ```
//!
//! ## Quick start
//!
//! ```no_run
//! use minix_input::{InputSource, SimInputSource, KeyboardState, MouseState, InputEvent, KeySymbol, MouseButton};
//!
//! // Create a simulated input source
//! let mut source = SimInputSource::new();
//! source.move_mouse(400, 300);
//! source.click_mouse(MouseButton::Left, 400, 300);
//!
//! // Poll for events
//! let events = source.poll();
//! for event in &events {
//!     match event {
//!         InputEvent::MouseMotion { x, y, .. } => {
//!             // Update cursor position
//!         }
//!         InputEvent::MouseButton { button, pressed, .. } => {
//!             // Handle click
//!         }
//!         _ => {}
//!     }
//! }
//! ```

#![no_std]

extern crate alloc;

pub mod events;
pub mod source;
pub mod event_loop;

// Re-export the most common types at the crate root.
pub use events::*;
pub use source::*;
pub use event_loop::*;
