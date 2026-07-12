//! # Input Events — Keyboard, mouse, and touch event types for the compositor
//!
//! Defines high-level input events that the compositor can dispatch to
//! surfaces. These events are more abstract than raw HID reports:
//!
//! - **Keyboard**: Press/Release with translated `KeySymbol` (e.g., `KeyA`,
//!   `KeyReturn`, `KeyF1`), plus modifier state tracking.
//! - **Mouse**: Motion (absolute/relative), button press/release, scroll.
//! - **Touch**: Touch down/move/up with positions.
//!
//! ## Source mapping
//!
//! Raw HID → `minix-xhci` → `/dev/kbd0` (8-byte boot report)
//! → `MinixHidSource` → `InputEvent::Keyboard` / `InputEvent::Mouse`

/// A translated keyboard key symbol (based on USB HID usage codes / evdev codes).
///
/// Covers the standard 104-key US keyboard layout plus media keys.
/// Values match USB HID keyboard usage codes for direct mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u16)]
pub enum KeySymbol {
    KeyA          = 0x04, KeyB          = 0x05,
    KeyC          = 0x06, KeyD          = 0x07,
    KeyE          = 0x08, KeyF          = 0x09,
    KeyG          = 0x0A, KeyH          = 0x0B,
    KeyI          = 0x0C, KeyJ          = 0x0D,
    KeyK          = 0x0E, KeyL          = 0x0F,
    KeyM          = 0x10, KeyN          = 0x11,
    KeyO          = 0x12, KeyP          = 0x13,
    KeyQ          = 0x14, KeyR          = 0x15,
    KeyS          = 0x16, KeyT          = 0x17,
    KeyU          = 0x18, KeyV          = 0x19,
    KeyW          = 0x1A, KeyX          = 0x1B,
    KeyY          = 0x1C, KeyZ          = 0x1D,
    // Numbers
    Key1          = 0x1E, Key2          = 0x1F,
    Key3          = 0x20, Key4          = 0x21,
    Key5          = 0x22, Key6          = 0x23,
    Key7          = 0x24, Key8          = 0x25,
    Key9          = 0x26, Key0          = 0x27,
    // Punctuation & editing
    Enter         = 0x28, Escape        = 0x29,
    Backspace     = 0x2A, Tab           = 0x2B,
    Space         = 0x2C, Minus         = 0x2D,
    Equal         = 0x2E, LeftBracket   = 0x2F,
    RightBracket  = 0x30, Backslash     = 0x31,
    Semicolon     = 0x33, Quote         = 0x34,
    Grave         = 0x35, Comma         = 0x36,
    Dot           = 0x37, Slash         = 0x38,
    CapsLock      = 0x39,
    // Function keys
    F1 = 0x3A, F2 = 0x3B, F3 = 0x3C, F4 = 0x3D,
    F5 = 0x3E, F6 = 0x3F, F7 = 0x40, F8 = 0x41,
    F9 = 0x42, F10 = 0x43, F11 = 0x44, F12 = 0x45,
    // Navigation
    PrintScreen = 0x46, ScrollLock = 0x47, Pause = 0x48,
    Insert = 0x49, Home = 0x4A, PageUp = 0x4B,
    Delete = 0x4C, End = 0x4D, PageDown = 0x4E,
    RightArrow = 0x4F, LeftArrow = 0x50,
    DownArrow = 0x51, UpArrow = 0x52,
    // Keypad
    NumLock = 0x53, KeypadDivide = 0x54, KeypadMultiply = 0x55,
    KeypadSubtract = 0x56, KeypadAdd = 0x57, KeypadEnter = 0x58,
    Keypad1 = 0x59, Keypad2 = 0x5A, Keypad3 = 0x5B,
    Keypad4 = 0x5C, Keypad5 = 0x5D, Keypad6 = 0x5E,
    Keypad7 = 0x5F, Keypad8 = 0x60, Keypad9 = 0x61,
    Keypad0 = 0x62, KeypadDot = 0x63,
    // Modifiers
    LeftCtrl  = 0xE0, LeftShift = 0xE1,
    LeftAlt   = 0xE2, LeftMeta  = 0xE3, // Windows/Command
    RightCtrl = 0xE4, RightShift = 0xE5,
    RightAlt  = 0xE6, RightMeta  = 0xE7,
    // Extras
    Application = 0x65, Power = 0x66,
    F13 = 0x68, F14 = 0x69, F15 = 0x6A, F16 = 0x6B,
    F17 = 0x6C, F18 = 0x6D, F19 = 0x6E, F20 = 0x6F,
    F21 = 0x70, F22 = 0x71, F23 = 0x72, F24 = 0x73,
    /// Unknown/unmapped key.
    Unknown = 0xFFFF,
}

