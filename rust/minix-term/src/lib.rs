// minix-term — Minimal terminal library for GergiOS.
//
// Provides raw mode terminal I/O via termios + ANSI escape codes.
// No ncurses dependency — uses only POSIX termios and write()/read().
//
// Cross-platform: uses cfg(unix) for POSIX-specific APIs (termios, ioctl, select).
// On non-Unix (Windows), provides a minimal stub that returns errors — enough
// for cargo check during development.
//
// # Quick start
//
// ```no_run
// use minix_term::Terminal;
//
// let mut term = Terminal::new().unwrap();
// term.clear();
// term.set_cursor(1, 1);
// term.set_fg(2); // green
// writeln!(term, "Hello, GergiOS!").unwrap();
// term.reset_style();
// term.show_cursor();
// let key = term.read_key().unwrap();
// println!("You pressed: {:?}", key);
// // Terminal drops → restores original termios
// ```

mod keys;
pub mod mouse;
pub mod gamepad;

use std::io::{self, Write};
use std::time::Duration;

pub use keys::Key;

// ===========================================================================
// Cross-platform raw fd type
// ===========================================================================

/// Cross-platform raw file descriptor.
/// On Unix this is `std::os::unix::io::RawFd` (= i32/c_int).
/// On Windows we use `i32` to match.
#[cfg(unix)]
type RawFd = std::os::unix::io::RawFd;
#[cfg(windows)]
type RawFd = i32;

/// Standard input fd (0).
const STDIN_FD: RawFd = 0;
/// Standard output fd (1).
const STDOUT_FD: RawFd = 1;

// ===========================================================================
// Termios flag constants (MINIX/NetBSD values)
// ===========================================================================

#[cfg(unix)]
mod termios_consts {
    /// Input flags (c_iflag)
    pub const BRKINT: libc::tcflag_t = 0x0002;
    pub const ICRNL: libc::tcflag_t = 0x0100;
    pub const IGNBRK: libc::tcflag_t = 0x0001;
    pub const IGNCR: libc::tcflag_t = 0x0080;
    pub const IGNPAR: libc::tcflag_t = 0x0004;
    pub const INLCR: libc::tcflag_t = 0x0040;
    pub const INPCK: libc::tcflag_t = 0x0010;
    pub const ISTRIP: libc::tcflag_t = 0x0020;
    pub const IXON: libc::tcflag_t = 0x0400;
    pub const PARMRK: libc::tcflag_t = 0x0008;

    /// Output flags (c_oflag)
    pub const OPOST: libc::tcflag_t = 0x0001;
    pub const ONLCR: libc::tcflag_t = 0x0002;
    pub const OCRNL: libc::tcflag_t = 0x0004;
    pub const ONOCR: libc::tcflag_t = 0x0010;
    pub const ONLRET: libc::tcflag_t = 0x0020;

    /// Control flags (c_cflag)
    pub const CSIZE: libc::tcflag_t = 0x0300;
    pub const CS8: libc::tcflag_t = 0x0300;
    pub const PARENB: libc::tcflag_t = 0x0400;
    pub const CREAD: libc::tcflag_t = 0x0080;
    pub const HUPCL: libc::tcflag_t = 0x1000;
    pub const CLOCAL: libc::tcflag_t = 0x2000;

    /// Local flags (c_lflag)
    pub const ECHO: libc::tcflag_t = 0x0008;
    pub const ECHOE: libc::tcflag_t = 0x0002; // BSD: ECHOE = 0x0002
    pub const ECHOK: libc::tcflag_t = 0x0004;
    pub const ECHONL: libc::tcflag_t = 0x0010;
    pub const ECHOPRT: libc::tcflag_t = 0x0020;
    pub const ECHOCTL: libc::tcflag_t = 0x0040;
    pub const ICANON: libc::tcflag_t = 0x0100;
    pub const IEXTEN: libc::tcflag_t = 0x00080000; // NetBSD/MINIX value
    pub const ISIG: libc::tcflag_t = 0x0080;
    pub const NOFLSH: libc::tcflag_t = 0x0400;
    pub const PENDIN: libc::tcflag_t = 0x4000;
    pub const TOSTOP: libc::tcflag_t = 0x00100000;

