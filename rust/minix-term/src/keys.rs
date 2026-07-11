// Keys — Key types and escape sequence parser for minix-term.
//
// Parses raw bytes from stdin into Key enum variants.
// Supports most common terminal input sequences (arrows, F-keys, navigation, etc.).

/// All supported key types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Regular ASCII character
    Char(char),

    // Control characters
    Tab,
    Enter,
    Esc,
    Backspace,
    Delete,

    // Arrow keys
    Up,
    Down,
    Left,
    Right,

    // Navigation keys
    Home,
    End,
    PageUp,
    PageDown,

    // Function keys
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,

    /// Ctrl+letter (e.g. Ctrl+C)
    Ctrl(char),

    // Shift+function keys
    ShiftF1, ShiftF2, ShiftF3, ShiftF4, ShiftF5, ShiftF6,
    ShiftF7, ShiftF8, ShiftF9, ShiftF10, ShiftF11, ShiftF12,

    /// Meta (Alt) + key combination
    Alt(char),

    /// Unrecognized byte sequence
    Unknown,
}

/// Parse a sequence of bytes into the first recognized Key.
///
/// First-match-wins. If the buffer contains multiple key events,
/// only the first is parsed (tail bytes are discarded).
pub fn parse_keys(buf: &[u8]) -> Key {
    if buf.is_empty() {
        return Key::Unknown;
    }

    let first = buf[0];

    // Single-byte matches
    match first {
        b'\x1b' => {
            if buf.len() == 1 {
                return Key::Esc;
            }
            parse_escape(&buf[1..])
        }
        b'\x7f' | b'\x08' => Key::Backspace,
        b'\x09' => Key::Tab,
        b'\x0a' | b'\x0d' => Key::Enter,
        // Ctrl+letter codes
        b'\x03' => Key::Ctrl('c'),
        b'\x04' => Key::Ctrl('d'),
        b'\x1a' => Key::Ctrl('z'),
        b'\x01' => Key::Ctrl('a'),
        b'\x02' => Key::Ctrl('b'),
        b'\x05' => Key::Ctrl('e'),
        b'\x06' => Key::Ctrl('f'),
        b'\x07' => Key::Ctrl('g'),
        b'\x0b' => Key::Ctrl('k'),
        b'\x0c' => Key::Ctrl('l'),
        b'\x0e' => Key::Ctrl('n'),
        b'\x0f' => Key::Ctrl('o'),
        b'\x10' => Key::Ctrl('p'),
        b'\x11' => Key::Ctrl('q'),
        b'\x12' => Key::Ctrl('r'),
        b'\x13' => Key::Ctrl('s'),
        b'\x14' => Key::Ctrl('t'),
        b'\x15' => Key::Ctrl('u'),
        b'\x16' => Key::Ctrl('v'),
        b'\x17' => Key::Ctrl('w'),
        b'\x18' => Key::Ctrl('x'),
        b'\x19' => Key::Ctrl('y'),
        _ if first >= 0x20 && first <= 0x7e => Key::Char(first as char),
        _ => Key::Unknown,
    }
}

/// Parse byte sequence after an ESC (0x1b).
fn parse_escape(buf: &[u8]) -> Key {
    if buf.is_empty() {
        return Key::Esc;
    }

    match buf[0] {
        b'[' => {
            // CSI (Control Sequence Introducer) — ESC [
            if buf.len() == 1 {
                return Key::Unknown;
            }

            let rest = &buf[1..];

            if rest.is_empty() {
                return Key::Unknown;
            }

            match rest[0] {
                // Simple CSI sequences
                b'A' => Key::Up,
                b'B' => Key::Down,
                b'C' => Key::Right,
                b'D' => Key::Left,
                b'H' => Key::Home,
                b'F' => Key::End,
                b'Z' => Key::Tab,       // Shift+Tab

                // CSI ~ sequences (ESC [ N ~)
                b'1' | b'2' | b'3' | b'4' | b'5' | b'6' | b'7' | b'8' => {
                    // Check for modified arrow: ESC [ 1 ; n A/B/C/D
                    if rest[0] == b'1' && rest.len() >= 4 && rest[1] == b';' {
                        parse_mod_arrow(rest)
                    } else {
                        parse_csi_tilde(rest)
                    }
                }

                _ => Key::Unknown,
            }
        }
        b'O' => {
            // SS3 (Single Shift Three) — ESC O
            if buf.len() < 2 {
                return Key::Unknown;
            }
            match buf[1] {
                b'P' => Key::F1,
                b'Q' => Key::F2,
                b'R' => Key::F3,
                b'S' => Key::F4,
                _ => Key::Unknown,
            }
        }
        _ => {
            // Alt+key: any other byte after ESC
            let ch = buf[0] as char;
            if ch.is_ascii() {
                Key::Alt(ch)
            } else {
                Key::Unknown
            }
        }
    }
}

