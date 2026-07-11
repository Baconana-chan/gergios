// Mouse — mouse input support for minix-term.
//
// Reads from the MINIX input server via /dev/mouse or /dev/mouseN.
// The input server delivers struct input_event records (HID-inspired format):
//
//   | page (u16) | code (u16) | value (i32) | flags (u16) | devid (u16) | rsvd[2] (u32) |
//
// Mouse movement:  page = 0x0001 (INPUT_PAGE_GD)
//                   code = 0x0030 (INPUT_GD_X) or 0x0031 (INPUT_GD_Y)
//                   value = delta (relative) or position (absolute)
//                   flags = 0x00 (absolute) or 0x04 (relative)
//
// Mouse buttons:   page = 0x0009 (INPUT_PAGE_BUTTON)
//                   code = 0x0001.. (INPUT_BUTTON_1, 2, 3, ...)
//                   value = 0 (release) or 1 (press)
//
// # Quick start
//
// ```no_run
// use minix_term::mouse::{Mouse, MouseAction};
//
// let mut mouse = Mouse::open().unwrap();
// if mouse.poll(std::time::Duration::from_millis(100)).unwrap() {
//     match mouse.read_action().unwrap() {
//         Some(MouseAction::Move { dx, dy }) => { /* move cursor */ }
//         Some(MouseAction::ButtonPress(btn)) => { /* click */ }
//         _ => {}
//     }
// }
// ```

use std::io;
use std::time::Duration;

// ===========================================================================
// Cross-platform types
// ===========================================================================

/// Cross-platform raw file descriptor.
#[cfg(unix)]
use std::os::unix::io::RawFd;
#[cfg(windows)]
type RawFd = i32;

// Unix-only libc bindings
#[cfg(unix)]
use libc;

// ===========================================================================
// Constants from minix/include/minix/input.h
// ===========================================================================

/// Event pages
const INPUT_PAGE_GD: u16 = 0x0001;     // General Desktop page
const INPUT_PAGE_KEY: u16 = 0x0007;    // Keyboard/Keypad page
const INPUT_PAGE_BUTTON: u16 = 0x0009; // Button page

/// General Desktop codes
const INPUT_GD_X: u16 = 0x0030;
const INPUT_GD_Y: u16 = 0x0031;
const INPUT_GD_WHEEL: u16 = 0x0038;    // Vertical wheel (HID usage)

/// Button codes
const INPUT_BUTTON_1: u16 = 0x0001;    // Left button
const INPUT_BUTTON_2: u16 = 0x0002;    // Right button
const INPUT_BUTTON_3: u16 = 0x0003;    // Middle button
const INPUT_BUTTON_4: u16 = 0x0004;    // X1 / Back
const INPUT_BUTTON_5: u16 = 0x0005;    // X2 / Forward

/// Event values
const INPUT_RELEASE: i32 = 0;
const INPUT_PRESS: i32 = 1;

/// Event flags
const INPUT_FLAG_REL: u16 = 0x04; // relative value (default is absolute = 0x00)

// ===========================================================================
// Types
// ===========================================================================

/// A raw input event from the MINIX input server.
///
/// Matches `struct input_event` from `<minix/input.h>` exactly.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct InputEvent {
    pub page: u16,
    pub code: u16,
    pub value: i32,
    pub flags: u16,
    pub devid: u16,
    pub rsvd: [u32; 2],
}

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
    Other(u16),
}

impl MouseButton {
    fn from_code(code: u16) -> Self {
        match code {
            INPUT_BUTTON_1 => MouseButton::Left,
            INPUT_BUTTON_2 => MouseButton::Right,
            INPUT_BUTTON_3 => MouseButton::Middle,
            INPUT_BUTTON_4 => MouseButton::X1,
            INPUT_BUTTON_5 => MouseButton::X2,
            n => MouseButton::Other(n),
        }
    }
}