    /// Special characters indices (c_cc) — NetBSD/MINIX layout
    pub const VINTR: usize = 0;
    pub const VQUIT: usize = 1;
    pub const VERASE: usize = 2;
    pub const VKILL: usize = 3;
    pub const VMIN: usize = 4;
    pub const VTIME: usize = 5;
    pub const VEOL: usize = 6;
    pub const VEOL2: usize = 7;
    pub const VSUSP: usize = 10;      // Ctrl+Z
    pub const VDSUSP: usize = 11;     // Ctrl+Y (BSD)
}

#[cfg(unix)]
use termios_consts::*;

// ===========================================================================
// ioctl constants (x86_64)
// ===========================================================================

/// _IOR('t', 104, struct winsize) on x86_64
#[cfg(unix)]
const TIOCGWINSZ: libc::c_ulong = 0x40087468;

// ===========================================================================
// Terminal — main type
// ===========================================================================

/// A handle to the terminal in raw mode.
///
/// On Unix: switches the terminal to raw mode (no echo, no line buffering,
/// character-by-character input). On drop, restores the original terminal settings.
///
/// On non-Unix (Windows): stub implementation that writes ANSI codes to stdout
/// but does not support raw mode. Used only for cargo check during development.
pub struct Terminal {
    /// Saved original terminal attributes (Unix only).
    #[cfg(unix)]
    orig_termios: Option<libc::termios>,
    /// Stdin file descriptor.
    stdin_fd: RawFd,
    /// Stdout file descriptor.
    stdout_fd: RawFd,
}

// ===========================================================================
// Terminal — Unix implementation
// ===========================================================================

#[cfg(unix)]
impl Terminal {
    /// Open the terminal and switch to raw mode (Unix only).
    ///
    /// Returns an error if tcgetattr or tcsetattr fails.
    pub fn new() -> io::Result<Self> {
        let stdin_fd = STDIN_FD;
        let stdout_fd = STDOUT_FD;

        // Save original termios
        let mut orig_termios = unsafe { std::mem::zeroed::<libc::termios>() };
        if unsafe { libc::tcgetattr(stdin_fd, &mut orig_termios) } != 0 {
            return Err(io::Error::last_os_error());
        }

        // Build raw termios (BSD cfmakeraw equivalent)
        let mut raw = orig_termios;

        // Input flags: disable all input processing
        raw.c_iflag &= !(BRKINT | ICRNL | IGNBRK | IGNCR | IGNPAR | INLCR | INPCK | ISTRIP | IXON | PARMRK);

        // Output flags: disable all output processing
        raw.c_oflag &= !(OPOST | ONLCR | OCRNL | ONOCR | ONLRET);

        // Control flags: set CS8, enable CREAD, CLOCAL, disable PARENB, HUPCL
        raw.c_cflag = (raw.c_cflag & !CSIZE) | CS8;
        raw.c_cflag |= CREAD | CLOCAL;
        raw.c_cflag &= !(PARENB | HUPCL);

        // Local flags: disable all line-discipline processing
        raw.c_lflag &= !(ECHO | ECHOE | ECHOK | ECHONL | ECHOPRT | ECHOCTL
                        | ICANON | IEXTEN | ISIG | NOFLSH | PENDIN | TOSTOP);

        // Timeout/byte settings: read returns as soon as at least 1 byte is available
        raw.c_cc[VMIN] = 1;   // wait for at least 1 byte
        raw.c_cc[VTIME] = 0;  // no timer (blocking read)

        // Disable special characters (_POSIX_VDISABLE = 0xFF on NetBSD/MINIX)
        raw.c_cc[VINTR] = 0xFF;
        raw.c_cc[VQUIT] = 0xFF;
        raw.c_cc[VSUSP] = 0xFF;
        raw.c_cc[VDSUSP] = 0xFF;

        // Apply raw mode
        if unsafe { libc::tcsetattr(stdin_fd, libc::TCSANOW, &raw) } != 0 {
            return Err(io::Error::last_os_error());
        }

        // Hide cursor by default
        write!(io::stdout(), "\x1B[?25l").ok();
        io::stdout().flush().ok();

        Ok(Terminal {
            orig_termios: Some(orig_termios),
            stdin_fd,
            stdout_fd,
        })
    }

    // =======================================================================
    // Input (Unix: POSIX read + select)
    // =======================================================================

