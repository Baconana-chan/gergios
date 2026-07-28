//! # Keyboard Shortcuts (Phase 4.5 — Lua-конфигурируемые горячие клавиши)
//!
//! Define compositor-wide keyboard shortcuts with a simple Lua-like config
//! format. Default bindings:
//!
//! | Shortcut | Action |
//! |----------|--------|
//! | Super+Return | LaunchTerminal |
//! | Super+Q | CloseWindow |
//! | Super+Tab | SwitchWorkspaceNext |
//! | Super+Plus | SwitchWorkspacePrev |
//! | Super+D | ToggleFloating |
//! | Super+F | ToggleMaximize |
//!
//! ## Config format (Lua-like)
//!
//! ```lua
//! -- Simple keybinding config
//! keybind({"Super", "Return"}, "launch_terminal")
//! keybind({"Super", "q"},     "close_window")
//! keybind({"Super", "Tab"},   "switch_workspace_next")
//! keybind({"Super", "Left"},  "switch_workspace_prev")
//! keybind({"Super", "d"},     "toggle_floating")
//! keybind({"Super", "f"},     "toggle_maximize")
//! ```
//!
//! A custom minimal parser built into the binary — no external Lua
//! dependency needed. The parser handles comments, strings, identifiers,
//! table literals, and function calls.

#![allow(dead_code)]

use alloc::string::String;
use alloc::vec::Vec;

use minix_input::KeySymbol;

// ── Key modifiers ─────────────────────────────────────────────────────────

/// Bitmask of held modifiers for keybindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModMask(u8);

impl ModMask {
    pub const SUPER: u8 = 0x01;
    pub const CTRL:  u8 = 0x02;
    pub const ALT:   u8 = 0x04;
    pub const SHIFT: u8 = 0x08;

    pub fn new() -> Self { Self(0) }

    pub fn contains(self, bit: u8) -> bool { (self.0 & bit) != 0 }

    /// Convert from minix_input::Modifiers to our ModMask.
    pub fn from_input_modifiers(m: minix_input::Modifiers) -> Self {
        let mut mask = 0u8;
        if m.meta() { mask |= Self::SUPER; }
        if m.ctrl() { mask |= Self::CTRL; }
        if m.alt()  { mask |= Self::ALT; }
        if m.shift(){ mask |= Self::SHIFT; }
        Self(mask)
    }
}

// ── KeyAction ─────────────────────────────────────────────────────────────

/// Actions that can be bound to keyboard shortcuts.
#[derive(Debug, Clone, PartialEq)]
pub enum KeyAction {
    /// Launch the default terminal emulator.
    LaunchTerminal,
    /// Close the currently focused window.
    CloseWindow,
    /// Switch to the next workspace.
    SwitchWorkspaceNext,
    /// Switch to the previous workspace.
    SwitchWorkspacePrev,
    /// Toggle the focused window between tiling and floating mode.
    ToggleFloating,
    /// Toggle the focused window between maximized and normal state.
    ToggleMaximize,
    /// Launch a specific app by name (parsed from config).
    LaunchApp(String),
    /// Run an arbitrary command string.
    RunCommand(String),
}

impl KeyAction {
    /// Parse an action name string from config.
    fn parse(name: &str) -> Option<Self> {
        match name {
            "launch_terminal" | "terminal" => Some(Self::LaunchTerminal),
            "close_window" | "close" => Some(Self::CloseWindow),
            "switch_workspace_next" | "workspace_next" => Some(Self::SwitchWorkspaceNext),
            "switch_workspace_prev" | "workspace_prev" => Some(Self::SwitchWorkspacePrev),
            "toggle_floating" | "floating" => Some(Self::ToggleFloating),
            "toggle_maximize" | "maximize" => Some(Self::ToggleMaximize),
            _ => {
                if let Some(cmd) = name.strip_prefix("launch:") {
                    Some(Self::LaunchApp(String::from(cmd)))
                } else if let Some(cmd) = name.strip_prefix("run:") {
                    Some(Self::RunCommand(String::from(cmd)))
                } else {
                    None
                }
            }
        }
    }
}