/// Parse CSI ~ sequence: ESC [ N ~
/// e.g. ESC [ 3 ~ = Delete, ESC [ 5 ~ = PageUp, ESC [ 1 1 ~ = F1
fn parse_csi_tilde(buf: &[u8]) -> Key {
    let mut num = 0u32;
    let mut found_tilde = false;

    for &b in buf {
        if b.is_ascii_digit() {
            num = num * 10 + (b - b'0') as u32;
        } else if b == b'~' {
            found_tilde = true;
            break;
        } else {
            return Key::Unknown;
        }
    }

    if !found_tilde {
        return Key::Unknown;
    }

    match num {
        1 | 7 => Key::Home,
        2 => Key::Unknown,   // Insert (uncommon)
        3 => Key::Delete,
        4 | 8 => Key::End,
        5 => Key::PageUp,
        6 => Key::PageDown,
        // Function keys (CSI ~ variant)
        11 => Key::F1,  12 => Key::F2,  13 => Key::F3,  14 => Key::F4,
        15 => Key::F5,  17 => Key::F6,  18 => Key::F7,  19 => Key::F8,
        20 => Key::F9,  21 => Key::F10, 23 => Key::F11, 24 => Key::F12,
        // Shift+Function keys
        25 => Key::ShiftF1,  26 => Key::ShiftF2,  28 => Key::ShiftF3,
        29 => Key::ShiftF4,  31 => Key::ShiftF5,  32 => Key::ShiftF6,
        33 => Key::ShiftF7,  34 => Key::ShiftF8,  35 => Key::ShiftF9,
        36 => Key::ShiftF10, 37 => Key::ShiftF11, 38 => Key::ShiftF12,
        _ => Key::Unknown,
    }
}

