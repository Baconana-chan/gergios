//! # Gamepad Input for Games
//!
//! Provides a unified API for reading gamepad state from USB HID gamepads
//! (`/dev/gamepad*`) and Bluetooth gamepads (via the BT daemon).
//!
//! ## Architecture
//!
//! ```text
//! ┌────────────────────────────────────────┐
//! │              Game Code                 │
//! ├────────────────────────────────────────┤
//! │           gamepad::Gamepad             │
//! │  open("/dev/gamepad0")                 │
//! │  poll() → read_state() → GamepadState  │
//! ├────────────────────────────────────────┤
//! │  USB HID     │   BT HID (via IPC)     │
//! │  /dev/gamepad0│  /dev/bt_gamepad0     │
//! │  (xhci hid)  │  (bt-daemon → gatt)   │
//! └──────────────┴────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```no_run
//! use minix_term::gamepad::{Gamepad, GamepadState, GamepadButton, GamepadAxis};
//!
//! let mut gp = Gamepad::open().unwrap();
//! gp.set_profile_name("Wireless Controller"); // auto-detect DS4
//!
//! loop {
//!     if gp.poll(16).unwrap() {  // ~60 FPS
//!         let state = gp.read_state().unwrap();
//!         if state.is_pressed(GamepadButton::A) {
//!             println!("Jump!");
//!         }
//!         let lx = state.axis_norm(GamepadAxis::LeftStickX);
//!         println!("Move: {}", lx);
//!     }
//! }
//! ```

use std::io;
use std::fs::File;
use std::time::Duration;

// ============================================================================
// Cross-platform re-exports
// ============================================================================

/// For Gamepad::poll() and read_state() — Unix only.
#[cfg(unix)]
use std::os::unix::io::AsRawFd;

// ============================================================================
// Gamepad Button, Axis, Hat (duplicated from minix-bt-stack::hidp for
// independence — minix-term doesn't depend on minix-bt-stack)
// ============================================================================

/// Standard gamepad button identifiers (up to 32 buttons).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum GamepadButton {
    A = 0,
    B = 1,
    X = 2,
    Y = 3,
    LeftBumper = 4,
    RightBumper = 5,
    LeftTrigger = 6,   // digital (L2)
    RightTrigger = 7,  // digital (R2)
    Select = 8,        // Back / -
    Start = 9,         // Forward / +
    LeftStick = 10,    // L3
    RightStick = 11,   // R3
    Guide = 12,        // PS / Xbox / Home
    Capture = 13,      // Share / Capture
    DpadUp = 14,
    DpadDown = 15,
    DpadLeft = 16,
    DpadRight = 17,
    Misc = 18,
}

/// Gamepad axis identifiers.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum GamepadAxis {
    LeftStickX = 0,
    LeftStickY = 1,
    RightStickX = 2,
    RightStickY = 3,
    LeftTrigger = 4,   // analog L2 (0..255 → 0..32767)
    RightTrigger = 5,  // analog R2
    AccelX = 6,        // IMU (DS4/DS5)
    AccelY = 7,
    AccelZ = 8,
    GyroX = 9,
    GyroY = 10,
    GyroZ = 11,
}

/// Hat switch position (8 directions + center).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum HatPosition {
    Center = 0x0F,
    North = 0x00,
    NorthEast = 0x01,
    East = 0x02,
    SouthEast = 0x03,
    South = 0x04,
    SouthWest = 0x05,
    West = 0x06,
    NorthWest = 0x07,
}

impl HatPosition {
    pub fn from_hat_value(v: u8) -> Self {
        match v & 0x0F {
            0x00 => Self::North,
            0x01 => Self::NorthEast,
            0x02 => Self::East,
            0x03 => Self::SouthEast,
            0x04 => Self::South,
            0x05 => Self::SouthWest,
            0x06 => Self::West,
            0x07 => Self::NorthWest,
            _ => Self::Center,
        }
    }
}

// ============================================================================
// Gamepad State
// ============================================================================