impl KeySymbol {
    /// Convert a USB HID keyboard usage code to a `KeySymbol`.
    pub const fn from_hid(code: u8) -> Self {
        match code {
            0x04 => Self::KeyA, 0x05 => Self::KeyB, 0x06 => Self::KeyC,
            0x07 => Self::KeyD, 0x08 => Self::KeyE, 0x09 => Self::KeyF,
            0x0A => Self::KeyG, 0x0B => Self::KeyH, 0x0C => Self::KeyI,
            0x0D => Self::KeyJ, 0x0E => Self::KeyK, 0x0F => Self::KeyL,
            0x10 => Self::KeyM, 0x11 => Self::KeyN, 0x12 => Self::KeyO,
            0x13 => Self::KeyP, 0x14 => Self::KeyQ, 0x15 => Self::KeyR,
            0x16 => Self::KeyS, 0x17 => Self::KeyT, 0x18 => Self::KeyU,
            0x19 => Self::KeyV, 0x1A => Self::KeyW, 0x1B => Self::KeyX,
            0x1C => Self::KeyY, 0x1D => Self::KeyZ,
            0x1E => Self::Key1, 0x1F => Self::Key2,
            0x20 => Self::Key3, 0x21 => Self::Key4,
            0x22 => Self::Key5, 0x23 => Self::Key6,
            0x24 => Self::Key7, 0x25 => Self::Key8,
            0x26 => Self::Key9, 0x27 => Self::Key0,
            0x28 => Self::Enter, 0x29 => Self::Escape,
            0x2A => Self::Backspace, 0x2B => Self::Tab,
            0x2C => Self::Space, 0x2D => Self::Minus,
            0x2E => Self::Equal, 0x2F => Self::LeftBracket,
            0x30 => Self::RightBracket, 0x31 => Self::Backslash,
            0x33 => Self::Semicolon, 0x34 => Self::Quote,
            0x35 => Self::Grave, 0x36 => Self::Comma,
            0x37 => Self::Dot, 0x38 => Self::Slash,
            0x39 => Self::CapsLock,
            0x3A => Self::F1, 0x3B => Self::F2,
            0x3C => Self::F3, 0x3D => Self::F4,
            0x3E => Self::F5, 0x3F => Self::F6,
            0x40 => Self::F7, 0x41 => Self::F8,
            0x42 => Self::F9, 0x43 => Self::F10,
            0x44 => Self::F11, 0x45 => Self::F12,
            0x46 => Self::PrintScreen, 0x47 => Self::ScrollLock,
            0x48 => Self::Pause, 0x49 => Self::Insert,
            0x4A => Self::Home, 0x4B => Self::PageUp,
            0x4C => Self::Delete, 0x4D => Self::End,
            0x4E => Self::PageDown, 0x4F => Self::RightArrow,
            0x50 => Self::LeftArrow, 0x51 => Self::DownArrow,
            0x52 => Self::UpArrow,
            0x53 => Self::NumLock,
            0x54 => Self::KeypadDivide, 0x55 => Self::KeypadMultiply,
            0x56 => Self::KeypadSubtract, 0x57 => Self::KeypadAdd,
            0x58 => Self::KeypadEnter,
            0x59 => Self::Keypad1, 0x5A => Self::Keypad2,
            0x5B => Self::Keypad3, 0x5C => Self::Keypad4,
            0x5D => Self::Keypad5, 0x5E => Self::Keypad6,
            0x5F => Self::Keypad7, 0x60 => Self::Keypad8,
            0x61 => Self::Keypad9, 0x62 => Self::Keypad0,
            0x63 => Self::KeypadDot,
            0x65 => Self::Application, 0x66 => Self::Power,
            0x68 => Self::F13, 0x69 => Self::F14,
            0x6A => Self::F15, 0x6B => Self::F16,
            0x6C => Self::F17, 0x6D => Self::F18,
            0x6E => Self::F19, 0x6F => Self::F20,
            0x70 => Self::F21, 0x71 => Self::F22,
            0x72 => Self::F23, 0x73 => Self::F24,
            0xE0 => Self::LeftCtrl, 0xE1 => Self::LeftShift,
            0xE2 => Self::LeftAlt, 0xE3 => Self::LeftMeta,
            0xE4 => Self::RightCtrl, 0xE5 => Self::RightShift,
            0xE6 => Self::RightAlt, 0xE7 => Self::RightMeta,
            _ => Self::Unknown,
        }
    }

