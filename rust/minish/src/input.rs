//! # Raw-mode Line Input
//!
//! Reads input character-by-character using raw terminal mode.
//! Only available on Unix (uses `libc::termios`).
//!
//! Supports:
//! - Arrow keys: Up/Down for history, Left/Right for cursor movement
//! - Tab completion (single match fills, multiple matches show list)
//! - Home/End, Backspace/Delete
//! - Ctrl+D (EOF on empty line), Ctrl+U (clear line), Ctrl+L (clear screen)
//! - Ctrl+C (interrupt — returns empty line)

use std::io::{self, Write};

/// Read a line of input with line editing support.
///
/// Returns `Some(line)` on Enter, `None` on EOF (Ctrl+D on empty line).
///
/// On Unix, uses raw terminal mode for full interactive editing.
/// On non-Unix, falls back to simple `stdin::read_line()`.
#[cfg(unix)]
pub fn read_line(
    prompt: &str,
    hist: &mut crate::history::History,
    _last_ec: i32,
) -> Option<String> {
    _read_line_unix(prompt, hist)
}

/// Non-Unix fallback: simple stdin line reading.
#[cfg(not(unix))]
pub fn read_line(
    prompt: &str,
    _hist: &mut crate::history::History,
    _last_ec: i32,
) -> Option<String> {
    print!("{}", prompt);
    io::stdout().flush().ok();
    let mut line = String::new();
    match io::stdin().read_line(&mut line) {
        Ok(0) => None,
        Ok(_) => Some(line.trim().to_string()),
        Err(_) => None,
    }
}

/// Unix-specific raw-mode implementation.
#[cfg(unix)]
fn _read_line_unix(prompt: &str, hist: &mut crate::history::History) -> Option<String> {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    // Save terminal attributes for raw mode
    let mut termios: libc::termios = unsafe { std::mem::zeroed() };
    unsafe {
        libc::tcgetattr(libc::STDIN_FILENO, &mut termios);
    }
    let original = termios;

    // Set raw mode (no echo, no buffering, no line discipline)
    unsafe {
        libc::cfmakeraw(&mut termios);
        libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &termios);
    }

    // Restore terminal mode helper
    let restore = || {
        unsafe {
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &original);
        }
    };

    let mut line = String::new();
    let mut cursor: usize = 0;

    // Print prompt
    print!("{}", prompt);
    stdout.flush().ok();

    let mut buf = [0u8; 16];

    loop {
        let n = match stdin.read(&mut buf) {
            Ok(n) if n > 0 => n,
            _ => {
                restore();
                return None;
            }
        };

        // Single-byte character
        if n == 1 && buf[0] < 128 {
            let key = buf[0];

            match key {
                // Enter
                13 | 10 => {
                    println!();
                    restore();
                    return Some(line);
                }

                // Backspace
                127 | 8 => {
                    if cursor > 0 {
                        line.remove(cursor - 1);
                        cursor -= 1;
                        redraw(&line, cursor);
                    }
                }

                // Tab — completion
                9 => {
                    if line.is_empty() {
                        continue;
                    }
                    do_tab_complete(&mut line, &mut cursor);
                }

                // Ctrl+C — interrupt (behave like Enter with empty line)
                3 => {
                    println!("^C");
                    restore();
                    return Some(String::new());
                }

                // Ctrl+D — EOF on empty line
                4 => {
                    if line.is_empty() {
                        println!();
                        restore();
                        return None;
                    }
                }

                // Ctrl+U — clear line
                21 => {
                    line.clear();
                    cursor = 0;
                    redraw(&line, cursor);
                }

                // Ctrl+L — clear screen
                12 => {
                    print!("\u{1b}[2J\u{1b}[H");
                    print!("{}", prompt);
                    redraw(&line, cursor);
                }

                // Escape sequence (arrow keys, Home, End, Delete)
                27 => {
                    let mut esc = [0u8; 8];
                    let en = match stdin.read(&mut esc) {
                        Ok(n) => n,
                        _ => continue,
                    };

                    // CSI sequences: ESC [ ...
                    if en >= 1 && esc[0] == b'[' {
                        match esc[1] {
                            // Up arrow — history older
                            b'A' => {
                                if !hist.is_empty() {
                                    hist.start_nav(&line);
                                    if let Some(cmd) = hist.older() {
                                        line = cmd.to_string();
                                        cursor = line.len();
                                        redraw(&line, cursor);
                                    }
                                }
                            }

                            // Down arrow — history newer
                            b'B' => {
                                if !hist.is_empty() && hist.is_navigating() {
                                    match hist.newer() {
                                        Some(cmd) => {
                                            line = cmd.to_string();
                                        }
                                        None => {
                                            hist.stop_nav();
                                            line.clear();
                                        }
                                    }
                                    cursor = line.len();
                                    redraw(&line, cursor);
                                }
                            }

                            // Right arrow
                            b'C' => {
                                if cursor < line.len() {
                                    cursor += 1;
                                    print!("\u{1b}[C");
                                    stdout.flush().ok();
                                }
                            }

                            // Left arrow
                            b'D' => {
                                if cursor > 0 {
                                    cursor -= 1;
                                    print!("\u{1b}[D");
                                    stdout.flush().ok();
                                }
                            }

                            // Home
                            b'H' => {
                                if cursor > 0 {
                                    cursor = 0;
                                    print!("\u{1b}[{}D", line.len());
                                    stdout.flush().ok();
                                }
                            }

                            // End
                            b'F' => {
                                if cursor < line.len() {
                                    print!("\u{1b}[{}C", line.len() - cursor);
                                    cursor = line.len();
                                    stdout.flush().ok();
                                }
                            }

                            // Delete (ESC [ 3 ~)
                            b'3' if en >= 3 && esc[2] == b'~' => {
                                if cursor < line.len() {
                                    line.remove(cursor);
                                    redraw(&line, cursor);
                                }
                            }

                            _ => {}
                        }
                    }

                    // SS3 sequences: ESC O ...
                    if en >= 1 && esc[0] == b'O' {
                        match esc[1] {
                            b'H' => {
                                cursor = 0;
                                print!("\u{1b}[{}D", line.len());
                                stdout.flush().ok();
                            }
                            b'F' => {
                                print!("\u{1b}[{}C", line.len() - cursor);
                                cursor = line.len();
                                stdout.flush().ok();
                            }
                            _ => {}
                        }
                    }
                }

                // Printable ASCII
                32..=126 => {
                    line.insert(cursor, key as char);
                    cursor += 1;
                    redraw(&line, cursor);
                }

                _ => {}
            }
        }
    }
}