/// Full gamepad state snapshot.
#[derive(Clone, Debug)]
pub struct GamepadState {
    /// Bitmask of pressed buttons (32 bits).
    pub buttons: u32,
    /// Axis values (signed 16-bit, range -32768..32767).
    pub axes: [i16; 12],
    /// Hat switch position.
    pub hat: HatPosition,
    /// Battery level (0..100, 101 = unknown).
    pub battery: u8,
    /// Whether the gamepad is connected.
    pub connected: bool,
    /// Whether the state was updated since last read.
    pub updated: bool,
}

impl Default for GamepadState {
    fn default() -> Self {
        Self {
            buttons: 0,
            axes: [0i16; 12],
            hat: HatPosition::Center,
            battery: 101,
            connected: false,
            updated: false,
        }
    }
}

impl GamepadState {
    /// Check if a specific button is pressed.
    pub fn is_pressed(&self, btn: GamepadButton) -> bool {
        (self.buttons & (1 << (btn as u8))) != 0
    }

    /// Get an axis value normalized to [-1.0, 1.0].
    pub fn axis_norm(&self, axis: GamepadAxis) -> f32 {
        let idx = axis as usize;
        if idx >= self.axes.len() {
            return 0.0;
        }
        let raw = self.axes[idx];
        if raw >= 0 {
            raw as f32 / 32767.0
        } else {
            -(raw as f32 / -32768.0)
        }
    }

    /// Get trigger value as [0.0, 1.0].
    pub fn trigger_norm(&self, trigger: GamepadAxis) -> f32 {
        debug_assert!(
            trigger == GamepadAxis::LeftTrigger || trigger == GamepadAxis::RightTrigger
        );
        let idx = trigger as usize;
        if idx >= self.axes.len() {
            return 0.0;
        }
        (self.axes[idx].max(0) as f32) / 32767.0
    }

    /// Mark state as read (clear updated flag).
    pub fn mark_read(&mut self) {
        self.updated = false;
    }

    /// Create a disconnected state.
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            ..Default::default()
        }
    }
}

// ============================================================================
// Gamepad Profile Detection
// ============================================================================

/// Known HID gamepad profiles for report parsing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GamepadProfile {
    /// Sony DualShock 4 (48-byte input report).
    DualShock4,
    /// Sony DualSense (PS5) (48-byte input report).
    DualSense,
    /// Nintendo Switch Pro Controller (8-byte input report).
    SwitchPro,
    /// Xbox Wireless Controller via BLE.
    XboxBle,
    /// 8BitDo or other generic HID gamepad.
    Generic,
    /// Unknown — try generic parser.
    Unknown,
}

impl GamepadProfile {
    /// Detect profile from device name string.
    pub fn detect(name: &str) -> Self {
        let lower = name.to_lowercase();
        if lower.contains("dualshock") || lower.contains("ds4") || lower.contains("wireless controller") {
            return Self::DualShock4;
        }
        if lower.contains("dualsense") || lower.contains("ds5") || lower.contains("ps5") {
            return Self::DualSense;
        }
        if lower.contains("switch") || lower.contains("pro controller") || lower.contains("joy-con") {
            return Self::SwitchPro;
        }
        if lower.contains("xbox") {
            return Self::XboxBle;
        }
        if lower.contains("8bitdo") || lower.contains("8bit") {
            return Self::Generic;
        }
        Self::Unknown
    }
}

// ============================================================================
// HID Report Parsers
// ============================================================================

