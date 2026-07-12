//! # Backend — Display backend abstraction
//!
//! The `Backend` trait abstracts over the display hardware. The compositor
//! calls `present()` with the final rendered output buffer, and the backend
//! is responsible for getting those pixels onto the screen.
//!
//! Currently provides:
//! - **MemBackend**: In-memory buffer (for testing / PNG export)
//! - **FBBackend**: MINIX framebuffer (stub, to be implemented)

use crate::pixel_buffer::PixelBuffer;

/// Trait for display backends.
pub trait Backend {
    /// Present a rendered frame to the display.
    ///
    /// This is called by the compositor after compositing all surfaces.
    /// The backend should display the pixel data on the screen (or store
    /// it for later export).
    fn present(&mut self, buffer: &PixelBuffer);

    /// Get the output dimensions this backend provides.
    fn dimensions(&self) -> (u32, u32);
}

// Re-export backend implementations
pub mod mem;