    /// Convert a HID boot protocol byte (from /dev/kbd0) to KeySymbol.
    /// Boot protocol has predefined codes; pass `0` for no-key.
    pub fn from_boot_protocol(code: u8) -> Option<Self> {
        if code == 0 { return None; }
        Some(Self::from_hid(code))
    }
}

// ── Keyboard modifier bitmask ─────────────────────────────────────────────

/// Bitmask of active keyboard modifiers.
///
/// Matches USB HID boot protocol modifier byte layout:
/// bit 0=LCtrl, 1=LShift, 2=LAlt, 3=LMeta (Win/Cmd),
/// bit 4=RCtrl, 5=RShift, 6=RAlt, 7=RMeta (Win/Cmd).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub(crate) bits: u8,
}

impl Modifiers {
    pub const fn from_bits(bits: u8) -> Self {
        Self { bits }
    }

    pub fn new() -> Self {
        Self { bits: 0 }
    }

    /// Check if any Shift key is held.
    pub fn shift(&self) -> bool { (self.bits & 0x22) != 0 }

    /// Check if any Ctrl key is held.
    pub fn ctrl(&self) -> bool { (self.bits & 0x11) != 0 }

    /// Check if any Alt key is held.
    pub fn alt(&self) -> bool { (self.bits & 0x44) != 0 }

    /// Check if any Meta (Win/Cmd) key is held.
    pub fn meta(&self) -> bool { (self.bits & 0x88) != 0 }

    /// Get the raw modifier byte.
    pub fn raw(&self) -> u8 { self.bits }
}

// ── Mouse ─────────────────────────────────────────────────────────────────

/// Mouse buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
    Other(u8),
}

impl MouseButton {
    pub const fn from_hid(btn: u8) -> Self {
        match btn {
            1 => Self::Left, 2 => Self::Right,
            3 => Self::Middle, 4 => Self::Back,
            5 => Self::Forward, n => Self::Other(n),
        }
    }
}

// ── Touch ─────────────────────────────────────────────────────────────────

/// A single touch point.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchPoint {
    pub id: u32,
    pub x: i32,
    pub y: i32,
}

/// Phase of a touch event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TouchPhase {
    Down,
    Move,
    Up,
    Cancel,
}

// ── Event enum ────────────────────────────────────────────────────────────

/// A high-level input event for the compositor.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    /// A key was pressed or released.
    Keyboard {
        key: KeySymbol,
        pressed: bool,
        modifiers: Modifiers,
    },
    /// Mouse moved to a new absolute position (relative to output).
    MouseMotion {
        x: i32, y: i32,
        dx: i32, dy: i32,
        modifiers: Modifiers,
    },
    /// Mouse button pressed or released.
    MouseButton {
        button: MouseButton,
        pressed: bool,
        x: i32, y: i32,
        modifiers: Modifiers,
    },
    /// Mouse scroll wheel moved.
    MouseWheel {
        delta_x: i32, delta_y: i32,
        modifiers: Modifiers,
    },
    /// Touch event.
    Touch {
        phase: TouchPhase,
        point: TouchPoint,
    },
    /// A frame boundary: all events in the current batch are done.
    /// The compositor should composite after receiving this.
    Frame,
}

// ── Keyboard state tracker ───────────────────────────────────────────────

/// Tracks the currently pressed HID key codes (up to 6 simultaneous keys).
#[derive(Debug, Clone)]
struct KeySet {
    pressed: alloc::vec::Vec<u8>,
}

impl KeySet {
    fn new() -> Self {
        Self { pressed: alloc::vec::Vec::with_capacity(6) }
    }

    fn contains(&self, code: u8) -> bool {
        self.pressed.contains(&code)
    }

    fn update(&mut self, keys: &[u8]) -> (alloc::vec::Vec<u8>, alloc::vec::Vec<u8>) {
        // Keys in the new report
        let mut new_pressed = alloc::vec::Vec::with_capacity(6);
        for &k in keys {
            if k > 0 && !new_pressed.contains(&k) {
                new_pressed.push(k);
            }
        }

        // Detect releases (were in old, not in new)
        let released: alloc::vec::Vec<u8> = self.pressed.iter()
            .filter(|k| !new_pressed.contains(k))
            .copied()
            .collect();

        // Detect presses (in new, not in old)
        let pressed: alloc::vec::Vec<u8> = new_pressed.iter()
            .filter(|k| !self.pressed.contains(k))
            .copied()
            .collect();

        self.pressed = new_pressed;
        (pressed, released)
    }
}

