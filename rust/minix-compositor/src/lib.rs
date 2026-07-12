//! # minix-compositor — MINIX Wayland Compositor
//!
//! A pure-Rust compositor for MINIX, providing:
//!
//! - **PixelBuffer**: Software RGBA framebuffer with alpha blending,
//!   fills, and blits. The fundamental rendering primitive.
//! - **FontSystem**: TTF font loading, text shaping via `rustybuzz`,
//!   and glyph rasterization with caching.
//! - **Surface**: A drawable region (rectangle) with an attached pixel
//!   buffer, position, and z-order — the building block of a window.
//! - **Compositor**: Manages a list of surfaces, composites them in
//!   z-order, and calls an output callback to present the final image.
//! - **Backend**: Abstraction over the display hardware. Currently
//!   provides an in-memory backend (for testing / PNG output) and a
//!   stub for the future MINIX framebuffer backend.
//!
//! ## Architecture
//!
//! ```text
//! Application (manages its own surfaces)
//!     │
//!     ▼
//! Compositor (z-order, dirty tracking, compositing)
//!     │
//!     ├── Surface (pixel buffer + position + z-order)
//!     │       └── PixelBuffer (RGBA 32bpp, blend, fill, blit)
//!     │
//!     └── Output (Backend trait)
//!             ├── MemBackend (in-memory, PNG export)
//!             └── FBBackend (MINIX framebuffer — not yet implemented)
//! ```
//!
//! ## Getting started
//!
//! ```no_run
//! use minix_compositor::prelude::*;
//! use minix_compositor::PixelBuffer;
//!
//! // Create a 800x600 RGBA buffer
//! let mut fb = PixelBuffer::new(800, 600);
//!
//! // Fill with a solid color
//! fb.clear([0x20, 0x30, 0x40, 0xFF]);
//!
//! // Draw a red rectangle
//! fb.fill_rect(100, 100, 200, 150, [0xFF, 0x00, 0x00, 0xFF]);
//! ```

#![no_std]

extern crate alloc;

pub mod pixel_buffer;
pub mod font;
pub mod surface;
pub mod compositor;
pub mod backend;

/// Re-exports for convenient `use minix_compositor::prelude::*`.
pub mod prelude {
    pub use crate::pixel_buffer::*;
    pub use crate::font::*;
    pub use crate::surface::*;
    pub use crate::compositor::*;
    pub use crate::backend::*;
}