/// Perform tab completion: extract the last word, query completions, apply match.
#[cfg(unix)]
fn do_tab_complete(line: &mut String, cursor: &mut usize) {
    let line_before_cursor = &line[..*cursor];
    let is_first_word = !line_before_cursor.contains(' ');

    // Extract ONLY the partial word being completed (after last space)
    let partial = line_before_cursor
        .split_whitespace()
        .last()
        .unwrap_or("");

    if partial.is_empty() {
        print!("\u{7}"); // bell
        io::stdout().flush().ok();
        return;
    }

    let completions = crate::complete::complete(partial, is_first_word);

    if completions.is_empty() {
        print!("\u{7}"); // bell — no matches
        io::stdout().flush().ok();
    } else if completions.len() == 1 {
        // Single match — replace the last word in line
        let last_space = line_before_cursor.rfind(' ');
        if let Some(space_pos) = last_space {
            line.replace_range(space_pos + 1..*cursor, &completions[0]);
            *cursor = space_pos + 1 + completions[0].len();
        } else {
            *line = completions[0].clone();
            *cursor = completions[0].len();
        }
        redraw(line, *cursor);
    } else {
        // Multiple matches — show list below, then re-prompt
        println!();
        for c in &completions {
            print!("{}  ", c);
        }
        println!();
        print!("{}", crate::prompt::render(0));
        redraw(line, *cursor);
    }
}

/// Redraw the current line on the terminal (ANSI escape codes).
#[cfg(unix)]
fn redraw(line: &str, cursor: usize) {
    // Clear to end of line, carriage return, print line, position cursor
    print!("\u{1b}[2K\r{}", line);
    if cursor < line.len() {
        print!("\u{1b}[{}D", line.len() - cursor);
    }
    io::stdout().flush().ok();
}

// ============================================================================
// Tests
// ============================================================================

// Tests for raw-mode functions only exist on Unix (where the functions exist).
#[cfg(all(test, unix))]
mod unix_tests {
    use super::*;

    #[test]
    fn test_redraw_doesnt_panic() {
        redraw("hello", 3);
        redraw("", 0);
        redraw("test", 4);
    }
}