/// A parsed mouse action derived from one or more raw input events.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseAction {
    /// Mouse moved by (dx, dy). Use `accumulated` to track position.
    Move { dx: i32, dy: i32 },
    /// A mouse button was pressed.
    ButtonPress(MouseButton),
    /// A mouse button was released.
    ButtonRelease(MouseButton),
    /// Vertical scroll wheel moved.
    Scroll(i32),
}

/// Accumulated mouse state (position + button mask).
///
/// Useful for games that need to query "where is the mouse?"
/// rather than processing individual events.
#[derive(Debug, Clone, Copy)]
pub struct MouseState {
    /// Absolute X position (tracked from relative deltas).
    pub x: i32,
    /// Absolute Y position.
    pub y: i32,
    /// Currently pressed buttons.
    pub buttons: MouseButtons,
}

bitflags::bitflags! {
    /// Bitmask of pressed mouse buttons.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct MouseButtons: u8 {
        const LEFT   = 0x01;
        const RIGHT  = 0x02;
        const MIDDLE = 0x04;
        const X1     = 0x08;
        const X2     = 0x10;
    }
}

impl Default for MouseState {
    fn default() -> Self {
        MouseState {
            x: 0,
            y: 0,
            buttons: MouseButtons::empty(),
        }
    }
}

impl MouseState {
    /// Update the state from a raw input event.
    /// Returns the corresponding MouseAction if the event is a mouse event.
    pub fn update(&mut self, ev: &InputEvent) -> Option<MouseAction> {
        match ev.page {
            INPUT_PAGE_GD => match ev.code {
                INPUT_GD_X => {
                    if ev.flags & INPUT_FLAG_REL != 0 {
                        self.x = self.x.wrapping_add(ev.value);
                        Some(MouseAction::Move {
                            dx: ev.value,
                            dy: 0,
                        })
                    } else {
                        self.x = ev.value;
                        Some(MouseAction::Move {
                            dx: 0,
                            dy: 0,
                        })
                    }
                }
                INPUT_GD_Y => {
                    if ev.flags & INPUT_FLAG_REL != 0 {
                        self.y = self.y.wrapping_add(ev.value);
                        Some(MouseAction::Move {
                            dx: 0,
                            dy: ev.value,
                        })
                    } else {
                        self.y = ev.value;
                        Some(MouseAction::Move {
                            dx: 0,
                            dy: 0,
                        })
                    }
                }
                INPUT_GD_WHEEL => Some(MouseAction::Scroll(ev.value)),
                _ => None,
            },

            INPUT_PAGE_BUTTON => {
                let btn = MouseButton::from_code(ev.code);
                match ev.value {
                    INPUT_PRESS => {
                        self.buttons |= button_to_bit(btn);
                        Some(MouseAction::ButtonPress(btn))
                    }
                    INPUT_RELEASE => {
                        self.buttons &= !button_to_bit(btn);
                        Some(MouseAction::ButtonRelease(btn))
                    }
                    _ => None,
                }
            }

            _ => None,
        }
    }

    /// Reset position to (0, 0) without releasing buttons.
    pub fn reset_position(&mut self) {
        self.x = 0;
        self.y = 0;
    }
}

fn button_to_bit(btn: MouseButton) -> MouseButtons {
    match btn {
        MouseButton::Left => MouseButtons::LEFT,
        MouseButton::Right => MouseButtons::RIGHT,
        MouseButton::Middle => MouseButtons::MIDDLE,
        MouseButton::X1 => MouseButtons::X1,
        MouseButton::X2 => MouseButtons::X2,
        MouseButton::Other(_) => MouseButtons::empty(),
    }
}

/// Default mouse device path on MINIX.
const DEFAULT_MOUSE_PATH: &str = "/dev/mouse";

// ===========================================================================
// Mouse device (Unix)
// ===========================================================================

/// A handle to a mouse device from the MINIX input server.
#[cfg(unix)]
pub struct Mouse {
    fd: RawFd,
    buf: [u8; 256],
    buf_offset: usize,
    buf_len: usize,
}

#[cfg(unix)]
impl Mouse {
    /// Open the default mouse device (`/dev/mouse`).
    pub fn open() -> io::Result<Self> {
        Self::open_path(DEFAULT_MOUSE_PATH)
    }

