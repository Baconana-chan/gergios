//! # USB HID Class Driver (Keyboard/Mouse)
//!
//! Implements the USB HID (Human Interface Device) class driver with:
//! - HID report descriptor parsing (short items)
//! - HID report data extraction via usage tracking
//! - Keyboard state tracking (8 modifiers + up to 6 simultaneous keys)
//! - Mouse state tracking (3 buttons + X/Y relative movement + wheel)
//! - Interrupt endpoint polling
//! - Integration with the USB device framework (UsbClassDriver trait)
//!
//! ## Report Descriptor Parser
//!
//! Parses HID short items to determine the report layout for Input items.
//! Tracks: usage page, usages, report size/count, and input flags
//! to build a picture of where each usage lives in the report.
//!
//! ## Device Types
//!
//! - **Keyboard**: HID usage Generic Desktop → Keyboard (0x01 → 0x06)
//!   Report: 8 modifiers + 1 reserved + 6 key codes = 8 bytes (typical)
//! - **Mouse**: HID usage Generic Desktop → Mouse (0x01 → 0x02)
//!   Report: 3 buttons + 5 pad + 1..4 bytes X + 1..4 bytes Y = 4..10 bytes

use crate::ffi;
use crate::registers::{
    self, usb_class, usb_descriptor, usb_xfer_type,
    InterfaceDescriptor, EndpointDescriptor,
    build_setup_packet,
    hid_item, hid_usage_page, hid_generic_desktop, hid_keyboard, hid_button,
};
use crate::ring::RingMem;
use crate::xhci::XhciController;
use crate::usb_device::{self, UsbDeviceInfo, UsbDeviceType, ProbeResult, UsbClassDriver};

// ============================================================================
// Constants
// ============================================================================

/// Maximum number of HID devices we can track simultaneously.
pub const MAX_HID_DEVICES: usize = 8;

/// Maximum size of a HID input report (bytes).
pub const MAX_REPORT_SIZE: usize = 64;

/// Maximum number of parsed fields per device.
pub const MAX_FIELDS: usize = 32;

/// Maximum report descriptor size to parse.
pub const MAX_REPORT_DESC_SIZE: usize = 512;

/// Maximum number of simultaneous key presses tracked.
pub const MAX_KEYS: usize = 6;

// ============================================================================
// HID Report Descriptor Parsing
// ============================================================================

/// Types of HID devices we care about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HidDeviceKind {
    Keyboard,
    Mouse,
    Other,
}

/// Describes a single field in a HID input report.
/// A field represents a set of consecutive bits with the same usage,
/// same report size, and same attributes.
#[derive(Clone, Copy, Debug)]
pub struct HidField {
    /// Byte offset from start of report (bit-aligned address / 8).
    pub byte_offset: u8,
    /// Bit offset within the start byte (0-7).
    pub bit_offset: u8,
    /// Number of bits per individual item in this field.
    pub bit_size: u8,
    /// Number of items in this field (report count).
    pub report_count: u8,
    /// Usage page (e.g., 0x01 Generic Desktop).
    pub usage_page: u16,
    /// Minimum usage value (for a range of usages).
    pub usage_min: u16,
    /// Maximum usage value (for a range of usages).
    pub usage_max: u16,
    /// Whether this field is Data (false) or Constant (true).
    pub is_constant: bool,
    /// Whether this field is Variable (true) or Array (false).
    pub is_variable: bool,
    /// Whether this field is Relative (true) or Absolute (false).
    pub is_relative: bool,
}

/// Parsed report layout for a single Report ID.
#[derive(Clone, Debug)]
pub struct HidReportLayout {
    /// Report ID (0 if none/not used).
    pub report_id: u8,
    /// Total size of this report in bytes.
    pub report_size_bytes: u8,
    /// Parsed input fields.
    pub fields: [HidField; MAX_FIELDS],
    /// Number of valid fields.
    pub num_fields: u8,
}

impl HidReportLayout {
    fn new() -> Self {
        Self {
            report_id: 0,
            report_size_bytes: 0,
            fields: [HidField {
                byte_offset: 0, bit_offset: 0, bit_size: 0, report_count: 0,
                usage_page: 0, usage_min: 0, usage_max: 0,
                is_constant: false, is_variable: false, is_relative: false,
            }; MAX_FIELDS],
            num_fields: 0,
        }
    }

    pub const fn new_static() -> Self {
        const FIELD: HidField = HidField::new_static();
        Self {
            report_id: 0,
            report_size_bytes: 0,
            fields: [FIELD; MAX_FIELDS],
            num_fields: 0,
        }
    }

    fn add_field(&mut self, field: HidField) {
        if (self.num_fields as usize) < MAX_FIELDS {
            self.fields[self.num_fields as usize] = field;
            self.num_fields += 1;
        }
    }
}

/// State for parsing HID report descriptors.
struct HidParserState {
    /// Current usage page.
    usage_page: u16,
    /// Current usages from Usage items (up to 2).
    usages: [u16; 2],
    num_usages: u8,
    /// Current usage minimum/maximum.
    usage_min: u16,
    usage_max: u16,
    has_usage_minmax: bool,
    /// Current report size (bits per field).
    report_size: u8,
    /// Current report count (number of fields).
    report_count: u8,
    /// Current report ID (0 if none).
    report_id: u8,
    /// Current bit offset in the report being built.
    bit_offset: u16,
    /// Whether we are inside a keyboard or mouse application collection.
    in_keyboard: bool,
    in_mouse: bool,
    /// Parsed layouts built so far.
    layout: HidReportLayout,
}

impl HidParserState {
    fn new() -> Self {
        Self {
            usage_page: 0,
            usages: [0; 2],
            num_usages: 0,
            usage_min: 0, usage_max: 0, has_usage_minmax: false,
            report_size: 0, report_count: 0, report_id: 0,
            bit_offset: 0,
            in_keyboard: false, in_mouse: false,
            layout: HidReportLayout::new(),
        }
    }

