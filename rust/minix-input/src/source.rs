//! # Input Sources — Abstraction over where input events come from
//!
//! The `InputSource` trait decouples event generation from event processing.
//!
//! ## Implementations
//!
//! - **SimInputSource**: Generates synthetic input events for testing and
//!   host-side development. Supports pre-loaded event sequences and simple
//!   timer-based random events (with `std` feature).
//! - **MinixHidSource** (requires `minix` feature + `target_os = "minix"`):
//!   Reads from `/dev/kbd0` and `/dev/mouse0` via MINIX VFS IPC.

// Re-export all event types for convenient use
pub use crate::events::*;

/// A source of input events.
///
/// The compositor calls `poll()` periodically to get any new events
/// that have accumulated since the last call.
pub trait InputSource {
    /// Poll for new input events since the last call.
    ///
    /// Returns a list of events in chronological order.
    /// Returns an empty vec if no events are available.
    fn poll(&mut self) -> alloc::vec::Vec<InputEvent>;

    /// Returns true if this source has pending events.
    fn has_pending(&self) -> bool;
}

// ── Simulated Input Source ────────────────────────────────────────────────

/// A simulated input source for testing and host-side development.
///
/// Can be populated with a pre-defined sequence of events (for testing)
/// or generate simple periodic events (e.g., wiggle the mouse).
#[derive(Debug, Clone)]
pub struct SimInputSource {
    /// Pre-loaded event queue.
    queue: alloc::vec::Vec<InputEvent>,
    /// Auto-generate events if `true`.
    auto_mode: bool,
    /// Counter for auto-generated event sequences.
    tick: u64,
}

impl SimInputSource {
    /// Create an empty simulated input source.
    pub fn new() -> Self {
        Self {
            queue: alloc::vec::Vec::new(),
            auto_mode: false,
            tick: 0,
        }
    }

    /// Create a simulated source with a pre-defined sequence of events.
    pub fn with_events(events: alloc::vec::Vec<InputEvent>) -> Self {
        Self {
            queue: events,
            auto_mode: false,
            tick: 0,
        }
    }

    /// Push a single event into the queue.
    pub fn push(&mut self, event: InputEvent) {
        self.queue.push(event);
    }

    /// Enable auto-mode: generates simple events each poll.
    /// Useful for testing animation without real hardware.
    pub fn set_auto_mode(&mut self, enable: bool) {
        self.auto_mode = enable;
    }

    /// Helper: generate a simple key press+release sequence.
    pub fn type_key(&mut self, key: KeySymbol) {
        self.queue.push(InputEvent::Keyboard {
            key,
            pressed: true,
            modifiers: Modifiers::new(),
        });
        self.queue.push(InputEvent::Keyboard {
            key,
            pressed: false,
            modifiers: Modifiers::new(),
        });
    }

    /// Helper: generate a mouse motion event.
    pub fn move_mouse(&mut self, x: i32, y: i32) {
        self.queue.push(InputEvent::MouseMotion {
            x,
            y,
            dx: 0,
            dy: 0,
            modifiers: Modifiers::new(),
        });
    }

    /// Helper: generate a mouse click.
    pub fn click_mouse(&mut self, button: MouseButton, x: i32, y: i32) {
        self.queue.push(InputEvent::MouseButton {
            button,
            pressed: true,
            x,
            y,
            modifiers: Modifiers::new(),
        });
        self.queue.push(InputEvent::MouseButton {
            button,
            pressed: false,
            x,
            y,
            modifiers: Modifiers::new(),
        });
    }
}

impl InputSource for SimInputSource {
    fn poll(&mut self) -> alloc::vec::Vec<InputEvent> {
        if self.auto_mode {
            // Generate a periodic mouse wiggle
            let mut events = alloc::vec::Vec::new();
            let t = self.tick;
            self.tick = self.tick.wrapping_add(1);

            // Every 60 ticks, move mouse in a circle
            if t % 60 == 0 {
                let angle = (t / 60) as f64 * 0.1;
                let x = 400 + (angle.sin() * 200.0) as i32;
                let y = 300 + (angle.cos() * 150.0) as i32;
                events.push(InputEvent::MouseMotion {
                    x, y, dx: 0, dy: 0,
                    modifiers: Modifiers::new(),
                });
            }

            // Every 300 ticks, type a letter
            if t % 300 == 0 {
                let letters = [KeySymbol::KeyH, KeySymbol::KeyE, KeySymbol::KeyL, KeySymbol::KeyL, KeySymbol::KeyO];
                let idx = ((t / 300) % 5) as usize;
                let sym = letters[idx];
                events.push(InputEvent::Keyboard {
                    key: sym, pressed: true,
                    modifiers: Modifiers::new(),
                });
                events.push(InputEvent::Keyboard {
                    key: sym, pressed: false,
                    modifiers: Modifiers::new(),
                });
            }

            events.push(InputEvent::Frame);
            return events;
        }

        // Drain the pre-loaded queue
        let mut events = alloc::vec::Vec::new();
        core::mem::swap(&mut events, &mut self.queue);
        if !events.is_empty() {
            events.push(InputEvent::Frame);
        }
        events
    }

    fn has_pending(&self) -> bool {
        self.auto_mode || !self.queue.is_empty()
    }
}

