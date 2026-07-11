//! # HID over GATT Profile (HOGP) — Bluetooth Gamepad Support
//!
//! Implements the Bluetooth HID over GATT Profile for BLE gamepads
//! (DualShock 4, DualSense, Switch Pro, Xbox Wireless via BLE, etc.).
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────┐
//! │                  GamepadState                            │
//! │  buttons: 32-bit mask                                    │
//! │  axes: [Lx, Ly, Rx, Ry, L2, R2]                         │
//! │  hat: N/NE/E/SE/S/SW/W/NW/Center                        │
//! └──────────────────────┬───────────────────────────────────┘
//!                        │ parse_hid_report()
//! ┌──────────────────────▼───────────────────────────────────┐
//! │               HidGamepadProfile trait                     │
//! │  - ds4: DualShock 4 layout (48 bytes)                    │
//! │  - ds5: DualSense layout (48 bytes)                      │
//! │  - switch_pro: Switch Pro layout (8 bytes)               │
//! │  - xbox_ble: Xbox BLE layout (variable)                  │
//! │  - generic: generic HID gamepad (via report descriptor)  │
//! └──────────────────────┬───────────────────────────────────┘
//!                        │
//! ┌──────────────────────▼───────────────────────────────────┐
//! │            HidOverGattClient                             │
//! │  - discover HID service via GATT                         │
//! │  - read report descriptor (0x2A4B)                       │
//! │  - subscribe to input report (0x2A4D) via CCCD          │
//! │  - receive notifications → parse → GamepadState          │
//! └──────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Standard HID over GATT UUIDs
//!
//! | UUID   | Name                  | Type     |
//! |--------|-----------------------|----------|
//! | 0x1812 | HID Service           | Service  |
//! | 0x2A4A | HID Information       | Char     |
//! | 0x2A4B | Report Map (descriptor)| Char     |
//! | 0x2A4D | Report (input/output)  | Char     |
//! | 0x2A4E | Protocol Mode         | Char     |
//! | 0x2A4C | HID Control Point     | Char     |
//! | 0x2908 | Report Reference      | Desc     |

#![allow(dead_code)]

use crate::gatt::{
    gatt_uuids, CharProperties, GattCharacteristic, GattClient, GattService,
};
use crate::types::BtUuid;

// ============================================================================
// HID over GATT UUIDs
// ============================================================================

/// HID Service UUID (0x1812).
pub const HID_SERVICE_UUID: BtUuid = BtUuid::from_uuid16(0x1812);
/// HID Information characteristic (0x2A4A).
pub const HID_INFORMATION_UUID: BtUuid = BtUuid::from_uuid16(0x2A4A);
/// Report Map characteristic (0x2A4B).
pub const REPORT_MAP_UUID: BtUuid = BtUuid::from_uuid16(0x2A4B);
/// Report characteristic (0x2A4D) — input, output, or feature.
pub const REPORT_UUID: BtUuid = BtUuid::from_uuid16(0x2A4D);
/// Protocol Mode characteristic (0x2A4E).
pub const PROTOCOL_MODE_UUID: BtUuid = BtUuid::from_uuid16(0x2A4E);
/// HID Control Point characteristic (0x2A4C).
pub const HID_CONTROL_POINT_UUID: BtUuid = BtUuid::from_uuid16(0x2A4C);
/// Report Reference descriptor (0x2908).
pub const REPORT_REFERENCE_UUID: BtUuid = BtUuid::from_uuid16(0x2908);

// HID information flags
pub const HID_INFO_FLAG_REMOTE_WAKE: u8 = 0x01;
pub const HID_INFO_FLAG_NORMALLY_CONNECTABLE: u8 = 0x02;

// Report types (from Report Reference descriptor)
pub const REPORT_TYPE_INPUT: u8 = 0x01;
pub const REPORT_TYPE_OUTPUT: u8 = 0x02;
pub const REPORT_TYPE_FEATURE: u8 = 0x03;

// Protocol Mode values
pub const PROTOCOL_MODE_REPORT: u8 = 0x01;
pub const PROTOCOL_MODE_BOOT: u8 = 0x00;

// ============================================================================
// Gamepad Button/Axis constants
// ============================================================================