    /// Read a single key press from the terminal.
    pub fn read_key(&mut self) -> io::Result<Key> {
        let mut buf = [0u8; 32];
        let n = unsafe {
            libc::read(
                self.stdin_fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };

        if n < 0 {
            return Err(io::Error::last_os_error());
        }

        if n == 0 {
            return Ok(Key::Unknown);
        }

        Ok(keys::parse_keys(&buf[..n as usize]))
    }

    /// Check if a key is available for reading without blocking.
    pub fn poll_key(&mut self, timeout: Duration) -> io::Result<bool> {
        unsafe {
            let mut fds: libc::fd_set = std::mem::zeroed();
            libc::FD_SET(self.stdin_fd, &mut fds);

            let mut tv = libc::timeval {
                tv_sec: timeout.as_secs() as libc::time_t,
                tv_usec: timeout.subsec_micros() as libc::suseconds_t,
            };

            let ret = libc::select(
                self.stdin_fd + 1,
                &mut fds,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut tv,
            );

            if ret < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(libc::FD_ISSET(self.stdin_fd, &fds))
            }
        }
    }

    /// Read a key with a timeout.
    pub fn read_key_timeout(&mut self, timeout: Duration) -> io::Result<Option<Key>> {
        if self.poll_key(timeout)? {
            self.read_key().map(Some)
        } else {
            Ok(None)
        }
    }

    // =======================================================================
    // Terminal queries (Unix: ioctl)
    // =======================================================================

    /// Get terminal size in (rows, columns).
    pub fn size(&self) -> (u16, u16) {
        unsafe {
            let mut ws: libc::winsize = std::mem::zeroed();
            if libc::ioctl(self.stdout_fd, TIOCGWINSZ, &mut ws as *mut _) == 0 {
                (ws.ws_row, ws.ws_col)
            } else {
                (24, 80) // fallback
            }
        }
    }

    /// Get number of rows.
    pub fn rows(&self) -> u16 {
        self.size().0
    }

    /// Get number of columns.
    pub fn cols(&self) -> u16 {
        self.size().1
    }
}

// ===========================================================================
// Terminal — non-Unix stub implementation (Windows development builds)
// ===========================================================================

#[cfg(not(unix))]
impl Terminal {
    /// Create a stub terminal (non-Unix platform).
    ///
    /// On Windows, raw mode is not supported — returns an error immediately.
    /// This prevents the TUI shell from entering an infinite spin loop
    /// where read_key() always returns Key::Unknown.
    pub fn new() -> io::Result<Self> {
        Err(io::Error::new(io::ErrorKind::Unsupported,
            "raw terminal I/O requires a Unix-like platform (MINIX, Linux, macOS)"))
    }

    /// Stub: always returns Unknown.
    pub fn read_key(&mut self) -> io::Result<Key> {
        Ok(Key::Unknown)
    }

    /// Stub: always returns false (no key available).
    pub fn poll_key(&mut self, _timeout: Duration) -> io::Result<bool> {
        Ok(false)
    }

    /// Stub: always returns None.
    pub fn read_key_timeout(&mut self, _timeout: Duration) -> io::Result<Option<Key>> {
        Ok(None)
    }

    /// Stub: returns (24, 80).
    pub fn size(&self) -> (u16, u16) {
        (24, 80)
    }

    /// Stub: returns 24.
    pub fn rows(&self) -> u16 {
        24
    }

    /// Stub: returns 80.
    pub fn cols(&self) -> u16 {
        80
    }
}

// ===========================================================================
// Terminal — shared methods (work with ANSI escape codes on any platform)
// ===========================================================================

impl Terminal {
    // =======================================================================
    // Output methods (ANSI escape codes)
    // =======================================================================

    /// Clear the entire screen and move cursor to home (1,1).
    pub fn clear(&self) {
        write!(io::stdout(), "\x1B[2J\x1B[H").ok();
        io::stdout().flush().ok();
    }

    /// Clear from cursor to end of line.
    pub fn clear_line(&self) {
        write!(io::stdout(), "\x1B[K").ok();
    }

    /// Clear from cursor to end of screen.
    pub fn clear_to_end(&self) {
        write!(io::stdout(), "\x1B[0J").ok();
    }

    /// Move cursor to row `row` (1-based), column `col` (1-based).
    pub fn set_cursor(&self, row: u16, col: u16) {
        write!(io::stdout(), "\x1B[{};{}H", row, col).ok();
    }

    /// Move cursor up by `n` rows.
    pub fn cursor_up(&self, n: u16) {
        write!(io::stdout(), "\x1B[{}A", n).ok();
    }