// ── KeyBinding ────────────────────────────────────────────────────────────

/// A single keybinding: modifiers + key + action.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyBinding {
    pub modifiers: ModMask,
    pub key: KeySymbol,
    pub action: KeyAction,
}

impl KeyBinding {
    pub fn new(modifiers: ModMask, key: KeySymbol, action: KeyAction) -> Self {
        Self { modifiers, key, action }
    }

    /// Check if a given modifier state + key press matches this binding.
    pub fn matches(&self, mods: ModMask, key: KeySymbol) -> bool {
        self.modifiers == mods && self.key == key
    }
}

// ── KeyBindings container ─────────────────────────────────────────────────

/// Container for all active keybindings.
#[derive(Debug, Clone)]
pub struct KeyBindings {
    bindings: Vec<KeyBinding>,
}

impl KeyBindings {
    /// Create a new empty set of bindings.
    pub fn new() -> Self {
        Self { bindings: Vec::new() }
    }

    /// Create the default set of keybindings.
    pub fn default() -> Self {
        Self {
            bindings: alloc::vec![
                KeyBinding::new(ModMask(ModMask::SUPER), KeySymbol::Enter, KeyAction::LaunchTerminal),
                KeyBinding::new(ModMask(ModMask::SUPER), KeySymbol::KeyQ, KeyAction::CloseWindow),
                KeyBinding::new(ModMask(ModMask::SUPER), KeySymbol::Tab, KeyAction::SwitchWorkspaceNext),
                KeyBinding::new(ModMask(ModMask::SUPER), KeySymbol::Grave, KeyAction::SwitchWorkspacePrev),
                KeyBinding::new(ModMask(ModMask::SUPER), KeySymbol::KeyD, KeyAction::ToggleFloating),
                KeyBinding::new(ModMask(ModMask::SUPER), KeySymbol::KeyF, KeyAction::ToggleMaximize),
            ],
        }
    }

    /// Look up a binding by modifier mask and key.
    /// Returns the action if found, or None.
    pub fn lookup(&self, mods: ModMask, key: KeySymbol) -> Option<&KeyAction> {
        self.bindings.iter()
            .find(|b| b.matches(mods, key))
            .map(|b| &b.action)
    }

    /// Number of registered bindings.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Check if bindings are empty.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self::default()
    }
}

// ── Minimal Lua tokenizer ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum LuaToken {
    Ident(String),
    String(String),
    Number(i64),
    LParen, RParen,
    LBrace, RBrace,
    Comma,
    Newline,
    Comment(String),
    Eof,
}

/// Tokenize a simple Lua-like config string.
fn tokenize(input: &str) -> Result<Vec<LuaToken>, &'static str> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            // Whitespace (non-newline)
            c if c.is_whitespace() && c != '\n' => { chars.next(); }
            // Newline
            '\n' => {
                chars.next();
                tokens.push(LuaToken::Newline);
            }
            // Line comment
            '-' => {
                chars.next(); // consume '-'
                if chars.peek() == Some(&'-') {
                    chars.next(); // consume second '-'
                    let mut comment = String::new();
                    while let Some(&c) = chars.peek() {
                        if c == '\n' { break; }
                        comment.push(c);
                        chars.next();
                    }
                    tokens.push(LuaToken::Comment(comment));
                } else {
                    return Err("expected '-' for comment");
                }
            }
            // Parentheses
            '(' => { chars.next(); tokens.push(LuaToken::LParen); }
            ')' => { chars.next(); tokens.push(LuaToken::RParen); }
            // Braces
            '{' => { chars.next(); tokens.push(LuaToken::LBrace); }
            '}' => { chars.next(); tokens.push(LuaToken::RBrace); }
            // Comma
            ',' => { chars.next(); tokens.push(LuaToken::Comma); }
            // String literal
            '"' => {
                chars.next(); // consume opening quote
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '"' { chars.next(); break; }
                    if c == '\\' {
                        chars.next();
                        if let Some(&esc) = chars.peek() {
                            match esc {
                                'n' => s.push('\n'),
                                't' => s.push('\t'),
                                '\\' => s.push('\\'),
                                '"' => s.push('"'),
                                _ => s.push(esc),
                            }
                            chars.next();
                        }
                    } else {
                        s.push(c);
                        chars.next();
                    }
                }
                tokens.push(LuaToken::String(s));
            }
            // Identifier or number
            c if c.is_alphabetic() || c == '_' => {
                let mut ident = String::new();
                ident.push(c);
                chars.next();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' { ident.push(c); chars.next(); }
                    else { break; }
                }
                tokens.push(LuaToken::Ident(ident));
            }
            c if c.is_ascii_digit() || c == '-' => {
                let mut num_str = String::new();
                if c == '-' {
                    num_str.push(c);
                    chars.next();
                }
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() { num_str.push(c); chars.next(); }
                    else { break; }
                }
                if let Ok(n) = num_str.parse::<i64>() {
                    tokens.push(LuaToken::Number(n));
                } else {
                    tokens.push(LuaToken::Ident(num_str));
                }
            }
            _ => {
                chars.next(); // skip unknown chars
            }
        }
    }

    tokens.push(LuaToken::Eof);
    Ok(tokens)
}