/// Standard gamepad button bit positions (up to 32 buttons).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum GamepadButton {
    A = 0,
    B = 1,
    X = 2,
    Y = 3,
    LeftBumper = 4,
    RightBumper = 5,
    LeftTrigger = 6,  // digital (L2)
    RightTrigger = 7, // digital (R2)
    Select = 8,       // or Back, -,
    Start = 9,        // or Forward, +,
    LeftStick = 10,   // L3
    RightStick = 11,  // R3
    Guide = 12,       // PS/Xbox/Home
    Capture = 13,     // Share/Capture
    DpadUp = 14,
    DpadDown = 15,
    DpadLeft = 16,
    DpadRight = 17,
    Misc = 18,
}

/// Gamepad axis identifiers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum GamepadAxis {
    LeftStickX = 0,
    LeftStickY = 1,
    RightStickX = 2,
    RightStickY = 3,
    LeftTrigger = 4,  // analog L2 (0..255 or 0..1023)
    RightTrigger = 5, // analog R2
    AccelX = 6,       // IMU (DS4/DS5)
    AccelY = 7,
    AccelZ = 8,
    GyroX = 9,
    GyroY = 10,
    GyroZ = 11,
}

/// Hat switch position (8 directions + center).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[repr(u8)]
pub enum HatPosition {
    #[default]
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

    pub fn is_center(&self) -> bool {
        *self == Self::Center
    }

    pub fn to_dpad_bits(&self) -> (bool, bool, bool, bool) {
        match self {
            HatPosition::North => (true, false, false, false),
            HatPosition::NorthEast => (true, true, false, false),
            HatPosition::East => (false, true, false, false),
            HatPosition::SouthEast => (false, true, true, false),
            HatPosition::South => (false, false, true, false),
            HatPosition::SouthWest => (false, false, true, true),
            HatPosition::West => (false, false, false, true),
            HatPosition::NorthWest => (true, false, false, true),
            HatPosition::Center => (false, false, false, false),
        }
    }
}

// ============================================================================
// Gamepad State
// ============================================================================

/// Full gamepad state snapshot.
#[derive(Clone, Debug, Default)]
pub struct GamepadState {
    /// Bitmask of pressed buttons (32 bits).
    pub buttons: u32,
    /// Axis values (signed 16-bit, range -32768..32767).
    /// Indexed by `GamepadAxis` variants.
    pub axes: [i16; 12],
    /// Hat position (DPad on many gamepads).
    pub hat: HatPosition,
    /// Battery level (0..100, 101 = unknown).
    pub battery: u8,
    /// Whether the gamepad is connected.
    pub connected: bool,
    /// Whether the state was updated since last read.
    pub updated: bool,
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
        // Triggers are typically 0..255 or 0..1023, mapped to signed 16-bit
        (self.axes[idx].max(0) as f32) / 32767.0
    }

    /// Mark state as read.
    pub fn mark_read(&mut self) {
        self.updated = false;
    }

    /// Create an empty (disconnected) state.
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            ..Default::default()
        }
    }
}

// ============================================================================
// HID Report Descriptor Parser (simplified)
// ============================================================================

/// Parsed HID report descriptor — identifies gamepad layout.
///
/// This is a simplified parser that extracts enough information to
/// correctly interpret gamepad reports from known devices. For a full
/// HID report descriptor parser, see the USB HID specification.
#[derive(Clone, Debug, Default)]
pub struct HidReportLayout {
    /// Total input report size in bytes.
    pub input_report_size: u8,
    /// Number of buttons in the report.
    pub button_count: u8,
    /// Number of axes (analog sticks + triggers).
    pub axis_count: u8,
    /// Whether the report uses HAT switch (DPad on hat).
    pub has_hat: bool,
    /// Whether the report has IMU data (accelerometer/gyro).
    pub has_imu: bool,
    /// Byte offsets for each field (if determinable).
    pub button_offset: u8,
    pub axis_offset: u8,
    pub hat_offset: u8,
}

/// Try to detect the gamepad type from the BLE device name or appearance.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HidGamepadProfile {
    /// Sony DualShock 4 (48-byte input report).
    DualShock4,
    /// Sony DualSense (PS5) (48-byte input report).
    DualSense,
    /// Nintendo Switch Pro Controller (8-byte input report, standard).
    SwitchPro,
    /// Xbox Wireless Controller via BLE (variable report).
    XboxBle,
    /// 8BitDo or other generic HID gamepad.
    Generic,
    /// Unknown — needs report descriptor parsing.
    Unknown,
}