/// Parse modified arrow keys: ESC [ 1 ; n X
/// where buf = "1;nX", X is A/B/C/D, n = modifier (2=Shift, 3=Alt, 5=Ctrl)
fn parse_mod_arrow(buf: &[u8]) -> Key {
    // buf starts AFTER ESC [, so buf = "1;nX"
    // We need "1;nX" — at least 4 bytes
    if buf.len() < 4 || buf[1] != b';' {
        return Key::Unknown;
    }

    let direction = buf[3];
    match direction {
        b'A' => Key::Up,
        b'B' => Key::Down,
        b'C' => Key::Right,
        b'D' => Key::Left,
        _ => Key::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_chars() {
        assert_eq!(parse_keys(b"a"), Key::Char('a'));
        assert_eq!(parse_keys(b"Z"), Key::Char('Z'));
        assert_eq!(parse_keys(b" "), Key::Char(' '));
        assert_eq!(parse_keys(b"5"), Key::Char('5'));
    }

    #[test]
    fn test_control_keys() {
        assert_eq!(parse_keys(b"\x03"), Key::Ctrl('c'));
        assert_eq!(parse_keys(b"\x1b"), Key::Esc);
        assert_eq!(parse_keys(b"\x09"), Key::Tab);
        assert_eq!(parse_keys(b"\x0a"), Key::Enter);
        assert_eq!(parse_keys(b"\x0d"), Key::Enter);
        assert_eq!(parse_keys(b"\x7f"), Key::Backspace);
    }

    #[test]
    fn test_arrow_keys() {
        assert_eq!(parse_keys(b"\x1b[A"), Key::Up);
        assert_eq!(parse_keys(b"\x1b[B"), Key::Down);
        assert_eq!(parse_keys(b"\x1b[C"), Key::Right);
        assert_eq!(parse_keys(b"\x1b[D"), Key::Left);
    }

    #[test]
    fn test_nav_keys() {
        assert_eq!(parse_keys(b"\x1b[H"), Key::Home);
        assert_eq!(parse_keys(b"\x1b[F"), Key::End);
        assert_eq!(parse_keys(b"\x1b[5~"), Key::PageUp);
        assert_eq!(parse_keys(b"\x1b[6~"), Key::PageDown);
        assert_eq!(parse_keys(b"\x1b[3~"), Key::Delete);
    }

    #[test]
    fn test_function_keys_ss3() {
        assert_eq!(parse_keys(b"\x1bOP"), Key::F1);
        assert_eq!(parse_keys(b"\x1bOQ"), Key::F2);
        assert_eq!(parse_keys(b"\x1bOR"), Key::F3);
        assert_eq!(parse_keys(b"\x1bOS"), Key::F4);
    }

    #[test]
    fn test_function_keys_csi() {
        assert_eq!(parse_keys(b"\x1b[11~"), Key::F1);
        assert_eq!(parse_keys(b"\x1b[12~"), Key::F2);
        assert_eq!(parse_keys(b"\x1b[13~"), Key::F3);
        assert_eq!(parse_keys(b"\x1b[14~"), Key::F4);
        assert_eq!(parse_keys(b"\x1b[15~"), Key::F5);
        assert_eq!(parse_keys(b"\x1b[17~"), Key::F6);
        assert_eq!(parse_keys(b"\x1b[18~"), Key::F7);
        assert_eq!(parse_keys(b"\x1b[19~"), Key::F8);
        assert_eq!(parse_keys(b"\x1b[20~"), Key::F9);
        assert_eq!(parse_keys(b"\x1b[21~"), Key::F10);
        assert_eq!(parse_keys(b"\x1b[23~"), Key::F11);
        assert_eq!(parse_keys(b"\x1b[24~"), Key::F12);
    }

    #[test]
    fn test_shift_function_keys() {
        assert_eq!(parse_keys(b"\x1b[25~"), Key::ShiftF1);
        assert_eq!(parse_keys(b"\x1b[26~"), Key::ShiftF2);
        assert_eq!(parse_keys(b"\x1b[28~"), Key::ShiftF3);
        assert_eq!(parse_keys(b"\x1b[29~"), Key::ShiftF4);
        assert_eq!(parse_keys(b"\x1b[31~"), Key::ShiftF5);
        assert_eq!(parse_keys(b"\x1b[32~"), Key::ShiftF6);
        assert_eq!(parse_keys(b"\x1b[33~"), Key::ShiftF7);
        assert_eq!(parse_keys(b"\x1b[34~"), Key::ShiftF8);
        assert_eq!(parse_keys(b"\x1b[35~"), Key::ShiftF9);
        assert_eq!(parse_keys(b"\x1b[36~"), Key::ShiftF10);
        assert_eq!(parse_keys(b"\x1b[37~"), Key::ShiftF11);
        assert_eq!(parse_keys(b"\x1b[38~"), Key::ShiftF12);
    }

    #[test]
    fn test_alt_keys() {
        assert_eq!(parse_keys(b"\x1ba"), Key::Alt('a'));
        assert_eq!(parse_keys(b"\x1bX"), Key::Alt('X'));
        assert_eq!(parse_keys(b"\x1b!"), Key::Alt('!'));
    }

    #[test]
    fn test_empty() {
        assert_eq!(parse_keys(b""), Key::Unknown);
    }

    #[test]
    fn test_home_end_xterm() {
        assert_eq!(parse_keys(b"\x1b[1~"), Key::Home);
        assert_eq!(parse_keys(b"\x1b[4~"), Key::End);
    }

    #[test]
    fn test_modified_arrow_parsing() {
        // ESC[1;5A = Ctrl+Up
        assert_eq!(parse_keys(b"\x1b[1;5A"), Key::Up);
        // ESC[1;5C = Ctrl+Right
        assert_eq!(parse_keys(b"\x1b[1;5C"), Key::Right);
    }

    #[test]
    fn test_shift_tab() {
        assert_eq!(parse_keys(b"\x1b[Z"), Key::Tab);
    }
}