/// Parse a DualShock 4 / DualSense input report.
fn parse_ds4_report(data: &[u8], state: &mut GamepadState) {
    if data.len() < 6 {
        return;
    }

    let offset = if data[0] == 0x01 || data[0] == 0x11 { 1 } else { 0 };

    // Buttons (16-bit LE)
    let buttons_low = if offset + 2 <= data.len() {
        u16::from_le_bytes([data[offset], data[offset + 1]])
    } else {
        0
    };
    let buttons_hi_and_hat = data.get(offset + 2).copied().unwrap_or(0);

    // Map DS4 buttons to standard GamepadButton
    if buttons_low & (1 << 0) != 0 { state.buttons |= 1 << (GamepadButton::Select as u8); }
    if buttons_low & (1 << 1) != 0 { state.buttons |= 1 << (GamepadButton::LeftStick as u8); }
    if buttons_low & (1 << 2) != 0 { state.buttons |= 1 << (GamepadButton::RightStick as u8); }
    if buttons_low & (1 << 3) != 0 { state.buttons |= 1 << (GamepadButton::Start as u8); }
    if buttons_low & (1 << 4) != 0 { state.buttons |= 1 << (GamepadButton::DpadUp as u8); }
    if buttons_low & (1 << 5) != 0 { state.buttons |= 1 << (GamepadButton::DpadRight as u8); }
    if buttons_low & (1 << 6) != 0 { state.buttons |= 1 << (GamepadButton::DpadDown as u8); }
    if buttons_low & (1 << 7) != 0 { state.buttons |= 1 << (GamepadButton::DpadLeft as u8); }
    if buttons_low & (1 << 8) != 0 { state.buttons |= 1 << (GamepadButton::LeftBumper as u8); }
    if buttons_low & (1 << 9) != 0 { state.buttons |= 1 << (GamepadButton::RightBumper as u8); }
    if buttons_low & (1 << 10) != 0 { state.buttons |= 1 << (GamepadButton::LeftTrigger as u8); }
    if buttons_low & (1 << 11) != 0 { state.buttons |= 1 << (GamepadButton::RightTrigger as u8); }
    // DS4 bit 12 = Share (Capture), bit 13 = Options (Start) — already mapped by bit 3
    if buttons_low & (1 << 14) != 0 { state.buttons |= 1 << (GamepadButton::Guide as u8); }
    if buttons_low & (1 << 15) != 0 { state.buttons |= 1 << (GamepadButton::Misc as u8); }
    // Extra: bit 16 = touchpad click (Capture on DS4)
    if buttons_hi_and_hat & (1 << 4) != 0 { state.buttons |= 1 << (GamepadButton::Capture as u8); }

    // HAT (bits 0-3 of buttons_hi_and_hat)
    state.hat = HatPosition::from_hat_value(buttons_hi_and_hat & 0x0F);

    // Analog sticks
    if offset + 7 <= data.len() {
        let lx = data[offset + 3];
        let ly = data[offset + 4];
        let rx = data[offset + 5];
        let ry = data[offset + 6];
        state.axes[GamepadAxis::LeftStickX as usize] = (lx as i16 - 128) * 256;
        state.axes[GamepadAxis::LeftStickY as usize] = (ly as i16 - 128) * 256;
        state.axes[GamepadAxis::RightStickX as usize] = (rx as i16 - 128) * 256;
        state.axes[GamepadAxis::RightStickY as usize] = (ry as i16 - 128) * 256;
    }

    // Analog triggers
    if offset + 9 <= data.len() {
        state.axes[GamepadAxis::LeftTrigger as usize] = (data[offset + 7] as i16) * 128;
        state.axes[GamepadAxis::RightTrigger as usize] = (data[offset + 8] as i16) * 128;
    }

    state.connected = true;
    state.updated = true;
}