    /// Move cursor down by `n` rows.
    pub fn cursor_down(&self, n: u16) {
        write!(io::stdout(), "\x1B[{}B", n).ok();
    }

    /// Move cursor right by `n` columns.
    pub fn cursor_right(&self, n: u16) {
        write!(io::stdout(), "\x1B[{}C", n).ok();
    }

    /// Move cursor left by `n` columns.
    pub fn cursor_left(&self, n: u16) {
        write!(io::stdout(), "\x1B[{}D", n).ok();
    }

    /// Save cursor position.
    pub fn save_cursor(&self) {
        write!(io::stdout(), "\x1B[s").ok();
    }

    /// Restore saved cursor position.
    pub fn restore_cursor(&self) {
        write!(io::stdout(), "\x1B[u").ok();
    }

    /// Hide cursor.
    pub fn hide_cursor(&self) {
        write!(io::stdout(), "\x1B[?25l").ok();
    }

    /// Show cursor.
    pub fn show_cursor(&self) {
        write!(io::stdout(), "\x1B[?25h").ok();
    }

    // =======================================================================
    // Style / color methods
    // =======================================================================

    /// Reset all text attributes (colors, bold, underline, etc.).
    pub fn reset_style(&self) {
        write!(io::stdout(), "\x1B[0m").ok();
    }

    /// Set foreground color from the 8-bit palette (0..255).
    pub fn set_fg(&self, color: u8) {
        write!(io::stdout(), "\x1B[38;5;{}m", color).ok();
    }

    /// Set background color from the 8-bit palette (0..255).
    pub fn set_bg(&self, color: u8) {
        write!(io::stdout(), "\x1B[48;5;{}m", color).ok();
    }

    /// Set foreground color using 24-bit RGB.
    pub fn set_fg_rgb(&self, r: u8, g: u8, b: u8) {
        write!(io::stdout(), "\x1B[38;2;{};{};{}m", r, g, b).ok();
    }

    /// Set background color using 24-bit RGB.
    pub fn set_bg_rgb(&self, r: u8, g: u8, b: u8) {
        write!(io::stdout(), "\x1B[48;2;{};{};{}m", r, g, b).ok();
    }

    /// Enable bold text.
    pub fn set_bold(&self) {
        write!(io::stdout(), "\x1B[1m").ok();
    }

    /// Enable dim text.
    pub fn set_dim(&self) {
        write!(io::stdout(), "\x1B[2m").ok();
    }

    /// Enable underline text.
    pub fn set_underline(&self) {
        write!(io::stdout(), "\x1B[4m").ok();
    }

    /// Enable blink text.
    pub fn set_blink(&self) {
        write!(io::stdout(), "\x1B[5m").ok();
    }

    /// Enable reverse video (swap foreground and background).
    pub fn set_reverse(&self) {
        write!(io::stdout(), "\x1B[7m").ok();
    }

    /// Enable strikethrough text.
    pub fn set_strikethrough(&self) {
        write!(io::stdout(), "\x1B[9m").ok();
    }
}

// ===========================================================================
// Write implementation
// ===========================================================================

impl Write for Terminal {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        #[cfg(unix)]
        {
            let ret = unsafe {
                libc::write(
                    self.stdout_fd,
                    buf.as_ptr() as *const libc::c_void,
                    buf.len(),
                )
            };
            if ret < 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(ret as usize)
            }
        }
        #[cfg(not(unix))]
        {
            io::stdout().write(buf)
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        #[cfg(unix)]
        {
            // write() on POSIX is unbuffered at the syscall level
            Ok(())
        }
        #[cfg(not(unix))]
        {
            io::stdout().flush()
        }
    }
}

// ===========================================================================
// Drop — restore terminal (Unix only)
// ===========================================================================

#[cfg(unix)]
impl Drop for Terminal {
    fn drop(&mut self) {
        // Show cursor and reset style
        write!(io::stdout(), "\x1B[?25h\x1B[0m").ok();
        io::stdout().flush().ok();

        // Restore original termios if saved
        if let Some(ref orig) = self.orig_termios {
            unsafe {
                libc::tcsetattr(self.stdin_fd, libc::TCSANOW, orig);
            }
        }
    }
}

#[cfg(not(unix))]
impl Drop for Terminal {
    fn drop(&mut self) {
        write!(io::stdout(), "\x1B[?25h\x1B[0m").ok();
        io::stdout().flush().ok();
    }
}