/// Parse a Lua-like config into KeyBindings.
pub fn parse_lua_config(config: &str) -> KeyBindings {
    let tokens = match tokenize(config) {
        Ok(t) => t,
        Err(_) => return KeyBindings::default(),
    };

    let mut bindings = Vec::new();
    let mut i = 0;

    while i < tokens.len() {
        // Skip newlines and comments at top level
        while i < tokens.len() && matches!(&tokens[i], LuaToken::Newline | LuaToken::Comment(_)) {
            i += 1;
        }
        if i >= tokens.len() || matches!(&tokens[i], LuaToken::Eof) { break; }

        match &tokens[i] {
            LuaToken::Ident(name) if name == "keybind" => {
                i += 1;

                // Skip newlines before '('
                while i < tokens.len() && tokens[i] == LuaToken::Newline { i += 1; }
                if i < tokens.len() && tokens[i] == LuaToken::LParen { i += 1; }

                // Skip newlines before '{'
                while i < tokens.len() && tokens[i] == LuaToken::Newline { i += 1; }
                if i >= tokens.len() || !matches!(tokens[i], LuaToken::LBrace) {
                    i += 1; continue;
                }
                i += 1; // skip '{'

                let mut mods = ModMask::new();
                let mut key = KeySymbol::Unknown;

                while i < tokens.len() && !matches!(tokens[i], LuaToken::RBrace) {
                    // Skip newlines inside table
                    while i < tokens.len() && tokens[i] == LuaToken::Newline { i += 1; }
                    if i >= tokens.len() { break; }

                    match &tokens[i] {
                        LuaToken::Comma => { i += 1; }
                        LuaToken::String(s) | LuaToken::Ident(s) => {
                            let mod_bit = match s.as_str() {
                                "Super" | "super" | "Meta" | "meta" | "Win" | "win" => Some(ModMask::SUPER),
                                "Ctrl" | "ctrl" | "Control" | "control" => Some(ModMask::CTRL),
                                "Alt" | "alt" | "Option" | "option" => Some(ModMask::ALT),
                                "Shift" | "shift" => Some(ModMask::SHIFT),
                                _ => None,
                            };
                            if let Some(bit) = mod_bit {
                                mods.0 |= bit;
                            } else {
                                key = parse_key_name(s);
                            }
                            i += 1;
                        }
                        _ => { i += 1; }
                    }
                }
                // Skip '}'
                while i < tokens.len() && tokens[i] == LuaToken::Newline { i += 1; }
                if i < tokens.len() && matches!(tokens[i], LuaToken::RBrace) { i += 1; }

                // Skip ',' and newlines
                while i < tokens.len() && tokens[i] == LuaToken::Newline { i += 1; }
                if i < tokens.len() && matches!(tokens[i], LuaToken::Comma) { i += 1; }
                while i < tokens.len() && tokens[i] == LuaToken::Newline { i += 1; }

                // Parse action string
                let action = if i < tokens.len() {
                    match &tokens[i] {
                        LuaToken::String(s) => KeyAction::parse(s),
                        LuaToken::Ident(s) => KeyAction::parse(s),
                        _ => None,
                    }
                } else {
                    None
                };
                if matches!(&tokens[i], LuaToken::String(_) | LuaToken::Ident(_)) { i += 1; }

                // Skip newlines before ')'
                while i < tokens.len() && tokens[i] == LuaToken::Newline { i += 1; }
                if i < tokens.len() && matches!(tokens[i], LuaToken::RParen) { i += 1; }

                if let Some(action) = action {
                    if key != KeySymbol::Unknown {
                        bindings.push(KeyBinding::new(mods, key, action));
                    }
                }
            }
            _ => { i += 1; }
        }
    }

    if bindings.is_empty() {
        KeyBindings::default()
    } else {
        KeyBindings { bindings }
    }
}