/// Parse a Switch Pro Controller input report.
fn parse_switch_pro_report(data: &[u8], state: &mut GamepadState) {
    if data.len() < 3 {
        return;
    }

    // Standard Switch Pro input: 8 bytes
    let offset = if data[0] == 0x30 || data[0] == 0x3F || data[0] == 0x21 { 3 } else { 0 };

    if offset + 5 > data.len() {
        return;
    }

    let btn_a = data[offset];
    let btn_b = data[offset + 1];

    // DPad (bits 0-3)
    let hat_val = btn_a & 0x0F;
    state.hat = if hat_val < 8 {
        match hat_val {
            0 => HatPosition::North,
            1 => HatPosition::NorthEast,
            2 => HatPosition::East,
            3 => HatPosition::SouthEast,
            4 => HatPosition::South,
            5 => HatPosition::SouthWest,
            6 => HatPosition::West,
            7 => HatPosition::NorthWest,
            _ => unreachable!(),
        }
    } else {
        HatPosition::Center
    };

    // ABXY
    if btn_b & (1 << 0) != 0 { state.buttons |= 1 << (GamepadButton::A as u8); }
    if btn_b & (1 << 1) != 0 { state.buttons |= 1 << (GamepadButton::B as u8); }
    if btn_b & (1 << 2) != 0 { state.buttons |= 1 << (GamepadButton::X as u8); }
    if btn_b & (1 << 3) != 0 { state.buttons |= 1 << (GamepadButton::Y as u8); }

    // L/R
    if btn_a & (1 << 4) != 0 { state.buttons |= 1 << (GamepadButton::LeftBumper as u8); }
    if btn_a & (1 << 5) != 0 { state.buttons |= 1 << (GamepadButton::RightBumper as u8); }
    if btn_a & (1 << 6) != 0 { state.buttons |= 1 << (GamepadButton::LeftTrigger as u8); }
    if btn_a & (1 << 7) != 0 { state.buttons |= 1 << (GamepadButton::RightTrigger as u8); }

    // System
    if btn_b & (1 << 4) != 0 { state.buttons |= 1 << (GamepadButton::Select as u8); }  // -
    if btn_b & (1 << 5) != 0 { state.buttons |= 1 << (GamepadButton::Start as u8); }   // +
    if btn_b & (1 << 6) != 0 { state.buttons |= 1 << (GamepadButton::LeftStick as u8); }
    if btn_b & (1 << 7) != 0 { state.buttons |= 1 << (GamepadButton::RightStick as u8); }

    // Sticks
    state.axes[GamepadAxis::LeftStickX as usize] = (data[offset + 2] as i16 - 128) * 256;
    state.axes[GamepadAxis::LeftStickY as usize] = (data[offset + 3] as i16 - 128) * 256;
    if offset + 5 <= data.len() {
        state.axes[GamepadAxis::RightStickX as usize] = (data[offset + 4] as i16 - 128) * 256;
    }

    state.connected = true;
    state.updated = true;
}

/// Parse a generic HID gamepad report (simple 6+ byte layout).
fn parse_generic_report(data: &[u8], state: &mut GamepadState) {
    if data.len() < 6 {
        return;
    }

    let offset = if data[0] == 0x01 { 1 } else { 0 };

    if offset + 6 > data.len() {
        return;
    }

    let buttons = u16::from_le_bytes([data[offset], data[offset + 1]]);

    // Standard HID button mapping
    if buttons & (1 << 0) != 0 { state.buttons |= 1 << (GamepadButton::A as u8); }
    if buttons & (1 << 1) != 0 { state.buttons |= 1 << (GamepadButton::B as u8); }
    if buttons & (1 << 2) != 0 { state.buttons |= 1 << (GamepadButton::X as u8); }
    if buttons & (1 << 3) != 0 { state.buttons |= 1 << (GamepadButton::Y as u8); }
    if buttons & (1 << 4) != 0 { state.buttons |= 1 << (GamepadButton::LeftBumper as u8); }
    if buttons & (1 << 5) != 0 { state.buttons |= 1 << (GamepadButton::RightBumper as u8); }
    if buttons & (1 << 6) != 0 { state.buttons |= 1 << (GamepadButton::Select as u8); }
    if buttons & (1 << 7) != 0 { state.buttons |= 1 << (GamepadButton::Start as u8); }
    if buttons & (1 << 8) != 0 { state.buttons |= 1 << (GamepadButton::LeftStick as u8); }
    if buttons & (1 << 9) != 0 { state.buttons |= 1 << (GamepadButton::RightStick as u8); }
    if buttons & (1 << 10) != 0 { state.buttons |= 1 << (GamepadButton::Guide as u8); }

    // DPad from high nibble
    let dpad = (buttons >> 12) & 0x0F;
    if dpad < 8 {
        state.hat = match dpad {
            0 => HatPosition::North,
            1 => HatPosition::NorthEast,
            2 => HatPosition::East,
            3 => HatPosition::SouthEast,
            4 => HatPosition::South,
            5 => HatPosition::SouthWest,
            6 => HatPosition::West,
            7 => HatPosition::NorthWest,
            _ => unreachable!(),
        };
    }

    // Sticks
    state.axes[GamepadAxis::LeftStickX as usize] = (data[offset + 2] as i16 - 128) * 256;
    state.axes[GamepadAxis::LeftStickY as usize] = (data[offset + 3] as i16 - 128) * 256;
    state.axes[GamepadAxis::RightStickX as usize] = (data[offset + 4] as i16 - 128) * 256;
    state.axes[GamepadAxis::RightStickY as usize] = (data[offset + 5] as i16 - 128) * 256;

    // Triggers
    if offset + 8 <= data.len() {
        state.axes[GamepadAxis::LeftTrigger as usize] = (data[offset + 6] as i16) * 128;
        state.axes[GamepadAxis::RightTrigger as usize] = (data[offset + 7] as i16) * 128;
    }

    state.connected = true;
    state.updated = true;
}