    /// Open a specific mouse device path (e.g. `/dev/mouse0`).
    pub fn open_path(path: &str) -> io::Result<Self> {
        let c_path = std::ffi::CString::new(path).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "path contains null byte")
        })?;

        let fd = unsafe {
            libc::open(
                c_path.as_ptr(),
                libc::O_RDONLY | libc::O_NONBLOCK,
            )
        };

        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(Mouse {
            fd,
            buf: [0u8; 256],
            buf_offset: 0,
            buf_len: 0,
        })
    }

    /// Check if mouse data is available for reading.
    pub fn poll(&self, timeout: Duration) -> io::Result<bool> {
        unsafe {
            let mut fds: libc::fd_set = std::mem::zeroed();
            libc::FD_SET(self.fd, &mut fds);

            let mut tv = libc::timeval {
                tv_sec: timeout.as_secs() as libc::time_t,
                tv_usec: timeout.subsec_micros() as libc::suseconds_t,
            };

            let ret = libc::select(
                self.fd + 1,
                &mut fds,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut tv,
            );

            if ret < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(libc::FD_ISSET(self.fd, &fds))
            }
        }
    }

    /// Read a single raw `InputEvent` from the device.
    pub fn read_event(&mut self) -> io::Result<Option<InputEvent>> {
        let event_size = std::mem::size_of::<InputEvent>();
        if self.buf_len - self.buf_offset < event_size {
            self.buf_offset = 0;
            self.buf_len = 0;

            let n = unsafe {
                libc::read(
                    self.fd,
                    self.buf.as_mut_ptr() as *mut libc::c_void,
                    self.buf.len(),
                )
            };

            if n < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::WouldBlock {
                    return Ok(None);
                }
                return Err(err);
            }

            if n == 0 {
                return Ok(None);
            }

            self.buf_len = n as usize;
        }

        let event: InputEvent = unsafe {
            std::ptr::read(self.buf[self.buf_offset..].as_ptr() as *const InputEvent)
        };

        self.buf_offset += event_size;
        Ok(Some(event))
    }

    /// Read all available events and accumulate them into a single `MouseAction`.
    pub fn read_action(&mut self) -> io::Result<Option<MouseAction>> {
        let mut dx: i32 = 0;
        let mut dy: i32 = 0;
        let mut action: Option<MouseAction> = None;

        loop {
            match self.read_event()? {
                None => break,
                Some(ev) => match ev.page {
                    INPUT_PAGE_GD => match ev.code {
                        INPUT_GD_X => { dx += ev.value; action = Some(MouseAction::Move { dx, dy }); }
                        INPUT_GD_Y => { dy += ev.value; action = Some(MouseAction::Move { dx, dy }); }
                        INPUT_GD_WHEEL => { dx = 0; dy = 0; action = Some(MouseAction::Scroll(ev.value)); }
                        _ => {}
                    },
                    INPUT_PAGE_BUTTON => {
                        dx = 0; dy = 0;
                        let btn = MouseButton::from_code(ev.code);
                        action = Some(match ev.value {
                            INPUT_PRESS => MouseAction::ButtonPress(btn),
                            _ => MouseAction::ButtonRelease(btn),
                        });
                    }
                    _ => {}
                },
            }
        }
        Ok(action)
    }
}

#[cfg(unix)]
impl Drop for Mouse {
    fn drop(&mut self) {
        unsafe { libc::close(self.fd); }
    }
}

// ===========================================================================
// Mouse device stub (non-Unix)
// ===========================================================================

/// Stub mouse implementation for non-Unix platforms (e.g., Windows dev builds).
/// All methods return errors or default values.
#[cfg(not(unix))]
pub struct Mouse;