impl HidGamepadProfile {
    /// Detect profile from device name and appearance.
    pub fn detect(name: &str, _appearance: Option<u16>) -> Self {
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
        if lower.contains("xbox") || lower.contains("xbox controller") {
            return Self::XboxBle;
        }
        if lower.contains("8bitdo") || lower.contains("8bit") {
            return Self::Generic;
        }
        Self::Unknown
    }

    /// Expected input report size in bytes.
    pub fn input_report_size(&self) -> u8 {
        match self {
            Self::DualShock4 => 48,   // DS4 USB/BT input report
            Self::DualSense => 48,     // DualSense USB/BT input report (varies by mode)
            Self::SwitchPro => 8,      // Standard Switch Pro input report
            Self::XboxBle => 20,       // Xbox BLE input report (varies)
            Self::Generic | Self::Unknown => 0, // Need to read report descriptor
        }
    }
}

// ============================================================================
// HID Report Parser (per-profile)
// ============================================================================

/// Parse a DualShock 4 input report into GamepadState.
/// DS4 USB report (64 bytes) or BT report (78 bytes) — common 48-byte variant.
fn parse_ds4_report(data: &[u8], state: &mut GamepadState) {
    if data.len() < 6 {
        return;
    }

    // DS4 report layout (simplified):
    // Byte 0: report ID (varies, often 0x01 for USB, 0x11 for BT)
    // Bytes 1-2: buttons (16-bit little-endian)
    // Byte 3: [hat:4, buttons_hi:4]
    // Byte 4: LeftStickX (0..255)
    // Byte 5: LeftStickY (0..255)
    // Byte 6: RightStickX (0..255)
    // Byte 7: RightStickY (0..255)
    // Bytes 8-9: trigger analog (L2, R2) (0..255 each)

    let offset = if data[0] == 0x01 || data[0] == 0x11 { 1 } else { 0 };

    // Buttons (bits 0-15)
    let buttons_low = if offset + 3 <= data.len() {
        u16::from_le_bytes([data[offset], data[offset + 1]])
    } else {
        0
    };
    // Buttons (bits 16-19) + hat (bits 20-23)
    let buttons_hi_and_hat = if offset + 3 <= data.len() {
        data[offset + 2]
    } else {
        0
    };

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
    // DS4 bit 12 = Share (→ Capture), bit 13 = Options (→ Start, but bit 3 already maps Start)
    if buttons_low & (1 << 12) != 0 { state.buttons |= 1 << (GamepadButton::Capture as u8); }
    if buttons_low & (1 << 13) != 0 { state.buttons |= 1 << (GamepadButton::Start as u8); }
    if buttons_low & (1 << 14) != 0 { state.buttons |= 1 << (GamepadButton::Guide as u8); }
    if buttons_low & (1 << 15) != 0 { state.buttons |= 1 << (GamepadButton::Misc as u8); }

    // Extra buttons: bit 16 = touchpad click, bit 17 = mic button
    if buttons_hi_and_hat & (1 << 4) != 0 { state.buttons |= 1 << (GamepadButton::Capture as u8); }

    // HAT (bits 0-3 of buttons_hi_and_hat)
    state.hat = HatPosition::from_hat_value(buttons_hi_and_hat & 0x0F);

    // Analog sticks
    if offset + 7 <= data.len() {
        let lx = data[offset + 3];
        let ly = data[offset + 4];
        let rx = data[offset + 5];
        let ry = data[offset + 6];

        // Map 0..255 to -32768..32767
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

/// Parse a Nintendo Switch Pro Controller input report.
fn parse_switch_pro_report(data: &[u8], state: &mut GamepadState) {
    if data.len() < 3 {
        return;
    }

    // Switch Pro standard input report: 8 bytes
    // Byte 0: report ID (likely 0x30 for standard input)
    // Byte 1: timer / Connection info
    // Byte 2: battery / connection
    // Byte 3: buttons A (dpad + L/R basics)
    // Byte 4: buttons B (ABXY + L3/R3 + Home/Capture)
    // Byte 5: LeftStickX
    // Byte 6: LeftStickY
    // Byte 7: RightStickX

    // Skip report ID byte if first byte is typical for Switch
    let offset = if data[0] == 0x30 || data[0] == 0x3F || data[0] == 0x21 { 3 } else { 0 };

    if offset + 5 > data.len() {
        return;
    }

    let btn_a = data[offset];     // DPad (4 bits) | L/R basics
    let btn_b = data[offset + 1]; // ABXY | L3/R3

    // DPad (bits 0-3)
    let hat_val = btn_a & 0x0F;
    if hat_val < 8 {
        // DPad to HAT mapping for Switch Pro (CW order, 0=up)
        state.hat = match hat_val {
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
    } else {
        state.hat = HatPosition::Center;
    }

    // Right-side buttons (ABXY)
    if btn_b & (1 << 0) != 0 { state.buttons |= 1 << (GamepadButton::A as u8); }
    if btn_b & (1 << 1) != 0 { state.buttons |= 1 << (GamepadButton::B as u8); }
    if btn_b & (1 << 2) != 0 { state.buttons |= 1 << (GamepadButton::X as u8); }
    if btn_b & (1 << 3) != 0 { state.buttons |= 1 << (GamepadButton::Y as u8); }

    // L/R buttons
    if btn_a & (1 << 4) != 0 { state.buttons |= 1 << (GamepadButton::LeftBumper as u8); }
    if btn_a & (1 << 5) != 0 { state.buttons |= 1 << (GamepadButton::RightBumper as u8); }
    if btn_a & (1 << 6) != 0 { state.buttons |= 1 << (GamepadButton::LeftTrigger as u8); }
    if btn_a & (1 << 7) != 0 { state.buttons |= 1 << (GamepadButton::RightTrigger as u8); }

    // System buttons
    if btn_b & (1 << 4) != 0 { state.buttons |= 1 << (GamepadButton::Select as u8); }   // -
    if btn_b & (1 << 5) != 0 { state.buttons |= 1 << (GamepadButton::Start as u8); }    // +
    if btn_b & (1 << 6) != 0 { state.buttons |= 1 << (GamepadButton::LeftStick as u8); }
    if btn_b & (1 << 7) != 0 { state.buttons |= 1 << (GamepadButton::RightStick as u8); }
    // The Guide (Home) button is separate on Switch Pro
    // Capture button doesn't exist on Switch Pro

    // Analog sticks (only LeftStick in this report, RightStick via separate bytes)
    let lx = data[offset + 2];
    let ly = data[offset + 3];
    state.axes[GamepadAxis::LeftStickX as usize] = (lx as i16 - 128) * 256;
    state.axes[GamepadAxis::LeftStickY as usize] = (ly as i16 - 128) * 256;

    // RightStick X (if available)
    if offset + 5 <= data.len() {
        let rx = data[offset + 4];
        state.axes[GamepadAxis::RightStickX as usize] = (rx as i16 - 128) * 256;
    }

    state.connected = true;
    state.updated = true;
}

/// Parse a generic HID gamepad report (simple 6-byte layout).
fn parse_generic_report(data: &[u8], state: &mut GamepadState) {
    if data.len() < 6 {
        return;
    }

    // Generic HID gamepad: many controllers use a simple layout
    // when in standard HID mode (no proprietary protocol).
    // Layout: [buttons(2) | LX(1) | LY(1) | RX(1) | RY(1)]

    let offset = if data[0] == 0x01 { 1 } else { 0 };

    if offset + 6 > data.len() {
        return;
    }

    let buttons = u16::from_le_bytes([data[offset], data[offset + 1]]);

    // Standard button mapping (many HID gamepads)
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

    // DPad from high 4 bits of first button byte (6-button mode)
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

    // Sticks (0..255 → -32768..32767)
    let lx = data[offset + 2];
    let ly = data[offset + 3];
    let rx = data[offset + 4];
    let ry = data[offset + 5];

    state.axes[GamepadAxis::LeftStickX as usize] = (lx as i16 - 128) * 256;
    state.axes[GamepadAxis::LeftStickY as usize] = (ly as i16 - 128) * 256;
    state.axes[GamepadAxis::RightStickX as usize] = (rx as i16 - 128) * 256;
    state.axes[GamepadAxis::RightStickY as usize] = (ry as i16 - 128) * 256;

    // Triggers (if available)
    if offset + 8 <= data.len() {
        state.axes[GamepadAxis::LeftTrigger as usize] = (data[offset + 6] as i16) * 128;
        state.axes[GamepadAxis::RightTrigger as usize] = (data[offset + 7] as i16) * 128;
    }

    state.connected = true;
    state.updated = true;
}

/// Main entry point: parse a HID input report into GamepadState.
pub fn parse_hid_report(profile: HidGamepadProfile, data: &[u8]) -> GamepadState {
    let mut state = GamepadState::default();

    match profile {
        HidGamepadProfile::DualShock4 | HidGamepadProfile::DualSense => {
            parse_ds4_report(data, &mut state);
        }
        HidGamepadProfile::SwitchPro => {
            parse_switch_pro_report(data, &mut state);
        }
        HidGamepadProfile::XboxBle => {
            // Xbox BLE uses a more complex report structure.
            // Fall back to generic parser for basic button/axis support.
            parse_generic_report(data, &mut state);
        }
        HidGamepadProfile::Generic | HidGamepadProfile::Unknown => {
            parse_generic_report(data, &mut state);
        }
    }

    state
}

// ============================================================================
// HID over GATT Client
// ============================================================================

/// Discovered HID service handles for a BLE gamepad.
#[derive(Clone, Debug)]
pub struct HidGattHandles {
    /// HID service range.
    pub service_start: u16,
    pub service_end: u16,
    /// HID Information value handle.
    pub hid_info_handle: Option<u16>,
    /// Report Map value handle.
    pub report_map_handle: Option<u16>,
    /// Input Report value handle.
    pub input_report_handle: Option<u16>,
    /// Input Report CCCD handle (for enabling notifications).
    pub input_report_cccd: Option<u16>,
    /// Protocol Mode value handle.
    pub protocol_mode_handle: Option<u16>,
    /// HID Control Point handle.
    pub control_point_handle: Option<u16>,
}

impl HidGattHandles {
    pub fn new() -> Self {
        Self {
            service_start: 0,
            service_end: 0,
            hid_info_handle: None,
            report_map_handle: None,
            input_report_handle: None,
            input_report_cccd: None,
            protocol_mode_handle: None,
            control_point_handle: None,
        }
    }
}

/// Take discovered services and find HID-related handles.
/// This would be called after service/characteristic discovery via GattClient.
pub fn find_hid_handles(
    services: &[GattService],
    characteristics: &[GattCharacteristic],
) -> HidGattHandles {
    let mut handles = HidGattHandles::new();

    // Find HID service range
    for svc in services {
        if svc.uuid == HID_SERVICE_UUID {
            handles.service_start = svc.start_handle;
            handles.service_end = svc.end_handle;
            break;
        }
    }

    // Find characteristics within the HID service
    for ch in characteristics {
        if ch.uuid == HID_INFORMATION_UUID {
            handles.hid_info_handle = Some(ch.value_handle);
        } else if ch.uuid == REPORT_MAP_UUID {
            handles.report_map_handle = Some(ch.value_handle);
        } else if ch.uuid == REPORT_UUID {
            handles.input_report_handle = Some(ch.value_handle);
            // CCCD is typically value_handle + 1
            if ch.cccd_handle.is_some() {
                handles.input_report_cccd = ch.cccd_handle;
            } else {
                handles.input_report_cccd = Some(ch.value_handle + 1);
            }
        } else if ch.uuid == PROTOCOL_MODE_UUID {
            handles.protocol_mode_handle = Some(ch.value_handle);
        } else if ch.uuid == HID_CONTROL_POINT_UUID {
            handles.control_point_handle = Some(ch.value_handle);
        }
    }

    handles
}

/// Build a CCCD write value to enable notifications.
pub fn build_cccd_enable_notify() -> Vec<u8> {
    vec![0x01, 0x00] // Little-endian: notifications enabled
}

/// Build a CCCD write value to enable indications.
pub fn build_cccd_enable_indicate() -> Vec<u8> {
    vec![0x02, 0x00] // Little-endian: indications enabled
}

/// Build a CCCD write value to disable everything.
pub fn build_cccd_disable() -> Vec<u8> {
    vec![0x00, 0x00]
}

// ============================================================================
// BT HID Device Profile Registration (for SDP)
// ============================================================================

/// Build SDP attributes for HID service (for registering as a HID host).
/// This is used when we want to advertise HID host support.
pub fn build_hid_host_service_record() -> crate::sdp_record::ServiceRecord {
    // HID Host UUID = 0x1200
    let mut record = crate::sdp_record::ServiceRecord::new(0);

    record.set_attr(
        crate::sdp_record::SdpAttrId::SERVICE_CLASS_ID_LIST,
        crate::sdp_record::DataElement::Seq(vec![
            // HID Host service class
            crate::sdp_record::DataElement::Uuid(BtUuid::from_uuid16(0x1200)),
        ]),
    );

    record.set_attr(
        crate::sdp_record::SdpAttrId::PROTOCOL_DESCRIPTOR_LIST,
        crate::sdp_record::DataElement::Seq(vec![
            crate::sdp_record::DataElement::Seq(vec![
                crate::sdp_record::DataElement::Uuid(crate::types::sdp_uuids::L2CAP),
            ]),
        ]),
    );

    record.set_attr(
        crate::sdp_record::SdpAttrId::BROWSE_GROUP_LIST,
        crate::sdp_record::DataElement::Seq(vec![
            crate::sdp_record::DataElement::Uuid(BtUuid::from_uuid16(0x1002)),
        ]),
    );

    record.set_attr(
        0x0100,
        crate::sdp_record::DataElement::String(b"GergiOS HID Host\0".to_vec()),
    );

    record
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hid_service_uuids() {
        assert_eq!(HID_SERVICE_UUID.as_uuid16(), Some(0x1812));
        assert_eq!(REPORT_UUID.as_uuid16(), Some(0x2A4D));
        assert_eq!(REPORT_MAP_UUID.as_uuid16(), Some(0x2A4B));
        assert_eq!(REPORT_REFERENCE_UUID.as_uuid16(), Some(0x2908));
        assert_eq!(PROTOCOL_MODE_UUID.as_uuid16(), Some(0x2A4E));
    }

    #[test]
    fn test_hat_position() {
        assert_eq!(HatPosition::from_hat_value(0x00), HatPosition::North);
        assert_eq!(HatPosition::from_hat_value(0x01), HatPosition::NorthEast);
        assert_eq!(HatPosition::from_hat_value(0x02), HatPosition::East);
        assert_eq!(HatPosition::from_hat_value(0x04), HatPosition::South);
        assert_eq!(HatPosition::from_hat_value(0x0F), HatPosition::Center);

        let (up, right, down, left) = HatPosition::NorthWest.to_dpad_bits();
        assert!(up);
        assert!(!right);
        assert!(!down);
        assert!(left);
    }

    #[test]
    fn test_gamepad_state_default() {
        let state = GamepadState::default();
        assert!(!state.connected);
        assert_eq!(state.buttons, 0);
        assert_eq!(state.axes, [0i16; 12]);
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
    fn test_ds4_report_parsing() {
        // Simulate a DS4 report: all buttons pressed, max sticks
        let mut report = [0u8; 48];
        report[0] = 0x01;  // report ID
        report[1] = 0xFF;  // buttons low byte (bits 0-7)
        report[2] = 0xFF;  // buttons high byte (bits 8-15)
        report[3] = 0x0F;  // HAT=SouthEast (0x03) + extra buttons

        // Sticks at max
        report[4] = 0xFF;  // LX
        report[5] = 0x00;  // LY (0 = max Y on DS4)
        report[6] = 0x80;  // RX (center)
        report[7] = 0x80;  // RY (center)

        // Triggers at max
        report[8] = 0xFF;  // L2
        report[9] = 0xFF;  // R2

        let state = parse_hid_report(HidGamepadProfile::DualShock4, &report);
        assert!(state.connected);
        assert!(state.is_pressed(GamepadButton::Select));
        assert!(state.is_pressed(GamepadButton::Select));
        assert!(state.is_pressed(GamepadButton::LeftStick));
        assert!(state.is_pressed(GamepadButton::DpadUp));
        assert!(state.is_pressed(GamepadButton::LeftBumper));
        assert!(state.is_pressed(GamepadButton::Guide));

        // HAT
        assert!(state.hat == HatPosition::SouthEast || state.hat != HatPosition::Center);

        // Sticks
        assert!(state.axis_norm(GamepadAxis::LeftStickX) > 0.9);
    }

    #[test]
    fn test_switch_pro_report_parsing() {
        let mut report = [0u8; 8];
        report[0] = 0x30;  // report ID
        report[1] = 0x00;  // timer
        report[2] = 0x00;  // battery
        report[3] = 0x00;  // btn_a (DPad=center, no L/R)
        // btn_b: A + B + X pressed (bits 0,1,2)
        report[4] = 0b00000111;  // A | B | X
        report[5] = 0x80;  // LX (center = 128)
        report[6] = 0x80;  // LY (center)
        report[7] = 0x80;  // RX (center)

        let state = parse_hid_report(HidGamepadProfile::SwitchPro, &report);
        assert!(state.is_pressed(GamepadButton::A));
        assert!(state.is_pressed(GamepadButton::B));
        assert!(state.is_pressed(GamepadButton::X));
        assert!(!state.is_pressed(GamepadButton::Y));
    }

    #[test]
    fn test_generic_report_parsing() {
        let mut report = [0u8; 8];
        report[0] = 0x01;  // report ID
        report[1] = 0b00000011;  // buttons: A + B pressed
        report[2] = 0x80;  // LX (center)
        report[3] = 0x80;  // LY (center)
        report[4] = 0x80;  // RX (center)
        report[5] = 0x80;  // RY (center)

        let state = parse_hid_report(HidGamepadProfile::Generic, &report);
        assert!(state.is_pressed(GamepadButton::A));
        assert!(state.is_pressed(GamepadButton::B));
        assert!(!state.is_pressed(GamepadButton::X));
    }

    #[test]
    fn test_profile_detection() {
        assert_eq!(
            HidGamepadProfile::detect("Wireless Controller", None),
            HidGamepadProfile::DualShock4
        );
        assert_eq!(
            HidGamepadProfile::detect("DualSense", None),
            HidGamepadProfile::DualSense
        );
        assert_eq!(
            HidGamepadProfile::detect("Pro Controller", None),
            HidGamepadProfile::SwitchPro
        );
        assert_eq!(
            HidGamepadProfile::detect("Xbox Wireless Controller", None),
            HidGamepadProfile::XboxBle
        );
        assert_eq!(
            HidGamepadProfile::detect("8BitDo SN30 Pro", None),
            HidGamepadProfile::Generic
        );
        assert_eq!(
            HidGamepadProfile::detect("Unknown Device", None),
            HidGamepadProfile::Unknown
        );
    }

    #[test]
    fn test_hid_handles_discovery() {
        let services = vec![GattService {
            uuid: HID_SERVICE_UUID,
            primary: true,
            start_handle: 0x0010,
            end_handle: 0x0020,
        }];

        let characteristics = vec![
            GattCharacteristic {
                uuid: HID_INFORMATION_UUID,
                properties: CharProperties::from_bits(CharProperties::READ),
                declaration_handle: 0x0011,
                value_handle: 0x0012,
                cccd_handle: None,
            },
            GattCharacteristic {
                uuid: REPORT_UUID,
                properties: CharProperties::from_bits(CharProperties::READ | CharProperties::NOTIFY),
                declaration_handle: 0x0013,
                value_handle: 0x0014,
                cccd_handle: Some(0x0015),
            },
        ];

        let handles = find_hid_handles(&services, &characteristics);
        assert_eq!(handles.service_start, 0x0010);
        assert_eq!(handles.hid_info_handle, Some(0x0012));
        assert_eq!(handles.input_report_handle, Some(0x0014));
        assert_eq!(handles.input_report_cccd, Some(0x0015));
    }

    #[test]
    fn test_cccd_values() {
        assert_eq!(build_cccd_enable_notify(), vec![0x01, 0x00]);
        assert_eq!(build_cccd_enable_indicate(), vec![0x02, 0x00]);
        assert_eq!(build_cccd_disable(), vec![0x00, 0x00]);
    }

    #[test]
    fn test_disconnected_state() {
        let state = GamepadState::disconnected();
        assert!(!state.connected);
        assert!(!state.updated);
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
    fn test_input_report_size() {
        assert_eq!(HidGamepadProfile::DualShock4.input_report_size(), 48);
        assert_eq!(HidGamepadProfile::SwitchPro.input_report_size(), 8);
        assert_eq!(HidGamepadProfile::Generic.input_report_size(), 0);
    }

    #[test]
    fn test_hid_host_service_record() {
        let record = build_hid_host_service_record();
        let class_uuids = record.service_class_uuids();
        assert_eq!(class_uuids.len(), 1);
        assert_eq!(class_uuids[0].as_uuid16(), Some(0x1200));
    }

    #[test]
    fn test_parse_hid_report_empty() {
        let state = parse_hid_report(HidGamepadProfile::DualShock4, &[]);
        assert!(!state.connected);
    }

    #[test]
    fn test_parse_hid_report_too_short() {
        let state = parse_hid_report(HidGamepadProfile::Generic, &[0x01, 0x00]);
        assert!(!state.connected);
    }
}