/// Main entry: parse a HID report into GamepadState.
fn parse_report(profile: GamepadProfile, data: &[u8]) -> GamepadState {
    let mut state = GamepadState::default();
    match profile {
        GamepadProfile::DualShock4 | GamepadProfile::DualSense => {
            parse_ds4_report(data, &mut state);
        }
        GamepadProfile::SwitchPro => {
            parse_switch_pro_report(data, &mut state);
        }
        _ => {
            parse_generic_report(data, &mut state);
        }
    }
    state
}

// ============================================================================
// Gamepad Device Handle
// ============================================================================

/// Default gamepad device path (USB HID gamepads via xHCI driver).
const DEFAULT_GAMEPAD_PATH: &str = "/dev/gamepad0";

/// HID input report buffer size (max 64 bytes for full-speed HID).
const REPORT_BUF_SIZE: usize = 64;

/// Gamepad device handle.
///
/// Opens a gamepad chardev (USB or BT) and reads HID input reports,
/// parsing them into `GamepadState` based on the detected profile.
pub struct Gamepad {
    /// File handle to the gamepad device.
    file: File,
    /// Detected HID profile for report parsing.
    profile: GamepadProfile,
    /// Latest parsed state.
    state: GamepadState,
    /// Raw report buffer.
    buf: [u8; REPORT_BUF_SIZE],
}

impl Gamepad {
    /// Open the default gamepad device (`/dev/gamepad0`).
    pub fn open() -> io::Result<Self> {
        Self::open_path(DEFAULT_GAMEPAD_PATH)
    }

    /// Open a specific gamepad device path.
    pub fn open_path(path: &str) -> io::Result<Self> {
        let file = File::open(path)?;
        Ok(Self {
            file,
            profile: GamepadProfile::Unknown,
            state: GamepadState::default(),
            buf: [0u8; REPORT_BUF_SIZE],
        })
    }

    /// Set the gamepad profile for report parsing.
    pub fn set_profile(&mut self, profile: GamepadProfile) {
        self.profile = profile;
    }

    /// Auto-detect profile from a device name string.
    pub fn set_profile_name(&mut self, name: &str) {
        self.profile = GamepadProfile::detect(name);
    }

    /// Get the current profile.
    pub fn profile(&self) -> GamepadProfile {
        self.profile
    }

    /// Get the latest parsed state (without reading).
    pub fn state(&self) -> &GamepadState {
        &self.state
    }

    /// Get mutable reference to the latest state.
    pub fn state_mut(&mut self) -> &mut GamepadState {
        &mut self.state
    }