/// Parse a key name string to KeySymbol.
fn parse_key_name(name: &str) -> KeySymbol {
    match name {
        "Return" | "Enter" => KeySymbol::Enter,
        "Tab" => KeySymbol::Tab,
        "Space" => KeySymbol::Space,
        "Escape" | "Esc" => KeySymbol::Escape,
        "Backspace" | "BS" => KeySymbol::Backspace,
        "Delete" | "Del" => KeySymbol::Delete,
        "Insert" | "Ins" => KeySymbol::Insert,
        "Home" => KeySymbol::Home,
        "End" => KeySymbol::End,
        "PageUp" | "PgUp" => KeySymbol::PageUp,
        "PageDown" | "PgDn" => KeySymbol::PageDown,
        "Left" | "LeftArrow" => KeySymbol::LeftArrow,
        "Right" | "RightArrow" => KeySymbol::RightArrow,
        "Up" | "UpArrow" => KeySymbol::UpArrow,
        "Down" | "DownArrow" => KeySymbol::DownArrow,
        "Grave" | "Backtick" | "Tilde" => KeySymbol::Grave,
        "Minus" | "Plus" => KeySymbol::Minus,
        "Equal" | "Equals" => KeySymbol::Equal,
        // Single-letter keys
        "a" | "A" => KeySymbol::KeyA, "b" | "B" => KeySymbol::KeyB,
        "c" | "C" => KeySymbol::KeyC, "d" | "D" => KeySymbol::KeyD,
        "e" | "E" => KeySymbol::KeyE, "f" | "F" => KeySymbol::KeyF,
        "g" | "G" => KeySymbol::KeyG, "h" | "H" => KeySymbol::KeyH,
        "i" | "I" => KeySymbol::KeyI, "j" | "J" => KeySymbol::KeyJ,
        "k" | "K" => KeySymbol::KeyK, "l" | "L" => KeySymbol::KeyL,
        "m" | "M" => KeySymbol::KeyM, "n" | "N" => KeySymbol::KeyN,
        "o" | "O" => KeySymbol::KeyO, "p" | "P" => KeySymbol::KeyP,
        "q" | "Q" => KeySymbol::KeyQ, "r" | "R" => KeySymbol::KeyR,
        "s" | "S" => KeySymbol::KeyS, "t" | "T" => KeySymbol::KeyT,
        "u" | "U" => KeySymbol::KeyU, "v" | "V" => KeySymbol::KeyV,
        "w" | "W" => KeySymbol::KeyW, "x" | "X" => KeySymbol::KeyX,
        "y" | "Y" => KeySymbol::KeyY, "z" | "Z" => KeySymbol::KeyZ,
        // Number keys
        "0" => KeySymbol::Key0, "1" => KeySymbol::Key1,
        "2" => KeySymbol::Key2, "3" => KeySymbol::Key3,
        "4" => KeySymbol::Key4, "5" => KeySymbol::Key5,
        "6" => KeySymbol::Key6, "7" => KeySymbol::Key7,
        "8" => KeySymbol::Key8, "9" => KeySymbol::Key9,
        // Function keys
        "F1" => KeySymbol::F1, "F2" => KeySymbol::F2,
        "F3" => KeySymbol::F3, "F4" => KeySymbol::F4,
        "F5" => KeySymbol::F5, "F6" => KeySymbol::F6,
        "F7" => KeySymbol::F7, "F8" => KeySymbol::F8,
        "F9" => KeySymbol::F9, "F10" => KeySymbol::F10,
        "F11" => KeySymbol::F11, "F12" => KeySymbol::F12,
        _ => KeySymbol::Unknown,
    }
}

