//! # Prompt Renderer
//!
//! Generates a colorized shell prompt in the format:
//!
//! ```text
//! user@hostname:/current/directory$ _
//! ```
//!
//! The prompt is colored green on success, red on failure.

/// ANSI color codes.
const C_RESET: &str = "\x1B[0m";
const C_GREEN: &str = "\x1B[38;5;2m";
const C_RED: &str = "\x1B[38;5;1m";
const C_BRIGHT_GREEN: &str = "\x1B[38;5;10m";
const C_BRIGHT_CYAN: &str = "\x1B[38;5;14m";
const C_WHITE: &str = "\x1B[38;5;7m";

/// Render a colorized prompt string.
///
/// `last_exit_code` controls the color:
/// - 0 (success) → green `$`
/// - non-zero (failure) → red `$`
pub fn render(last_exit_code: i32) -> String {
    let user = get_user();
    let hostname = get_hostname();
    let cwd = get_cwd();

    let dollar_color = if last_exit_code == 0 { C_BRIGHT_GREEN } else { C_RED };
    let dollar = if is_root() { "#" } else { "$" };

    format!(
        "{}{}{}{}{}{} {}{}{}{} {}{}{} ",
        C_GREEN, user,              // user in green
        C_WHITE, "@",               // @ in white
        C_BRIGHT_CYAN, hostname,    // hostname in cyan
        C_RESET,                    // reset
        C_BRIGHT_CYAN, cwd,         // cwd in cyan
        C_RESET,                    // reset
        dollar_color, dollar,       // $ in success/failure color
        C_RESET,                    // reset
    )
}

/// Get current username (or "?").
fn get_user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_else(|_| "?".to_string())
}

/// Get hostname (or "localhost").
fn get_hostname() -> String {
    #[cfg(unix)]
    {
        let mut buf = [0u8; 256];
        let result = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
        if result == 0 {
            let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
            return String::from_utf8_lossy(&buf[..len]).to_string();
        }
    }
    std::env::var("HOSTNAME").unwrap_or_else(|_| "localhost".to_string())
}

/// Get current working directory.
fn get_cwd() -> String {
    match std::env::current_dir() {
        Ok(path) => {
            let s = path.to_string_lossy().to_string();
            // Replace $HOME with ~
            if let Ok(home) = std::env::var("HOME") {
                if s.starts_with(&home) {
                    return format!("~{}", &s[home.len()..]);
                }
            }
            s
        }
        Err(_) => "?".to_string(),
    }
}

/// Check if running as root.
fn is_root() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_returns_string() {
        let prompt = render(0);
        assert!(!prompt.is_empty());
        assert!(prompt.contains('$') || prompt.contains('#'));
    }

    #[test]
    fn test_render_failure_color() {
        let prompt = render(1);
        assert!(!prompt.is_empty());
    }

    #[test]
    fn test_get_user_returns_something() {
        let user = get_user();
        assert!(!user.is_empty());
    }

    #[test]
    fn test_get_hostname_returns_something() {
        let hostname = get_hostname();
        assert!(!hostname.is_empty());
    }

    #[test]
    fn test_get_cwd_returns_something() {
        let cwd = get_cwd();
        assert!(!cwd.is_empty());
    }

    #[test]
    fn test_is_root_returns_bool() {
        // Just verify it doesn't panic
        let _root = is_root();
    }
}