/// Tracks the current keyboard state: pressed keys and modifiers.
///
/// Given a `(modifiers, key_codes)` tuple from boot protocol,
/// produces a sequence of `InputEvent::Keyboard` press/release events.
#[derive(Debug, Clone)]
pub struct KeyboardState {
    pressed: KeySet,
    modifiers: Modifiers,
}

impl KeyboardState {
    pub fn new() -> Self {
        Self {
            pressed: KeySet::new(),
            modifiers: Modifiers::new(),
        }
    }

    /// Update state from a boot-protocol keyboard report.
    ///
    /// `modbits`: modifier byte (from /dev/kbd0 byte 0)
    /// `keys`: up to 6 key codes (from /dev/kbd0 bytes 2-7)
    ///
    /// Returns a list of `InputEvent`s for changed keys.
    pub fn update(&mut self, modbits: u8, keys: &[u8]) -> alloc::vec::Vec<InputEvent> {
        let mut events = alloc::vec::Vec::new();

        // Detect modifier changes
        let old_mods = self.modifiers;
        self.modifiers = Modifiers::from_bits(modbits);

        // Emit modifier events for changed modifiers
        for (bit, sym) in [
            (0, KeySymbol::LeftCtrl), (1, KeySymbol::LeftShift),
            (2, KeySymbol::LeftAlt), (3, KeySymbol::LeftMeta),
            (4, KeySymbol::RightCtrl), (5, KeySymbol::RightShift),
            (6, KeySymbol::RightAlt), (7, KeySymbol::RightMeta),
        ] {
            let was = (old_mods.bits >> bit) & 1;
            let now = (modbits >> bit) & 1;
            if was != now {
                events.push(InputEvent::Keyboard {
                    key: sym,
                    pressed: now != 0,
                    modifiers: self.modifiers,
                });
            }
        }

        // Detect key code changes
        let (pressed, released) = self.pressed.update(keys);

        for code in released {
            if let Some(sym) = KeySymbol::from_boot_protocol(code) {
                events.push(InputEvent::Keyboard {
                    key: sym,
                    pressed: false,
                    modifiers: self.modifiers,
                });
            }
        }

        for code in pressed {
            if let Some(sym) = KeySymbol::from_boot_protocol(code) {
                events.push(InputEvent::Keyboard {
                    key: sym,
                    pressed: true,
                    modifiers: self.modifiers,
                });
            }
        }

        events
    }

    /// Check if a specific key is currently pressed.
    pub fn is_pressed(&self, key: KeySymbol) -> bool {
        let code = key as u8;
        if code >= 0xE0 {
            let bit = code - 0xE0;
            (self.modifiers.bits & (1 << bit)) != 0
        } else {
            self.pressed.contains(code)
        }
    }

    /// Get current modifiers.
    pub fn modifiers(&self) -> Modifiers {
        self.modifiers
    }
}

// ── Mouse state tracker ──────────────────────────────────────────────────

/// Tracks absolute mouse position and button state.
///
/// Converts relative mouse deltas (from boot protocol) into absolute
/// positions suitable for the compositor.
#[derive(Debug, Clone)]
pub struct MouseState {
    pub x: i32,
    pub y: i32,
    pub buttons: u8,
}

impl MouseState {
    pub fn new() -> Self {
        Self { x: 0, y: 0, buttons: 0 }
    }

    /// Apply a relative mouse motion report (from /dev/mouse0).
    ///
    /// `(dx, dy)`: relative movement in pixels.
    /// `btn_bits`: button bitmask (bit 0=left, 1=right, 2=middle).
    /// `output_w`, `output_h`: output dimensions for clamping.
    ///
    /// Returns a list of input events.
    pub fn update(
        &mut self,
        dx: i16, dy: i16,
        btn_bits: u8, wheel: i8,
        output_w: u32, output_h: u32,
    ) -> alloc::vec::Vec<InputEvent> {
        let mut events = alloc::vec::Vec::new();

        // Track position changes
        let old_x = self.x;
        let old_y = self.y;

        // Clamp to output dimensions
        let max_x = if output_w > 0 { (output_w - 1) as i32 } else { 0 };
        let max_y = if output_h > 0 { (output_h - 1) as i32 } else { 0 };
        self.x = (self.x + dx as i32).clamp(0, max_x);
        self.y = (self.y + dy as i32).clamp(0, max_y);

        if self.x != old_x || self.y != old_y {
            events.push(InputEvent::MouseMotion {
                x: self.x, y: self.y,
                dx: self.x - old_x, dy: self.y - old_y,
                modifiers: self.modifiers(),
            });
        }

        // Detect button changes
        let old_buttons = self.buttons;
        for bit in 0..3 {
            let was = (old_buttons >> bit) & 1;
            let now = (btn_bits >> bit) & 1;
            if was != now {
                let button = match bit {
                    0 => MouseButton::Left,
                    1 => MouseButton::Right,
                    2 => MouseButton::Middle,
                    _ => unreachable!(),
                };
                events.push(InputEvent::MouseButton {
                    button,
                    pressed: now != 0,
                    x: self.x, y: self.y,
                    modifiers: self.modifiers(),
                });
            }
        }
        self.buttons = btn_bits;

        // Wheel event
        if wheel != 0 {
            events.push(InputEvent::MouseWheel {
                delta_x: 0, delta_y: wheel as i32,
                modifiers: self.modifiers(),
            });
        }

        events
    }