impl Default for SimInputSource {
    fn default() -> Self {
        Self::new()
    }
}

// ── MINIX HID chardev Source ─────────────────────────────────────────────

/// Reads input events from MINIX HID character devices (/dev/kbd0, /dev/mouse0).
///
/// Uses MINIX VFS IPC (`open`, `read`) to access the devices.
/// The devices provide boot-protocol report format:
///
/// **Keyboard** (8 bytes):
/// - byte 0: modifiers (bit 0=LCtrl, 1=LShift, 2=LAlt, 3=LGui, 4=RCtrl, 5=RShift, 6=RAlt, 7=RGui)
/// - byte 1: reserved
/// - bytes 2-7: key codes (up to 6 simultaneous keys)
///
/// **Mouse** (8 bytes):
/// - byte 0: buttons (bit 0=Left, 1=Right, 2=Middle)
/// - byte 1: X movement (signed)
/// - byte 2: Y movement (signed)
/// - byte 3: wheel (signed)
/// - bytes 4-7: reserved
///
/// Only available on `#[cfg(target_os = "minix")]`.
#[cfg(feature = "minix")]
pub mod minix_backend {
    use crate::events::*;
    use super::InputSource;

    /// MINIX HID character device source.
    ///
    /// Opens `/dev/kbd0` and `/dev/mouse0` on startup and polls them
    /// for input events each cycle.
    pub struct MinixHidSource {
        kbd_fd: Option<i32>,
        mouse_fd: Option<i32>,
        kbd_state: crate::events::KeyboardState,
        mouse_state: crate::events::MouseState,
        output_width: u32,
        output_height: u32,
    }

    impl MinixHidSource {
        /// Create and open HID character devices.
        pub fn new(output_width: u32, output_height: u32) -> Self {
            let kbd_fd = Self::open_dev(b"/dev/kbd0\0");
            let mouse_fd = Self::open_dev(b"/dev/mouse0\0");
            Self {
                kbd_fd,
                mouse_fd,
                kbd_state: KeyboardState::new(),
                mouse_state: MouseState::new(),
                output_width,
                output_height,
            }
        }

        /// Open a character device via VFS IPC.
        fn open_dev(path: &[u8]) -> Option<i32> {
            // On MINIX, opening a device involves sending VFS_OPEN to VFS
            let mut msg = minix_rs::Message::new();
            msg.write_i32(0, path.as_ptr() as i32); // path pointer
            // TODO: Fix VFS_OPEN message layout for 64-bit MINIX.
        // The pointer is truncated to i32 on 64-bit systems.
        // Need to use the correct message format with m1_p1 for path.
        let result = minix_rs::vfs_syscall(minix_rs::VFS_OPEN, &mut msg);
            if result >= 0 { Some(result) } else { None }
        }

        /// Read from a character device via VFS IPC.
        fn read_dev(fd: i32, buf: &mut [u8]) -> Option<usize> {
            if fd < 0 { return None; }
            let mut msg = minix_rs::Message::new();
            msg.write_i32(0, fd);
            msg.write_ptr(8, buf.as_ptr() as usize);
            msg.write_i32(16, buf.len() as i32);
            // TODO: Fix VFS_READ message layout for 64-bit MINIX.
            // VFS_READ expects different message fields.
            let result = minix_rs::vfs_syscall(minix_rs::VFS_READ, &mut msg);
            if result > 0 { Some(result as usize) } else { None }
        }

        /// Set/resize the output dimensions (for mouse clamping).
        pub fn set_output_size(&mut self, width: u32, height: u32) {
            self.output_width = width;
            self.output_height = height;
        }
    }

    impl InputSource for MinixHidSource {
        fn poll(&mut self) -> alloc::vec::Vec<InputEvent> {
            let mut events = alloc::vec::Vec::new();

            // Poll keyboard
            if let Some(kbd_fd) = self.kbd_fd {
                let mut buf = [0u8; 8];
                if Self::read_dev(kbd_fd, &mut buf).unwrap_or(0) >= 8 {
                    let modbits = buf[0];
                    let keys = [buf[2], buf[3], buf[4], buf[5], buf[6], buf[7]];
                    let mut kbd_events = self.kbd_state.update(modbits, &keys);
                    events.append(&mut kbd_events);
                }
            }

            // Poll mouse
            if let Some(mouse_fd) = self.mouse_fd {
                let mut buf = [0u8; 8];
                if Self::read_dev(mouse_fd, &mut buf).unwrap_or(0) >= 4 {
                    let btn_bits = buf[0];
                    let dx = buf[1] as i8 as i16;
                    let dy = buf[2] as i8 as i16;
                    let wheel = buf[3] as i8;
                    let mut mouse_events = self.mouse_state.update(
                        dx, dy, btn_bits, wheel,
                        self.output_width, self.output_height,
                    );
                    events.append(&mut mouse_events);
                }
            }

            // Signal frame boundary
            events.push(InputEvent::Frame);
            events
        }

        fn has_pending(&self) -> bool {
            // Always true — the VFS read is non-blocking; if no data,
            // poll will just return an empty vec (or Frame only)
            true
        }
    }
}

#[cfg(not(feature = "minix"))]
pub mod minix_backend {
    // Stub module for non-MINIX platforms — empty.
}