// ════════════════════════════════════════════════════════════════
// Tests
// ════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bindings() {
        let kb = KeyBindings::default();
        assert_eq!(kb.len(), 6);

        // Super+Enter → LaunchTerminal
        assert_eq!(
            kb.lookup(ModMask(ModMask::SUPER), KeySymbol::Enter),
            Some(&KeyAction::LaunchTerminal),
        );

        // Super+Q → CloseWindow
        assert_eq!(
            kb.lookup(ModMask(ModMask::SUPER), KeySymbol::KeyQ),
            Some(&KeyAction::CloseWindow),
        );

        // No match without Super
        assert_eq!(kb.lookup(ModMask::new(), KeySymbol::KeyQ), None);

        // Wrong key
        assert_eq!(kb.lookup(ModMask(ModMask::SUPER), KeySymbol::KeyZ), None);
    }

    #[test]
    fn matching_requires_exact_mods() {
        let kb = KeyBindings::default();
        // Super+Ctrl+Q should NOT match Super+Q
        assert_eq!(
            kb.lookup(ModMask(ModMask::SUPER | ModMask::CTRL), KeySymbol::KeyQ),
            None,
        );
    }

    #[test]
    fn parse_launch_terminal() {
        assert_eq!(KeyAction::parse("launch_terminal"), Some(KeyAction::LaunchTerminal));
        assert_eq!(KeyAction::parse("terminal"), Some(KeyAction::LaunchTerminal));
    }

    #[test]
    fn parse_launch_app() {
        assert_eq!(
            KeyAction::parse("launch:file-manager"),
            Some(KeyAction::LaunchApp(String::from("file-manager"))),
        );
    }

    #[test]
    fn parse_run_command() {
        assert_eq!(
            KeyAction::parse("run:/bin/ls -la"),
            Some(KeyAction::RunCommand(String::from("/bin/ls -la"))),
        );
    }

    #[test]
    fn parse_invalid_action() {
        assert_eq!(KeyAction::parse("nonexistent"), None);
    }

    #[test]
    fn lua_config_simple() {
        let config = r#"
            keybind({"Super", "Return"}, "launch_terminal")
            keybind({"Super", "q"}, "close_window")
        "#;
        let kb = parse_lua_config(config);
        assert_eq!(kb.len(), 2);
        assert_eq!(
            kb.lookup(ModMask(ModMask::SUPER), KeySymbol::Enter),
            Some(&KeyAction::LaunchTerminal),
        );
        assert_eq!(
            kb.lookup(ModMask(ModMask::SUPER), KeySymbol::KeyQ),
            Some(&KeyAction::CloseWindow),
        );
    }

    #[test]
    fn lua_config_with_comments() {
        let config = r#"
            -- This is a comment
            keybind({"Super", "Tab"}, "workspace_next")
            -- Another comment
        "#;
        let kb = parse_lua_config(config);
        assert_eq!(kb.len(), 1);
        assert_eq!(
            kb.lookup(ModMask(ModMask::SUPER), KeySymbol::Tab),
            Some(&KeyAction::SwitchWorkspaceNext),
        );
    }

    #[test]
    fn lua_config_multiple_modifiers() {
        let config = r#"keybind({"Super", "Ctrl", "q"}, "close_window")"#;
        let kb = parse_lua_config(config);
        assert_eq!(kb.len(), 1);
        assert_eq!(
            kb.lookup(ModMask(ModMask::SUPER | ModMask::CTRL), KeySymbol::KeyQ),
            Some(&KeyAction::CloseWindow),
        );
    }

    #[test]
    fn lua_config_toggle_floating() {
        let config = r#"keybind({"Super", "d"}, "toggle_floating")"#;
        let kb = parse_lua_config(config);
        assert_eq!(kb.len(), 1);
        assert_eq!(
            kb.lookup(ModMask(ModMask::SUPER), KeySymbol::KeyD),
            Some(&KeyAction::ToggleFloating),
        );
    }

    #[test]
    fn lua_config_launch_app() {
        let config = r#"keybind({"Super", "e"}, "launch:file-manager")"#;
        let kb = parse_lua_config(config);
        assert_eq!(kb.len(), 1);
        assert_eq!(
            kb.lookup(ModMask(ModMask::SUPER), KeySymbol::KeyE),
            Some(&KeyAction::LaunchApp(String::from("file-manager"))),
        );
    }

    #[test]
    fn invalid_config_falls_back_to_defaults() {
        let config = "this is not valid lua at all !!!";
        let kb = parse_lua_config(config);
        assert_eq!(kb.len(), 6); // default
    }

    #[test]
    fn empty_config_falls_back_to_defaults() {
        let kb = parse_lua_config("");
        assert_eq!(kb.len(), 6); // default
    }

    #[test]
    fn modmask_from_lua_config() {
        // Test that the parser correctly identifies modifier combinations
        let config = r##"keybind({"Super", "Ctrl", "q"}, "close_window")"##;
        let kb = parse_lua_config(config);
        assert_eq!(kb.len(), 1);
        assert_eq!(
            kb.lookup(ModMask(ModMask::SUPER | ModMask::CTRL), KeySymbol::KeyQ),
            Some(&KeyAction::CloseWindow),
        );
    }

    #[test]
    fn modmask_from_input() {
        let input_mods = minix_input::Modifiers::from_bits(0x88); // both Meta bits
        let mask = ModMask::from_input_modifiers(input_mods);
        assert!(mask.contains(ModMask::SUPER));
        assert!(!mask.contains(ModMask::CTRL));
        assert!(!mask.contains(ModMask::ALT));
        assert!(!mask.contains(ModMask::SHIFT));
    }

    #[test]
    fn parse_key_names() {
        assert_eq!(parse_key_name("Return"), KeySymbol::Enter);
        assert_eq!(parse_key_name("Tab"), KeySymbol::Tab);
        assert_eq!(parse_key_name("Escape"), KeySymbol::Escape);
        assert_eq!(parse_key_name("Left"), KeySymbol::LeftArrow);
        assert_eq!(parse_key_name("q"), KeySymbol::KeyQ);
        assert_eq!(parse_key_name("F1"), KeySymbol::F1);
        assert_eq!(parse_key_name("Space"), KeySymbol::Space);
        assert_eq!(parse_key_name("nonexistent"), KeySymbol::Unknown);
    }

    #[test]
    fn tokenizer_basics() {
        let tokens = tokenize("keybind({\"Super\", \"q\"}, \"close\")").unwrap();
        assert!(tokens.contains(&LuaToken::Ident(String::from("keybind"))));
        assert!(tokens.contains(&LuaToken::LParen));
        assert!(tokens.contains(&LuaToken::LBrace));
        assert!(tokens.contains(&LuaToken::String(String::from("Super"))));
        assert!(tokens.contains(&LuaToken::String(String::from("q"))));
        assert!(tokens.contains(&LuaToken::Comma));
        assert!(tokens.contains(&LuaToken::String(String::from("close"))));
        assert!(tokens.contains(&LuaToken::RParen));
    }

    #[test]
    fn tokenizer_comment() {
        let tokens = tokenize("-- hello\nkeybind").unwrap();
        assert!(tokens.iter().any(|t| matches!(t, LuaToken::Comment(c) if c.trim() == "hello")));
        assert!(tokens.contains(&LuaToken::Ident(String::from("keybind"))));
    }
}