    fn reset_local(&mut self) {
        self.usages = [0; 2];
        self.num_usages = 0;
        self.usage_min = 0;
        self.usage_max = 0;
        self.has_usage_minmax = false;
    }
}

/// Parse a HID short item byte (first byte of any short item).
fn parse_item_prefix(byte: u8) -> (u8, u8, u8) {
    let tag = (byte & hid_item::TAG_MASK) >> hid_item::TAG_SHIFT;
    let typ = (byte & hid_item::TYPE_MASK) >> hid_item::TYPE_SHIFT;
    let size = byte & hid_item::SIZE_MASK;
    (tag, typ, size)
}

/// Read item data as u32 from the descriptor bytes.
fn read_item_data(data: &[u8], size_code: u8) -> (u32, usize) {
    match size_code {
        0 => (0, 0),   // No data
        1 => (data[0] as u32, 1),
        2 => (u16::from_le_bytes([data[0], data[1]]) as u32, 2),
        3 => (u32::from_le_bytes([data[0], data[1], data[2], data[3]]), 4),
        _ => (0, 0),
    }
}

/// Parse a HID report descriptor and extract the report layout.
/// Scans for Input items and tracks their usage, size, and position.
/// Only processes items inside Application collections for Keyboard or Mouse.
fn parse_report_descriptor(desc: &[u8]) -> HidReportLayout {
    let mut state = HidParserState::new();
    let mut collection_stack: [u8; 4] = [0; 4];
    let mut collection_depth: usize = 0;

    let mut i = 0;
    while i < desc.len() {
        let (tag, typ, size_code) = parse_item_prefix(desc[i]);
        let data_start = i + 1;
        let (data, consumed) = if data_start < desc.len() {
            let max_avail = desc.len() - data_start;
            let actual_size = match size_code {
                0 => 0,
                1 => core::cmp::min(1, max_avail),
                2 => core::cmp::min(2, max_avail),
                3 => core::cmp::min(4, max_avail),
                _ => 0,
            };
            if actual_size > 0 {
                read_item_data(&desc[data_start..], actual_size as u8)
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        };
        let item_len = 1 + consumed;

        match typ {
            // Global items
            hid_item::TYPE_GLOBAL => {
                match tag {
                    hid_item::TAG_USAGE_PAGE => {
                        state.usage_page = data as u16;
                    }
                    hid_item::TAG_REPORT_SIZE => {
                        state.report_size = data as u8;
                    }
                    hid_item::TAG_REPORT_COUNT => {
                        state.report_count = data as u8;
                    }
                    hid_item::TAG_REPORT_ID => {
                        state.report_id = data as u8;
                        state.bit_offset = 0;
                        state.layout.report_id = data as u8;
                    }
                    hid_item::TAG_LOGICAL_MIN | hid_item::TAG_LOGICAL_MAX |
                    hid_item::TAG_PHYSICAL_MIN | hid_item::TAG_PHYSICAL_MAX |
                    hid_item::TAG_UNIT | hid_item::TAG_UNIT_EXPONENT => {
                        // Tracked but not used for our purposes
                    }
                    hid_item::TAG_PUSH => {
                        // Stack push — not implemented in this simple parser
                    }
                    hid_item::TAG_POP => {
                        // Stack pop — not implemented
                    }
                    _ => {}
                }
            }
            // Local items
            hid_item::TYPE_LOCAL => {
                match tag {
                    hid_item::TAG_USAGE => {
                        if (state.num_usages as usize) < state.usages.len() {
                            state.usages[state.num_usages as usize] = data as u16;
                            state.num_usages += 1;
                        }
                    }
                    hid_item::TAG_USAGE_MIN => {
                        state.usage_min = data as u16;
                        state.has_usage_minmax = true;
                    }
                    hid_item::TAG_USAGE_MAX => {
                        state.usage_max = data as u16;
                    }
                    _ => {}
                }
            }
            // Main items
            hid_item::TYPE_MAIN => {
                match tag {
                    hid_item::TAG_COLLECTION => {
                        let col_type = data as u8;
                        if collection_depth < collection_stack.len() {
                            collection_stack[collection_depth] = col_type;
                        }
                        collection_depth += 1;

                        // Check if entering a keyboard or mouse application collection
                        if col_type == hid_item::COL_APPLICATION {
                            // Use the tracked usages
                            for u_idx in 0..state.num_usages as usize {
                                let usage = state.usages[u_idx];
                                if state.usage_page == hid_usage_page::GENERIC_DESKTOP {
                                    if usage == hid_generic_desktop::KEYBOARD {
                                        state.in_keyboard = true;
                                        state.bit_offset = 0;
                                    } else if usage == hid_generic_desktop::MOUSE {
                                        state.in_mouse = true;
                                        state.bit_offset = 0;
                                    }
                                }
                            }
                        }
                    }
                    hid_item::TAG_END_COLLECTION => {
                        if collection_depth > 0 {
                            collection_depth -= 1;
                            // Check if exiting keyboard/mouse application
                            if collection_depth == 0 || collection_stack[collection_depth - 1] != hid_item::COL_APPLICATION {
                                // We've exited the application collection
                                // Reset for potential next application
                            }
                        }
                        // After closing collection, check if we were in keyboard/mouse
                        if collection_depth == 0 {
                            state.in_keyboard = false;
                            state.in_mouse = false;
                        }
                    }
                    hid_item::TAG_INPUT => {
                        let flags = data as u16;
                        let is_constant = (flags & hid_item::input::DATA_CONST as u16) != 0;
                        let is_variable = (flags & hid_item::input::VAR_ARRAY as u16) != 0;
                        let is_relative = (flags & hid_item::input::ABS_REL as u16) != 0;

                        // Calculate byte/bit offset
                        let bit_size = state.report_size as u16 * state.report_count as u16;
                        let byte_offset = (state.bit_offset / 8) as u8;
                        let bit_offset_in_byte = (state.bit_offset % 8) as u8;

                        // Set up usage range
                        let (usage_min, usage_max) = if state.has_usage_minmax {
                            (state.usage_min, state.usage_max)
                        } else if state.num_usages > 0 {
                            (state.usages[0], state.usages[if state.num_usages > 1 { 1 } else { 0 }])
                        } else {
                            (0, 0)
                        };

                        let field = HidField {
                            byte_offset,
                            bit_offset: bit_offset_in_byte,
                            bit_size: state.report_size,
                            report_count: state.report_count,
                            usage_page: state.usage_page,
                            usage_min,
                            usage_max,
                            is_constant,
                            is_variable,
                            is_relative,
                        };

                        state.layout.add_field(field);

                        // Update bit offset for next field
                        state.bit_offset += bit_size;

                        // Reset local items
                        state.reset_local();
                    }
                    hid_item::TAG_OUTPUT | hid_item::TAG_FEATURE => {
                        // Skip output and feature items — we only care about input
                        let bit_size = state.report_size as u16 * state.report_count as u16;
                        state.bit_offset += bit_size;
                        state.reset_local();
                    }
                    _ => {}
                }
            }
            _ => {}
        }

        if item_len == 0 { break; }
        i += item_len;
    }

    state.layout.report_size_bytes = ((state.bit_offset + 7) / 8) as u8;
    state.layout
}

// ============================================================================
// Keyboard State
// ============================================================================

/// Represents the current state of a USB keyboard.
#[derive(Clone, Copy, Debug)]
pub struct KeyboardState {
    /// Modifier key bitmask (bit 0=LCtrl, 1=LShift, 2=LAlt, 3=LGui,
    ///                         bit 4=RCtrl, 5=RShift, 6=RAlt, 7=RGui).
    pub modifiers: u8,
    /// Currently pressed key codes (up to 6 simultaneous).
    pub keys: [u8; MAX_KEYS],
    /// Number of currently pressed keys.
    pub key_count: u8,
}

impl KeyboardState {
    fn new() -> Self {
        Self { modifiers: 0, keys: [0; MAX_KEYS], key_count: 0 }
    }

    pub const fn new_static() -> Self {
        Self { modifiers: 0, keys: [0; MAX_KEYS], key_count: 0 }
    }

    /// Check if a specific key is currently pressed.
    pub fn is_key_pressed(&self, key_code: u8) -> bool {
        for i in 0..self.key_count as usize {
            if self.keys[i] == key_code {
                return true;
            }
        }
        false
    }

    /// Check if a modifier is active.
    pub fn is_modifier_active(&self, modifier_bit: u8) -> bool {
        (self.modifiers & (1u8 << modifier_bit)) != 0
    }

    /// Convert HID usage code to ASCII character (US layout, no shift).
    pub fn keycode_to_ascii(key_code: u8, shift: bool) -> Option<u8> {
        match key_code {
            hid_keyboard::A => Some(if shift { b'A' } else { b'a' }),
            hid_keyboard::B => Some(if shift { b'B' } else { b'b' }),
            hid_keyboard::C => Some(if shift { b'C' } else { b'c' }),
            hid_keyboard::D => Some(if shift { b'D' } else { b'd' }),
            hid_keyboard::E => Some(if shift { b'E' } else { b'e' }),
            hid_keyboard::F => Some(if shift { b'F' } else { b'f' }),
            hid_keyboard::G => Some(if shift { b'G' } else { b'g' }),
            hid_keyboard::H => Some(if shift { b'H' } else { b'h' }),
            hid_keyboard::I => Some(if shift { b'I' } else { b'i' }),
            hid_keyboard::J => Some(if shift { b'J' } else { b'j' }),
            hid_keyboard::K => Some(if shift { b'K' } else { b'k' }),
            hid_keyboard::L => Some(if shift { b'L' } else { b'l' }),
            hid_keyboard::M => Some(if shift { b'M' } else { b'm' }),
            hid_keyboard::N => Some(if shift { b'N' } else { b'n' }),
            hid_keyboard::O => Some(if shift { b'O' } else { b'o' }),
            hid_keyboard::P => Some(if shift { b'P' } else { b'p' }),
            hid_keyboard::Q => Some(if shift { b'Q' } else { b'q' }),
            hid_keyboard::R => Some(if shift { b'R' } else { b'r' }),
            hid_keyboard::S => Some(if shift { b'S' } else { b's' }),
            hid_keyboard::T => Some(if shift { b'T' } else { b't' }),
            hid_keyboard::U => Some(if shift { b'U' } else { b'u' }),
            hid_keyboard::V => Some(if shift { b'V' } else { b'v' }),
            hid_keyboard::W => Some(if shift { b'W' } else { b'w' }),
            hid_keyboard::X => Some(if shift { b'X' } else { b'x' }),
            hid_keyboard::Y => Some(if shift { b'Y' } else { b'y' }),
            hid_keyboard::Z => Some(if shift { b'Z' } else { b'z' }),
            hid_keyboard::_1 => Some(if shift { b'!' } else { b'1' }),
            hid_keyboard::_2 => Some(if shift { b'@' } else { b'2' }),
            hid_keyboard::_3 => Some(if shift { b'#' } else { b'3' }),
            hid_keyboard::_4 => Some(if shift { b'$' } else { b'4' }),
            hid_keyboard::_5 => Some(if shift { b'%' } else { b'5' }),
            hid_keyboard::_6 => Some(if shift { b'^' } else { b'6' }),
            hid_keyboard::_7 => Some(if shift { b'&' } else { b'7' }),
            hid_keyboard::_8 => Some(if shift { b'*' } else { b'8' }),
            hid_keyboard::_9 => Some(if shift { b'(' } else { b'9' }),
            hid_keyboard::_0 => Some(if shift { b')' } else { b'0' }),
            hid_keyboard::ENTER => Some(b'\n'),
            hid_keyboard::ESCAPE => Some(0x1B),
            hid_keyboard::BACKSPACE => Some(0x08),
            hid_keyboard::TAB => Some(b'\t'),
            hid_keyboard::SPACEBAR => Some(b' '),
            hid_keyboard::MINUS => Some(if shift { b'_' } else { b'-' }),
            hid_keyboard::EQUAL => Some(if shift { b'+' } else { b'=' }),
            hid_keyboard::LEFT_BRACKET => Some(if shift { b'{' } else { b'[' }),
            hid_keyboard::RIGHT_BRACKET => Some(if shift { b'}' } else { b']' }),
            hid_keyboard::BACKSLASH => Some(if shift { b'|' } else { b'\\' }),
            hid_keyboard::SEMICOLON => Some(if shift { b':' } else { b';' }),
            hid_keyboard::QUOTE => Some(if shift { b'\"' } else { b'\'' }),
            hid_keyboard::GRAVE => Some(if shift { b'~' } else { b'`' }),
            hid_keyboard::COMMA => Some(if shift { b'<' } else { b',' }),
            hid_keyboard::DOT => Some(if shift { b'>' } else { b'.' }),
            hid_keyboard::SLASH => Some(if shift { b'?' } else { b'/' }),
            hid_keyboard::CAPS_LOCK |
            hid_keyboard::F1..=hid_keyboard::F24 |
            hid_keyboard::NUM_LOCK |
            hid_keyboard::SCROLL_LOCK |
            hid_keyboard::LEFT_CTRL | hid_keyboard::LEFT_SHIFT |
            hid_keyboard::LEFT_ALT | hid_keyboard::LEFT_GUI |
            hid_keyboard::RIGHT_CTRL | hid_keyboard::RIGHT_SHIFT |
            hid_keyboard::RIGHT_ALT | hid_keyboard::RIGHT_GUI |
            hid_keyboard::INSERT | hid_keyboard::HOME | hid_keyboard::PAGE_UP |
            hid_keyboard::DELETE | hid_keyboard::END | hid_keyboard::PAGE_DOWN |
            hid_keyboard::RIGHT_ARROW | hid_keyboard::LEFT_ARROW |
            hid_keyboard::DOWN_ARROW | hid_keyboard::UP_ARROW |
            hid_keyboard::PRINT_SCREEN | hid_keyboard::PAUSE |
            hid_keyboard::APPLICATION | hid_keyboard::POWER |
            hid_keyboard::KEYPAD_DIVIDE | hid_keyboard::KEYPAD_MULTIPLY |
            hid_keyboard::KEYPAD_SUBTRACT | hid_keyboard::KEYPAD_ADD |
            hid_keyboard::KEYPAD_ENTER | hid_keyboard::KEYPAD_DOT |
            hid_keyboard::KEYPAD_SLASH => None, // Non-printable
            _ => None,
        }
    }
}

// ============================================================================
// Mouse State
// ============================================================================

/// Represents the current state of a USB mouse.
#[derive(Clone, Copy, Debug)]
pub struct MouseState {
    /// Button bitmask (bit 0=Left, 1=Right, 2=Middle).
    pub buttons: u8,
    /// X-axis movement (relative, signed).
    pub x: i16,
    /// Y-axis movement (relative, signed).
    pub y: i16,
    /// Wheel movement (relative, signed, positive = scroll down).
    pub wheel: i8,
    /// Whether there was movement since last read.
    pub has_moved: bool,
    /// Whether buttons changed since last read.
    pub buttons_changed: bool,
}

impl MouseState {
    fn new() -> Self {
        Self { buttons: 0, x: 0, y: 0, wheel: 0, has_moved: false, buttons_changed: false }
    }

    pub const fn new_static() -> Self {
        Self { buttons: 0, x: 0, y: 0, wheel: 0, has_moved: false, buttons_changed: false }
    }
}

// ============================================================================
// HID Device State
// ============================================================================

/// State for a single HID device (keyboard or mouse).
pub struct HidDevice {
    /// xHCI device slot ID.
    pub slot_id: u8,
    /// Interrupt IN endpoint number.
    pub ep_in: u8,
    /// Max packet size of interrupt endpoint.
    pub mps: u16,
    /// Device kind (Keyboard or Mouse).
    pub kind: HidDeviceKind,
    /// DMA buffer for interrupt data.
    pub intr_buf: Option<RingMem>,
    /// Parsed report layout.
    pub layout: HidReportLayout,
    /// Current keyboard state (valid if kind == Keyboard).
    pub keyboard: KeyboardState,
    /// Previous keyboard state (for detecting changes).
    pub prev_keyboard: KeyboardState,
    /// Current mouse state (valid if kind == Mouse).
    pub mouse: MouseState,
}

impl HidDevice {
    fn new() -> Self {
        Self {
            slot_id: 0, ep_in: 0, mps: 0,
            kind: HidDeviceKind::Other,
            intr_buf: None,
            layout: HidReportLayout::new(),
            keyboard: KeyboardState::new(),
            prev_keyboard: KeyboardState::new(),
            mouse: MouseState::new(),
        }
    }

    /// Create a static instance (for `pub static mut`).
    pub const fn new_static() -> Self {
        Self {
            slot_id: 0, ep_in: 0, mps: 0,
            kind: HidDeviceKind::Other,
            intr_buf: None,
            layout: HidReportLayout::new_static(),
            keyboard: KeyboardState::new_static(),
            prev_keyboard: KeyboardState::new_static(),
            mouse: MouseState::new_static(),
        }
    }
}

// ============================================================================
// Report Parser (extract data from raw HID report bytes)
// ============================================================================

/// Parse the raw HID report bytes using the parsed layout.
/// Fills in keyboard or mouse state accordingly.
fn parse_hid_report(dev: &mut HidDevice, report_data: &[u8]) {
    match dev.kind {
        HidDeviceKind::Keyboard => parse_keyboard_report(dev, report_data),
        HidDeviceKind::Mouse => parse_mouse_report(dev, report_data),
        HidDeviceKind::Other => {}
    }
}

/// Parse a keyboard HID report using the layout fields.
fn parse_keyboard_report(dev: &mut HidDevice, data: &[u8]) {
    // Save previous state
    dev.prev_keyboard = dev.keyboard;

    // Reset current state
    let mut modifiers: u8 = 0;
    let mut keys = [0u8; MAX_KEYS];
    let mut key_count: u8 = 0;

    for i in 0..dev.layout.num_fields as usize {
        let field = &dev.layout.fields[i];
        if field.is_constant {
            continue;
        }

        // Extract the field value
        let byte_idx = field.byte_offset as usize;
        if byte_idx >= data.len() {
            continue;
        }

        let bit_count = field.bit_size as u16;

        if field.usage_page == hid_usage_page::KEYBOARD_KEYPAD as u16 &&
           field.usage_min >= 0xE0 && field.usage_max >= 0xE7 {
            // Modifier keys (8 bits, one per modifier)
            let val = data[byte_idx]; // The whole byte is the modifier mask
            modifiers = val;
        } else if field.usage_page == hid_usage_page::KEYBOARD_KEYPAD as u16 &&
                  !field.is_variable && field.is_constant == false {
            // Array-style key codes (typical for keyboard boot protocol)
            let byte_pos = field.byte_offset as usize;
            // Each item in the array is `bit_size` bits; total bytes = (bit_size * report_count + 7) / 8
            let field_bytes = ((field.bit_size as usize) * (field.report_count as usize) + 7) / 8;
            // Read key codes from the array
            for b in 0..field_bytes {
                if byte_pos + b < data.len() {
                    let kc = data[byte_pos + b];
                    if kc > 0 && kc != hid_keyboard::ERROR_ROLLOVER &&
                       kc != hid_keyboard::ERROR_UNDEFINED {
                        if (key_count as usize) < MAX_KEYS {
                            keys[key_count as usize] = kc;
                            key_count += 1;
                        }
                    }
                }
            }
        } else if field.is_variable {
            // Variable-style: each bit is a separate value
            // This handles individual modifier-like bit fields
            if bit_count == 1 {
                let byte_val = if byte_idx < data.len() { data[byte_idx] } else { 0 };
                let bit_val = (byte_val >> field.bit_offset) & 1;
                // Track as generic key presses based on usage
                if bit_val != 0 && field.usage_page == hid_usage_page::KEYBOARD_KEYPAD as u16 {
                    // This is likely a modifier key bit
                    // Map to modifier bit position
                    for usage_idx in field.usage_min..=field.usage_max {
                        if usage_idx >= 0xE0 && usage_idx <= 0xE7 {
                            let mod_bit = (usage_idx - 0xE0) as u8;
                            modifiers |= 1 << mod_bit;
                        }
                    }
                }
            }
        }
    }

    dev.keyboard = KeyboardState { modifiers, keys, key_count };
}

/// Parse a mouse HID report using the layout fields.
fn parse_mouse_report(dev: &mut HidDevice, data: &[u8]) {
    let prev_buttons = dev.mouse.buttons;
    let mut buttons: u8 = 0;
    let mut x: i16 = 0;
    let mut y: i16 = 0;
    let mut wheel: i8 = 0;
    let mut has_moved = false;

    for i in 0..dev.layout.num_fields as usize {
        let field = &dev.layout.fields[i];
        if field.is_constant {
            continue;
        }

        let byte_idx = field.byte_offset as usize;
        if byte_idx >= data.len() {
            continue;
        }

        if field.usage_page == hid_usage_page::BUTTON as u16 {
            // Button field
            if field.is_variable && field.bit_size == 1 {
                // Each bit is a button
                let byte_val = data[byte_idx];
                for b in 0..core::cmp::min(field.usage_max - field.usage_min + 1, 8) as u8 {
                    let bit_val = (byte_val >> (field.bit_offset + b)) & 1;
                    if bit_val != 0 {
                        buttons |= 1 << b;
                    }
                }
            }
        } else if field.usage_page == hid_usage_page::GENERIC_DESKTOP as u16 {
            // X, Y, Wheel axes
            if (field.usage_min <= hid_generic_desktop::X &&
                 field.usage_max >= hid_generic_desktop::X) ||
               (!field.has_usage_minmax() && field.byte_offset == 1) // fallback
            {
                // Extract X value
                let signed_val = extract_signed_value(data, byte_idx,
                    field.bit_offset as u16, field.bit_size as u16);
                if field.is_relative {
                    x = signed_val;
                    if signed_val != 0 { has_moved = true; }
                }
            }
            if (field.usage_min <= hid_generic_desktop::Y &&
                 field.usage_max >= hid_generic_desktop::Y) ||
               (!field.has_usage_minmax() && field.byte_offset == 2) // fallback
            {
                let signed_val = extract_signed_value(data, byte_idx,
                    field.bit_offset as u16, field.bit_size as u16);
                if field.is_relative {
                    y = signed_val;
                    if signed_val != 0 { has_moved = true; }
                }
            }
            if (field.usage_min <= hid_generic_desktop::WHEEL &&
                 field.usage_max >= hid_generic_desktop::WHEEL) ||
               field.usage_min == hid_generic_desktop::WHEEL ||
               field.usage_max == hid_generic_desktop::WHEEL
            {
                let signed_val = extract_signed_value(data, byte_idx,
                    field.bit_offset as u16, field.bit_size as u16);
                wheel = signed_val as i8;
                if signed_val != 0 { has_moved = true; }
            }
        }
    }

    let buttons_changed = buttons != prev_buttons;
    dev.mouse = MouseState { buttons, x, y, wheel, has_moved, buttons_changed };
}

/// Extract a signed value from data at the given byte offset and bit position.
fn extract_signed_value(data: &[u8], byte_offset: usize, bit_offset: u16, bit_size: u16) -> i16 {
    if bit_size == 0 || byte_offset >= data.len() {
        return 0;
    }

    let bits_available = (data.len() - byte_offset) as u16 * 8 - bit_offset;
    let actual_bits = core::cmp::min(bit_size, bits_available);

    if actual_bits <= 8 {
        let mut raw = data[byte_offset] as u16;
        if actual_bits < 8 {
            raw = (raw >> bit_offset) & ((1u16 << actual_bits) - 1);
            // Sign-extend
            if (raw & (1u16 << (actual_bits - 1))) != 0 {
                raw |= !((1u16 << actual_bits) - 1);
            }
        }
        raw as i8 as i16
    } else if actual_bits <= 16 {
        let byte_count = ((actual_bits + 7) / 8) as usize;
        let mut raw: u16 = 0;
        for b in 0..core::cmp::min(byte_count, data.len() - byte_offset) {
            raw |= (data[byte_offset + b] as u16) << (b * 8);
        }
        if actual_bits < 16 {
            raw = (raw >> bit_offset) & ((1u16 << actual_bits) - 1);
            if (raw & (1u16 << (actual_bits - 1))) != 0 {
                raw |= !((1u16 << actual_bits) - 1);
            }
        }
        raw as i16
    } else {
        0
    }
}

// Helper trait for checking usage_minmax
impl HidField {
    fn has_usage_minmax(&self) -> bool {
        self.usage_min != 0 || self.usage_max != 0
    }

    pub const fn new_static() -> Self {
        Self {
            byte_offset: 0, bit_offset: 0, bit_size: 0, report_count: 0,
            usage_page: 0, usage_min: 0, usage_max: 0,
            is_constant: false, is_variable: false, is_relative: false,
        }
    }
}

// ============================================================================
// HID Class Driver
// ============================================================================

/// The USB HID class driver instance.
pub struct HidDriver {
    /// Tracked HID devices.
    pub devices: [HidDevice; MAX_HID_DEVICES],
    /// Number of active HID devices.
    pub num_devices: usize,
    /// Verbose logging.
    pub verbose: u8,
}

impl HidDriver {
    pub fn new(verbose: u8) -> Self {
        Self {
            devices: core::array::from_fn(|_| HidDevice::new()),
            num_devices: 0,
            verbose,
        }
    }

    /// Create a static instance (for `pub static mut`).
    pub const fn new_static() -> Self {
        const DEV: HidDevice = HidDevice::new_static();
        Self {
            devices: [DEV, DEV, DEV, DEV, DEV, DEV, DEV, DEV],
            num_devices: 0,
            verbose: 0,
        }
    }

    fn find_by_slot(&self, slot_id: u8) -> Option<&HidDevice> {
        self.devices[..self.num_devices].iter().find(|d| d.slot_id == slot_id)
    }

    fn find_by_slot_mut(&mut self, slot_id: u8) -> Option<&mut HidDevice> {
        self.devices[..self.num_devices].iter_mut().find(|d| d.slot_id == slot_id)
    }

    fn remove_device(&mut self, slot_id: u8) {
        let mut found = None;
        for i in 0..self.num_devices {
            if self.devices[i].slot_id == slot_id {
                found = Some(i);
                break;
            }
        }
        if let Some(idx) = found {
            // Free DMA buffer before removing
            if let Some(b) = &mut self.devices[idx].intr_buf {
                b.free();
            }
            for j in idx..self.num_devices - 1 {
                let tmp = core::mem::replace(&mut self.devices[j + 1], HidDevice::new());
                self.devices[j] = tmp;
            }
            self.num_devices -= 1;
        }
    }

    /// Fetch the HID report descriptor from the device via control transfer.
    /// The HID descriptor (type 0x21) is read first to get the report descriptor length,
    /// then the report descriptor (type 0x22) is fetched.
    fn fetch_report_descriptor(xhc: &mut XhciController, slot_id: u8,
        buf_virt: *mut u8, buf_phys: u64, buf_len: usize
    ) -> Option<usize> {
        if buf_len < 10 { return None; }

        // Step 1: Read the HID descriptor (type 0x21) to get report descriptor length
        // bmRequestType = 0x81 (D2H, Standard, Interface), wIndex = 0 (first interface)
        let pkt = build_setup_packet(
            0x81, // D2H, Standard, Interface
            crate::registers::usb_req::GET_DESCRIPTOR,
            (crate::registers::USB_HID_DT_HID as u16) << 8,
            0, // interface number — use 0 for first interface
            9, // HID descriptor is 9 bytes
        );
        if !xhc.control_transfer(slot_id, &pkt, true, buf_phys, 9) {
            if xhc.verbose >= 2 {
                ffi::print(b"xHCI: HID: failed to get HID descriptor\0");
            }
            return None;
        }

        // Parse HID descriptor to get report descriptor length
        // HID descriptor layout: bLength(1), bDescriptorType(1), bcdHID(2),
        // bCountryCode(1), bNumDescriptors(1), bDescriptorType(1), wDescriptorLength(2)
        let data = unsafe { core::slice::from_raw_parts(buf_virt as *const u8, 9) };
        if data.len() < 9 || data[0] < 9 {
            return None;
        }

        let num_subdesc = data[5] as usize;
        let mut report_desc_len: usize = 0;
        let mut offset = 6;
        for _ in 0..num_subdesc {
            if offset + 3 <= data.len() {
                let desc_type = data[offset];
                if desc_type == crate::registers::USB_HID_DT_REPORT {
                    report_desc_len = u16::from_le_bytes([data[offset + 1], data[offset + 2]]) as usize;
                }
                offset += 3;
            }
        }

        if report_desc_len == 0 || report_desc_len > MAX_REPORT_DESC_SIZE {
            // Fallback: try to read report descriptor with expected size
            if buf_len >= 64 {
                let pkt2 = build_setup_packet(
                    0x81, crate::registers::usb_req::GET_DESCRIPTOR,
                    (crate::registers::USB_HID_DT_REPORT as u16) << 8,
                    0, 64,
                );
                if xhc.control_transfer(slot_id, &pkt2, true, buf_phys, 64) {
                    return Some(64);
                }
            }
            return None;
        }

        let read_len = core::cmp::min(report_desc_len, buf_len);

        // Step 2: Read the report descriptor (type 0x22)
        let pkt2 = build_setup_packet(
            0x81, crate::registers::usb_req::GET_DESCRIPTOR,
            (crate::registers::USB_HID_DT_REPORT as u16) << 8,
            0, read_len as u16,
        );
        if xhc.control_transfer(slot_id, &pkt2, true, buf_phys, read_len as u32) {
            Some(read_len)
        } else {
            None
        }
    }

    /// Poll the HID device's interrupt endpoint for a new report.
    /// Returns true if a report was received.
    fn poll_interrupt(dev: &mut HidDevice, xhc: &mut XhciController) -> bool {
        if dev.ep_in == 0 {
            return false;
        }

        let buf: *const RingMem = match &dev.intr_buf {
            Some(b) => b,
            None => return false,
        };
        // Use raw pointer to avoid immutable borrow during queue_bulk_transfer
        let buf_phys = unsafe { (*buf).phys };

        let transfer_len = core::cmp::max(dev.mps as u32, 8);
        if !xhc.queue_bulk_transfer(dev.slot_id, dev.ep_in, true, buf_phys, transfer_len, true) {
            return false;
        }
        if !xhc.poll_transfer_event(100_000) { // 100ms timeout
            return false;
        }

        // Parse the report from the DMA buffer
        let buf_virt = unsafe { (*buf).virt };
        let report_data = unsafe { core::slice::from_raw_parts(buf_virt as *const u8, transfer_len as usize) };
        parse_hid_report(dev, report_data);
        true
    }

    /// Read the keyboard state. Returns Some if a keyboard report was received.
    /// `clear_changes` — if true, resets change tracking after read.
    pub fn read_keyboard(&mut self, xhc: &mut XhciController, slot_id: u8) -> Option<&KeyboardState> {
        if let Some(dev) = self.find_by_slot_mut(slot_id) {
            if dev.kind != HidDeviceKind::Keyboard { return None; }
            Self::poll_interrupt(dev, xhc);
            Some(&dev.keyboard)
        } else {
            None
        }
    }

    /// Read the mouse state. Returns Some if a mouse report was received.
    pub fn read_mouse(&mut self, xhc: &mut XhciController, slot_id: u8) -> Option<&MouseState> {
        if let Some(dev) = self.find_by_slot_mut(slot_id) {
            if dev.kind != HidDeviceKind::Mouse { return None; }
            Self::poll_interrupt(dev, xhc);
            Some(&dev.mouse)
        } else {
            None
        }
    }
}

impl UsbClassDriver for HidDriver {
    fn class_code(&self) -> u8 { usb_class::HID }
    fn subclass_code(&self) -> u8 { 0 }
    fn protocol_code(&self) -> u8 { 0 }

    fn name(&self) -> &'static [u8] { b"USB HID\0" }

    fn probe(&mut self, xhc: &mut XhciController, slot_id: u8, _dev_info: &UsbDeviceInfo) -> ProbeResult {
        if self.num_devices >= MAX_HID_DEVICES {
            return ProbeResult::Failed;
        }

        if self.verbose >= 1 {
            ffi::print(b"xHCI: probing HID device\0");
        }

        // Step 1: Setup EP0 transfer ring
        if !xhc.setup_ep0_transfer_ring(slot_id) {
            return ProbeResult::Failed;
        }

        // Step 2: Read config descriptor to find HID interface + interrupt endpoint
        let mut cfg_buf = match RingMem::alloc(256) {
            Some(b) => b,
            None => return ProbeResult::Failed,
        };

        if !xhc.get_config_descriptor(slot_id, 0, cfg_buf.virt, cfg_buf.phys, cfg_buf.size) {
            cfg_buf.free();
            return ProbeResult::Failed;
        }

        let cfg_data = unsafe { core::slice::from_raw_parts(cfg_buf.virt as *const u8, cfg_buf.size) };

        // Scan for HID interface + interrupt IN endpoint
        let mut found = false;
        let mut device_kind = HidDeviceKind::Other;
        let mut ep_in = 0u8;
        let mut mps = 0u16;
        let mut iface_number = 0u8;
        let mut protocol = 0u8; // 0=Boot, 1=Report

        let mut i = 0;
        let mut in_hid_iface = false;
        while i + 1 < cfg_data.len() {
            let len = cfg_data[i] as usize;
            let desc_type = cfg_data[i + 1];
            if len < 2 { break; }

            match desc_type {
                t if t == usb_descriptor::INTERFACE => {
                    in_hid_iface = false;
                    if i + 9 <= cfg_data.len() {
                        let iface = match InterfaceDescriptor::parse(&cfg_data[i..]) {
                            Some(d) => d,
                            None => break,
                        };
                        if iface.bInterfaceClass == usb_class::HID {
                            in_hid_iface = true;
                            iface_number = iface.bInterfaceNumber;
                            protocol = iface.bInterfaceProtocol;
                            // Determine device kind from protocol:
                            // 1 = Keyboard (Boot Protocol), 2 = Mouse (Boot Protocol)
                            match iface.bInterfaceProtocol {
                                1 => device_kind = HidDeviceKind::Keyboard,
                                2 => device_kind = HidDeviceKind::Mouse,
                                _ => device_kind = HidDeviceKind::Other,
                            }
                            if self.verbose >= 1 {
                                ffi::print(b"xHCI: HID interface found\0");
                            }
                        }
                    }
                }
                t if t == usb_descriptor::ENDPOINT => {
                    if in_hid_iface && i + 7 <= cfg_data.len() {
                        let ep = match EndpointDescriptor::parse(&cfg_data[i..]) {
                            Some(e) => e,
                            None => break,
                        };
                        if ep.transfer_type() == usb_xfer_type::INTERRUPT && ep.is_in() {
                            ep_in = ep.endpoint_number();
                            mps = ep.max_packet_size();
                            found = true;
                            if self.verbose >= 1 {
                                ffi::print(b"xHCI: HID interrupt IN endpoint found\0");
                            }
                        }
                    }
                }
                _ => {}
            }

            if len == 0 { break; }
            i += len;
        }

        if !found || ep_in == 0 {
            cfg_buf.free();
            if self.verbose >= 1 {
                ffi::print(b"xHCI: HID no interrupt endpoint\0");
            }
            return ProbeResult::Claimed; // Claim but won't work without interrupt
        }

        // For keyboard boot protocol (subclass 1) and mouse boot protocol (subclass 2),
        // the report format is standardized: no need to parse the report descriptor.
        // Set kind based on protocol rather than parsing.
        if protocol == 1 {
            device_kind = HidDeviceKind::Keyboard;
        } else if protocol == 2 {
            device_kind = HidDeviceKind::Mouse;
        } else {
            // For report protocol, we'd need to parse the report descriptor.
            // Fall back to protocol-based detection.
            device_kind = HidDeviceKind::Other;
        }

        // Step 3: Read report descriptor to determine report layout
        let mut report_buf = match RingMem::alloc(MAX_REPORT_DESC_SIZE) {
            Some(b) => b,
            None => { cfg_buf.free(); return ProbeResult::Failed; }
        };

        let desc_len = match Self::fetch_report_descriptor(xhc, slot_id,
            report_buf.virt, report_buf.phys, MAX_REPORT_DESC_SIZE)
        {
            Some(len) => len,
            None => {
                // If we can't get the report descriptor, use boot protocol defaults
                report_buf.free();
                cfg_buf.free();

                // Allocate interrupt buffer based on protocol defaults
                let intr_size = match device_kind {
                    HidDeviceKind::Keyboard => 8,  // Standard keyboard report
                    HidDeviceKind::Mouse => 8,      // Standard mouse report
                    HidDeviceKind::Other => mps as usize,
                };
                let mut intr_buf = match RingMem::alloc(core::cmp::max(64, intr_size.next_power_of_two())) {
                    Some(b) => b,
                    None => return ProbeResult::Failed,
                };

                // Create a default layout for boot protocol
                let mut layout = HidReportLayout::new();
                layout.report_size_bytes = intr_size as u8;

                // Set the interrupt endpoint in HW
                let dci = XhciController::ep_num_to_dci(ep_in, true);
                if !xhc.configure_endpoint(slot_id, dci, 7 /* Interrupt IN */,
                    mps, 3, mps) {
                    intr_buf.free();
                    return ProbeResult::Failed;
                }

                let dev_idx = self.num_devices;
                self.devices[dev_idx].slot_id = slot_id;
                self.devices[dev_idx].ep_in = ep_in;
                self.devices[dev_idx].mps = mps;
                self.devices[dev_idx].kind = device_kind;
                self.devices[dev_idx].intr_buf = Some(intr_buf);
                self.devices[dev_idx].layout = layout;
                self.devices[dev_idx].keyboard = KeyboardState::new();
                self.devices[dev_idx].prev_keyboard = KeyboardState::new();
                self.devices[dev_idx].mouse = MouseState::new();
                self.num_devices += 1;

                if self.verbose >= 1 {
                    ffi::print(b"xHCI: HID device ready (boot protocol)\0");
                }
                return ProbeResult::Claimed;
            }
        };

        // Step 3b: Parse the report descriptor
        let desc_data = unsafe { core::slice::from_raw_parts(report_buf.virt as *const u8, desc_len) };
        let layout = parse_report_descriptor(desc_data);

        report_buf.free();
        cfg_buf.free();

        // Determine device kind from layout if not already known from protocol
        if device_kind == HidDeviceKind::Other {
            for i in 0..layout.num_fields as usize {
                let f = &layout.fields[i];
                if f.usage_page == hid_usage_page::GENERIC_DESKTOP as u16 &&
                   (f.usage_min <= hid_generic_desktop::KEYBOARD &&
                    f.usage_max >= hid_generic_desktop::KEYBOARD) {
                    device_kind = HidDeviceKind::Keyboard;
                    break;
                }
            }
            if device_kind == HidDeviceKind::Other {
                for i in 0..layout.num_fields as usize {
                    let f = &layout.fields[i];
                    if f.usage_page == hid_usage_page::GENERIC_DESKTOP as u16 &&
                       (f.usage_min <= hid_generic_desktop::MOUSE &&
                        f.usage_max >= hid_generic_desktop::MOUSE) {
                        device_kind = HidDeviceKind::Mouse;
                        break;
                    }
                }
            }
        }

        // Step 4: Configure interrupt endpoint
        let dci = XhciController::ep_num_to_dci(ep_in, true);
        if !xhc.configure_endpoint(slot_id, dci, 7 /* Interrupt IN */,
            mps, 3, mps) {
            return ProbeResult::Failed;
        }

        // Step 5: Allocate interrupt buffer
        let report_size = core::cmp::max(layout.report_size_bytes as usize, mps as usize);
        let buf_size = report_size.next_power_of_two();
        let intr_buf = match RingMem::alloc(buf_size) {
            Some(b) => b,
            None => return ProbeResult::Failed,
        };

        let dev_idx = self.num_devices;
        self.devices[dev_idx].slot_id = slot_id;
        self.devices[dev_idx].ep_in = ep_in;
        self.devices[dev_idx].mps = mps;
        self.devices[dev_idx].kind = device_kind;
        self.devices[dev_idx].intr_buf = Some(intr_buf);
        self.devices[dev_idx].layout = layout;
        self.devices[dev_idx].keyboard = KeyboardState::new();
        self.devices[dev_idx].prev_keyboard = KeyboardState::new();
        self.devices[dev_idx].mouse = MouseState::new();
        self.num_devices += 1;

        if self.verbose >= 1 {
            ffi::print(b"xHCI: HID device ready\0");
        }
        ProbeResult::Claimed
    }

    fn disconnect(&mut self, xhc: &mut XhciController, slot_id: u8) {
        if let Some(dev) = self.find_by_slot_mut(slot_id) {
            if let Some(b) = &mut dev.intr_buf {
                b.free();
                dev.intr_buf = None;
            }
        }
        self.remove_device(slot_id);
        if self.verbose >= 1 {
            ffi::print(b"xHCI: HID device disconnected\0");
        }
    }
}