    fn modifiers(&self) -> Modifiers {
        // Mouse state doesn't track keyboard modifiers directly
        Modifiers::new()
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_symbol_from_hid() {
        assert_eq!(KeySymbol::from_hid(0x04), KeySymbol::KeyA);
        assert_eq!(KeySymbol::from_hid(0x1E), KeySymbol::Key1);
        assert_eq!(KeySymbol::from_hid(0x28), KeySymbol::Enter);
        assert_eq!(KeySymbol::from_hid(0xE1), KeySymbol::LeftShift);
        assert_eq!(KeySymbol::from_hid(0xFF), KeySymbol::Unknown);
    }

    #[test]
    fn keyboard_state_tracks_modifiers() {
        let mut state = KeyboardState::new();
        let events = state.update(0x02, &[0; 6]); // LShift pressed
        assert!(!events.is_empty());
        assert!(state.is_pressed(KeySymbol::LeftShift));
        assert!(state.modifiers().shift());

        let events = state.update(0x00, &[0; 6]); // LShift released
        assert!(!events.is_empty());
        assert!(!state.is_pressed(KeySymbol::LeftShift));
    }

    #[test]
    fn keyboard_state_tracks_key_press_release() {
        let mut state = KeyboardState::new();
        let events = state.update(0x00, &[0x04, 0, 0, 0, 0, 0]); // A pressed
        let kbd_count = events.iter()
            .filter(|e| matches!(e, InputEvent::Keyboard { .. }))
            .count();
        assert_eq!(kbd_count, 1);
        assert!(state.is_pressed(KeySymbol::KeyA));

        let events = state.update(0x00, &[0; 6]);
        assert!(!events.is_empty());
        assert!(!state.is_pressed(KeySymbol::KeyA));
    }

    #[test]
    fn key_set_tracks_diffs() {
        let mut ks = KeySet::new();
        let (pressed, released) = ks.update(&[0x04, 0x05]);
        assert_eq!(pressed.len(), 2);
        assert!(released.is_empty());
        assert!(ks.contains(0x04));

        let (pressed, released) = ks.update(&[0x04]);
        assert!(pressed.is_empty());
        assert_eq!(released.len(), 1);
        assert_eq!(released[0], 0x05);
    }

    #[test]
    fn mouse_state_tracks_motion() {
        let mut state = MouseState::new();
        let events = state.update(10, 5, 0, 0, 800, 600);
        assert_eq!(state.x, 10);
        assert_eq!(state.y, 5);
        assert!(events.iter().any(|e| matches!(e, InputEvent::MouseMotion { .. })));
    }

    #[test]
    fn mouse_state_clamps_to_output() {
        let mut state = MouseState::new();
        state.update(-5, -5, 0, 0, 800, 600);
        assert_eq!(state.x, 0);
        assert_eq!(state.y, 0);

        state.update(1000, 1000, 0, 0, 800, 600);
        assert_eq!(state.x, 799);
        assert_eq!(state.y, 599);
    }

    #[test]
    fn mouse_state_button_events() {
        let mut state = MouseState::new();
        let events = state.update(0, 0, 0x01, 0, 800, 600);
        assert!(events.iter().any(|e|
            matches!(e, InputEvent::MouseButton {
                button: MouseButton::Left, pressed: true, ..
            })
        ));

        let events = state.update(0, 0, 0x00, 0, 800, 600);
        assert!(events.iter().any(|e|
            matches!(e, InputEvent::MouseButton {
                button: MouseButton::Left, pressed: false, ..
            })
        ));
    }

    #[test]
    fn mouse_wheel_event() {
        let mut state = MouseState::new();
        let events = state.update(0, 0, 0, 3, 800, 600);
        assert!(events.iter().any(|e|
            matches!(e, InputEvent::MouseWheel { delta_y: 3, .. })
        ));
    }
}