#[cfg(not(unix))]
impl Mouse {
    pub fn open() -> io::Result<Self> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "mouse input requires a Unix-like platform"))
    }
    pub fn open_path(_path: &str) -> io::Result<Self> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "mouse input requires a Unix-like platform"))
    }
    pub fn poll(&self, _timeout: Duration) -> io::Result<bool> { Ok(false) }
    pub fn read_event(&mut self) -> io::Result<Option<InputEvent>> { Ok(None) }
    pub fn read_action(&mut self) -> io::Result<Option<MouseAction>> { Ok(None) }
}
// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_event_size() {
        assert_eq!(std::mem::size_of::<InputEvent>(), 20);
    }

    #[test]
    fn test_mouse_state_tracks_position() {
        let mut state = MouseState::default();
        assert_eq!(state.x, 0);
        assert_eq!(state.y, 0);

        let ev = InputEvent {
            page: INPUT_PAGE_GD,
            code: INPUT_GD_X,
            value: 10,
            flags: INPUT_FLAG_REL,
            devid: 0,
            rsvd: [0, 0],
        };
        let action = state.update(&ev);
        assert_eq!(action, Some(MouseAction::Move { dx: 10, dy: 0 }));
        assert_eq!(state.x, 10);

        let ev = InputEvent {
            page: INPUT_PAGE_GD,
            code: INPUT_GD_Y,
            value: -5,
            flags: INPUT_FLAG_REL,
            devid: 0,
            rsvd: [0, 0],
        };
        let action = state.update(&ev);
        assert_eq!(action, Some(MouseAction::Move { dx: 0, dy: -5 }));
        assert_eq!(state.y, -5);
    }

    #[test]
    fn test_mouse_state_tracks_buttons() {
        let mut state = MouseState::default();
        assert!(state.buttons.is_empty());

        let ev = InputEvent { page: INPUT_PAGE_BUTTON, code: INPUT_BUTTON_1, value: INPUT_PRESS, flags: 0, devid: 0, rsvd: [0, 0] };
        let action = state.update(&ev);
        assert_eq!(action, Some(MouseAction::ButtonPress(MouseButton::Left)));
        assert!(state.buttons.contains(MouseButtons::LEFT));

        let ev = InputEvent { page: INPUT_PAGE_BUTTON, code: INPUT_BUTTON_2, value: INPUT_PRESS, flags: 0, devid: 0, rsvd: [0, 0] };
        state.update(&ev);
        assert!(state.buttons.contains(MouseButtons::RIGHT));

        let ev = InputEvent { page: INPUT_PAGE_BUTTON, code: INPUT_BUTTON_1, value: INPUT_RELEASE, flags: 0, devid: 0, rsvd: [0, 0] };
        state.update(&ev);
        assert!(!state.buttons.contains(MouseButtons::LEFT));
        assert!(state.buttons.contains(MouseButtons::RIGHT));
    }

    #[test]
    fn test_mouse_state_absolute() {
        let mut state = MouseState::default();
        let ev = InputEvent { page: INPUT_PAGE_GD, code: INPUT_GD_X, value: 640, flags: 0, devid: 0, rsvd: [0, 0] };
        state.update(&ev);
        assert_eq!(state.x, 640);
    }

    #[test]
    fn test_mouse_button_mapping() {
        assert_eq!(MouseButton::from_code(INPUT_BUTTON_1), MouseButton::Left);
        assert_eq!(MouseButton::from_code(INPUT_BUTTON_5), MouseButton::X2);
        assert_eq!(MouseButton::from_code(42), MouseButton::Other(42));
    }

    #[test]
    fn test_scroll_event() {
        let mut state = MouseState::default();
        let ev = InputEvent { page: INPUT_PAGE_GD, code: INPUT_GD_WHEEL, value: 1, flags: INPUT_FLAG_REL, devid: 0, rsvd: [0, 0] };
        let action = state.update(&ev);
        assert_eq!(action, Some(MouseAction::Scroll(1)));
        assert_eq!(state.x, 0);
        assert_eq!(state.y, 0);
    }

    #[test]
    fn test_wrapping_overflow() {
        let mut state = MouseState::default();
        let ev = InputEvent { page: INPUT_PAGE_GD, code: INPUT_GD_X, value: -10, flags: INPUT_FLAG_REL, devid: 0, rsvd: [0, 0] };
        state.update(&ev);
        assert_eq!(state.x, -10i32);
    }
}