    /// Poll for new data with a timeout in milliseconds (Unix only).
    ///
    /// On non-Unix platforms, returns `Ok(false)` (no data available).
    pub fn poll(&mut self, _timeout_ms: u32) -> io::Result<bool> {
        #[cfg(unix)]
        {
            let fd = self.file.as_raw_fd();
            unsafe {
                let mut fds: libc::fd_set = std::mem::zeroed();
                libc::FD_SET(fd, &mut fds);

                let mut tv = libc::timeval {
                    tv_sec: (_timeout_ms / 1000) as libc::time_t,
                    tv_usec: ((_timeout_ms % 1000) * 1000) as libc::suseconds_t,
                };

                let ret = libc::select(
                    fd + 1,
                    &mut fds,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut tv,
                );

                if ret < 0 {
                    return Err(io::Error::last_os_error());
                } else {
                    return Ok(libc::FD_ISSET(fd, &fds));
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = &self.file; // suppress unused warning
            Ok(false)
        }
    }

    /// Read and parse the latest HID input report (Unix only).
    ///
    /// On non-Unix platforms, returns the cached state with updated=false.
    pub fn read_state(&mut self) -> io::Result<GamepadState> {
        #[cfg(unix)]
        {
            let fd = self.file.as_raw_fd();
            let n = unsafe {
                libc::read(
                    fd,
                    self.buf.as_mut_ptr() as *mut libc::c_void,
                    REPORT_BUF_SIZE,
                )
            };

            if n < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::WouldBlock {
                    let mut state = self.state.clone();
                    state.updated = false;
                    return Ok(state);
                }
                return Err(err);
            }

            if n == 0 {
                self.state = GamepadState::disconnected();
                return Ok(self.state.clone());
            }

            let data = &self.buf[..n as usize];
            self.state = parse_report(self.profile, data);

            if self.profile == GamepadProfile::Unknown {
                self.profile = match n {
                    48 => GamepadProfile::DualShock4,
                    8 => GamepadProfile::SwitchPro,
                    _ => GamepadProfile::Generic,
                };
            }

            Ok(self.state.clone())
        }
        #[cfg(not(unix))]
        {
            let mut state = self.state.clone();
            state.updated = false;
            Ok(state)
        }
    }

    /// Check if the gamepad is connected.
    pub fn is_connected(&self) -> bool {
        self.state.connected
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gamepad_state_default() {
        let state = GamepadState::default();
        assert!(!state.connected);
        assert!(!state.updated);
        assert_eq!(state.buttons, 0);
        assert_eq!(state.hat, HatPosition::Center);
    }

    #[test]
    fn test_gamepad_state_buttons() {
        let mut state = GamepadState::default();
        state.buttons = 1 << (GamepadButton::A as u8);
        assert!(state.is_pressed(GamepadButton::A));
        assert!(!state.is_pressed(GamepadButton::B));
        assert!(!state.is_pressed(GamepadButton::Guide));
    }

    #[test]
    fn test_gamepad_axis_norm() {
        let mut state = GamepadState::default();
        state.axes[GamepadAxis::LeftStickX as usize] = 32767;
        let norm = state.axis_norm(GamepadAxis::LeftStickX);
        assert!((norm - 1.0).abs() < 0.001);

        state.axes[GamepadAxis::LeftStickX as usize] = -32768;
        let norm = state.axis_norm(GamepadAxis::LeftStickX);
        assert!((norm + 1.0).abs() < 0.001);
    }

    #[test]
    fn test_trigger_norm() {
        let mut state = GamepadState::default();
        state.axes[GamepadAxis::LeftTrigger as usize] = 16384;
        let norm = state.trigger_norm(GamepadAxis::LeftTrigger);
        assert!((norm - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_hat_position() {
        assert_eq!(HatPosition::from_hat_value(0x00), HatPosition::North);
        assert_eq!(HatPosition::from_hat_value(0x03), HatPosition::SouthEast);
        assert_eq!(HatPosition::from_hat_value(0x0F), HatPosition::Center);
    }

    #[test]
    fn test_profile_detection() {
        assert_eq!(GamepadProfile::detect("Wireless Controller"), GamepadProfile::DualShock4);
        assert_eq!(GamepadProfile::detect("DualSense"), GamepadProfile::DualSense);
        assert_eq!(GamepadProfile::detect("Pro Controller"), GamepadProfile::SwitchPro);
        assert_eq!(GamepadProfile::detect("Xbox Wireless Controller"), GamepadProfile::XboxBle);
        assert_eq!(GamepadProfile::detect("8BitDo SN30 Pro"), GamepadProfile::Generic);
        assert_eq!(GamepadProfile::detect("Unknown Device"), GamepadProfile::Unknown);
    }

    #[test]
    fn test_parse_ds4_report() {
        let mut report = [0u8; 48];
        report[0] = 0x01;  // report ID
        report[1] = 0xFF;  // buttons low
        report[2] = 0xFF;  // buttons high
        report[3] = 0x05;  // HAT=East (0x02) + capture bit
        report[4] = 0xFF;  // LX = max
        report[5] = 0x00;  // LY = max
        report[6] = 0x80;  // RX = center
        report[7] = 0x80;  // RY = center
        report[8] = 0xFF;  // L2 = max
        report[9] = 0xFF;  // R2 = max

        let state = parse_report(GamepadProfile::DualShock4, &report);
        assert!(state.connected);
        assert!(state.is_pressed(GamepadButton::Select));
        assert!(state.is_pressed(GamepadButton::DpadUp));
        assert!(state.is_pressed(GamepadButton::LeftBumper));
        assert!(state.axis_norm(GamepadAxis::LeftStickX) > 0.9);
        assert!(state.trigger_norm(GamepadAxis::LeftTrigger) > 0.9);
    }

    #[test]
    fn test_parse_switch_pro_report() {
        let mut report = [0u8; 8];
        report[0] = 0x30;
        report[3] = 0x00;  // DPad center
        report[4] = 0b00000111;  // A + B + X

        let state = parse_report(GamepadProfile::SwitchPro, &report);
        assert!(state.is_pressed(GamepadButton::A));
        assert!(state.is_pressed(GamepadButton::B));
        assert!(state.is_pressed(GamepadButton::X));
        assert!(!state.is_pressed(GamepadButton::Y));
        assert_eq!(state.hat, HatPosition::Center);
    }

    #[test]
    fn test_parse_generic_report() {
        let mut report = [0u8; 8];
        report[1] = 0b00000011;  // A + B
        report[2] = 0x80;  // LX center
        report[3] = 0x80;  // LY center
        report[4] = 0x80;  // RX center
        report[5] = 0x80;  // RY center

        let state = parse_report(GamepadProfile::Generic, &report);
        assert!(state.connected);
        assert!(state.is_pressed(GamepadButton::A));
        assert!(state.is_pressed(GamepadButton::B));
        assert!(!state.is_pressed(GamepadButton::X));
    }

    #[test]
    fn test_empty_report() {
        let state = parse_report(GamepadProfile::DualShock4, &[]);
        assert!(!state.connected);
    }

    #[test]
    fn test_short_report() {
        let state = parse_report(GamepadProfile::Generic, &[0x01, 0x00, 0x01]);
        assert!(!state.connected);
    }

    #[test]
    fn test_set_profile_name() {
        // Just test profile detection via set_profile_name — the actual
        // auto-detect-from-size logic in read_state() requires a real fd.
        assert_eq!(GamepadProfile::detect("Wireless Controller"), GamepadProfile::DualShock4);
        assert_eq!(GamepadProfile::detect("Unknown"), GamepadProfile::Unknown);
    }

    #[test]
    fn test_disconnected_state() {
        let state = GamepadState::disconnected();
        assert!(!state.connected);
    }

    #[test]
    fn test_mark_read() {
        let mut state = GamepadState {
            connected: true,
            updated: true,
            ..Default::default()
        };
        state.mark_read();
        assert!(!state.updated);
    }

    #[test]
    fn test_is_pressed_multiple() {
        let mut state = GamepadState::default();
        state.buttons = (1 << (GamepadButton::A as u8))
                      | (1 << (GamepadButton::B as u8))
                      | (1 << (GamepadButton::Guide as u8));
        assert!(state.is_pressed(GamepadButton::A));
        assert!(state.is_pressed(GamepadButton::B));
        assert!(state.is_pressed(GamepadButton::Guide));
        assert!(!state.is_pressed(GamepadButton::X));
    }
}
