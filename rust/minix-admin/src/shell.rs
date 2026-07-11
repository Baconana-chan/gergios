//! # Interactive Shell (TUI)
//!
//! Interactive TUI shell for `minix-admin` using `minix-term` as backend.
//!
//! ## Features
//!
//! - Colorful prompt (`admin@hostname > `)
//! - Line editing: insert anywhere, cursor keys, Home/End
//! - Command history: Up/Down arrows
//! - Tab completion: commands, subcommands, service names
//! - Colorized output: errors in red, success in green, headers in bold cyan
//!
//! ## Architecture
//!
//! ```text
//! shell::run()
//!   │
//!   ├── init Terminal (raw mode, hide cursor)
//!   ├── loop:
//!   │     ├── render prompt + line
//!   │     ├── read_key()
//!   │     ├── handle key:
//!   │     │   ├── Char → insert into line
//!   │     │   ├── Backspace → delete before cursor
//!   │     │   ├── Enter → execute command
//!   │     │   ├── Up/Down → history navigation
//!   │     │   ├── Left/Right → cursor movement
//!   │     │   ├── Home/End → jump to start/end
//!   │     │   ├── Tab → autocomplete
//!   │     │   ├── Ctrl+C → interrupt (new prompt)
//!   │     │   ├── Ctrl+D → exit (if line empty)
//!   │     │   └── Ctrl+L → clear screen
//!   │     │
//!   │     └── handle_enter():
//!   │         ├── add to history
//!   │         ├── parse line into args
//!   │         ├── dispatch to existing CLI functions
//!   │         └── display output (colorized)
//!   │
//!   └── cleanup: show cursor, restore termios
//! ```

use crate::{cli, network, security, services, system};
use minix_term::{Key, Terminal};
use std::io::{self, Write};

// ===========================================================================
// Constants
// ===========================================================================

/// ANSI color codes.
const C_RESET: &str = "\x1B[0m";
const C_BOLD: &str = "\x1B[1m";
const C_DIM: &str = "\x1B[2m";
const C_RED: &str = "\x1B[38;5;1m";
const C_GREEN: &str = "\x1B[38;5;2m";
const C_YELLOW: &str = "\x1B[38;5;3m";
const C_BLUE: &str = "\x1B[38;5;4m";
const C_MAGENTA: &str = "\x1B[38;5;5m";
const C_CYAN: &str = "\x1B[38;5;6m";
const C_WHITE: &str = "\x1B[38;5;7m";
const C_BRIGHT_GREEN: &str = "\x1B[38;5;10m";
const C_BRIGHT_CYAN: &str = "\x1B[38;5;14m";

/// Prompt string.
const PROMPT: &str = "admin@gergios";

/// Max history entries.
const MAX_HISTORY: usize = 100;

/// Max completions to show inline.
const MAX_INLINE_COMPLETIONS: usize = 10;

// ===========================================================================
// Command table for tab-completion
// ===========================================================================

/// Known top-level commands.
const COMMANDS: &[&str] = &[
    "services",
    "system",
    "network",
    "security",
    "help",
    "version",
    "exit",
    "quit",
    "clear",
];

/// Subcommands for tab-completion.
const SERVICE_SUBCMDS: &[&str] = &["list", "status", "start", "stop", "restart"];
const SYSTEM_SUBCMDS: &[&str] = &["info", "cpu", "memory", "disk", "uptime"];
const NETWORK_SUBCMDS: &[&str] = &["interfaces", "status", "stats", "route", "arp"];
const SECURITY_SUBCMDS: &[&str] = &["mac", "caps", "audit"];
const MAC_SUBCMDS: &[&str] = &["status", "enable", "disable", "rules"];
const CAPS_SUBCMDS: &[&str] = &["list", "set"];
const AUDIT_SUBCMDS: &[&str] = &["status", "enable", "disable", "events", "stats"];

/// Known service names (for services status/start/stop).
const SERVICE_NAMES: &[&str] = &[
    "rs", "pm", "vfs", "vm", "ds", "sched", "auditd", "macd",
    "bluetoothd", "devman", "pci", "ahci", "e1000", "virtio_blk",
    "virtio_net", "input", "is", "tty", "log", "inetd",
];

// ===========================================================================
// Shell state
// ===========================================================================

/// Interactive shell state.
pub struct Shell {
    /// Current input line (bytes for Unicode support).
    line: Vec<u8>,
    /// Cursor position (byte offset into `line`).
    cursor: usize,
    /// Command history (newest first).
    history: Vec<String>,
    /// Current history browsing index (`None` = fresh line).
    history_idx: Option<usize>,
    /// Saved line when browsing history.
    saved_line: String,
    /// Terminal dimensions.
    rows: u16,
    cols: u16,
}

impl Shell {
    /// Create a new shell state.
    fn new(rows: u16, cols: u16) -> Self {
        Shell {
            line: Vec::new(),
            cursor: 0,
            history: Vec::with_capacity(MAX_HISTORY),
            history_idx: None,
            saved_line: String::new(),
            rows,
            cols,
        }
    }

    // =======================================================================
    // Core loop
    // =======================================================================

    /// Run the interactive shell. Returns when the user exits (Ctrl+D or `exit`).
    pub fn run() -> io::Result<()> {
        let mut term = Terminal::new()?;
        let (rows, cols) = term.size();
        let mut sh = Shell::new(rows, cols);

        // Welcome banner
        term.clear();
        sh.print_banner(&mut term);
        sh.print_help_hint(&mut term);

        loop {
            // Render prompt line
            sh.render(&mut term);

            // Read a key
            let key = term.read_key()?;

            // Handle the key
            let should_exit = sh.handle_key(&mut term, key)?;
            if should_exit {
                break;
            }
        }

        // Cleanup
        term.show_cursor();
        term.reset_style();

        Ok(())
    }

    /// Handle a single key press. Returns `true` if the shell should exit.
    fn handle_key(&mut self, term: &mut Terminal, key: Key) -> io::Result<bool> {
        match key {
            // ── Exit ──
            // Ctrl+D on empty line = exit (EOT = 0x04, mapped to Ctrl('d') by minix-term)
            Key::Ctrl('d') if self.line.is_empty() => {
                return Ok(true);
            }

            // ── Enter ──
            Key::Enter => {
                self.handle_enter(term)?;
            }

            // ── Backspace / Delete ──
            Key::Backspace => {
                if self.cursor > 0 {
                    self.line.remove(self.cursor - 1);
                    self.cursor -= 1;
                }
            }
            Key::Delete => {
                if self.cursor < self.line.len() {
                    self.line.remove(self.cursor);
                }
            }

            // ── Cursor movement ──
            Key::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            Key::Right => {
                if self.cursor < self.line.len() {
                    self.cursor += 1;
                }
            }
            Key::Home => {
                self.cursor = 0;
            }
            Key::End => {
                self.cursor = self.line.len();
            }

            // ── History ──
            Key::Up => {
                self.history_up();
            }
            Key::Down => {
                self.history_down();
            }

            // ── Tab completion ──
            Key::Tab => {
                self.tab_complete(term)?;
            }

            // ── Ctrl+C — cancel line ──
            Key::Ctrl('c') => {
                // Cancel current line, new prompt
                self.line.clear();
                self.cursor = 0;
                self.history_idx = None;
                // Print ^C and newline
                write!(term, "^C\r\n")?;
            }

            // ── Ctrl+L — clear screen ──
            Key::Ctrl('l') => {
                term.clear();
                self.print_banner(&mut *term);
            }

            // ── Printable characters ──
            Key::Char(ch) => {
                if ch.is_ascii() && (ch as u8) >= 0x20 && (ch as u8) <= 0x7e {
                    let byte = ch as u8;
                    self.line.insert(self.cursor, byte);
                    self.cursor += 1;
                }
            }

            // ── Ignore everything else ──
            _ => {}
        }

        Ok(false)
    }

    // =======================================================================
    // History navigation
    // =======================================================================

    fn history_up(&mut self) {
        if self.history.is_empty() {
            return;
        }

        // Save current line on first history navigation
        if self.history_idx.is_none() {
            self.saved_line = String::from_utf8_lossy(&self.line).to_string();
            self.history_idx = Some(0);
        } else {
            // Move to older entry
            let idx = self.history_idx.unwrap();
            if idx + 1 < self.history.len() {
                self.history_idx = Some(idx + 1);
            }
        }

        // Load history entry
        let idx = self.history_idx.unwrap();
        let entry = &self.history[idx];
        self.line = entry.as_bytes().to_vec();
        self.cursor = self.line.len();
    }

    fn history_down(&mut self) {
        if self.history_idx.is_none() {
            return;
        }

        let idx = self.history_idx.unwrap();
        if idx > 0 {
            self.history_idx = Some(idx - 1);
            let entry = &self.history[idx - 1];
            self.line = entry.as_bytes().to_vec();
            self.cursor = self.line.len();
        } else {
            // Back to saved line
            self.history_idx = None;
            self.line = self.saved_line.as_bytes().to_vec();
            self.cursor = self.line.len();
        }
    }

    // =======================================================================
    // Tab completion
    // =======================================================================

    fn tab_complete(&mut self, term: &mut Terminal) -> io::Result<()> {
        let line_str = String::from_utf8_lossy(&self.line);
        let line = line_str.trim_start();

        if line.is_empty() {
            // Nothing to complete — show all commands
            self.show_completions(term, COMMANDS)?;
            return Ok(());
        }

        let words: Vec<&str> = line.split_whitespace().collect();
        let last_word = words.last().copied().unwrap_or("");
        let is_first = words.len() == 1;

        if is_first {
            // Complete top-level command
            let completions: Vec<&str> = COMMANDS
                .iter()
                .filter(|c| c.starts_with(last_word))
                .copied()
                .collect();

            if completions.len() == 1 {
                // Single match — replace last word
                self.replace_last_word(&completions[0]);
            } else if !completions.is_empty() {
                self.show_completions(term, &completions)?;
            }
        } else if words.len() >= 2 {
            // Complete subcommand
            let cmd = words[0];
            let completions: Vec<&str> = match cmd {
                "services" => SERVICE_SUBCMDS
                    .iter()
                    .filter(|s| if words.len() == 2 { s.starts_with(last_word) } else { false })
                    .copied()
                    .collect(),
                "system" => SYSTEM_SUBCMDS
                    .iter()
                    .filter(|s| s.starts_with(last_word))
                    .copied()
                    .collect(),
                "network" | "net" => NETWORK_SUBCMDS
                    .iter()
                    .filter(|s| s.starts_with(last_word))
                    .copied()
                    .collect(),
                "security" | "sec" => SECURITY_SUBCMDS
                    .iter()
                    .filter(|s| s.starts_with(last_word))
                    .copied()
                    .collect(),
                _ => Vec::new(),
            };

            // If completing a service name (e.g., `services status <tab>`)
            if words.len() >= 3 {
                let cmd = words[0];
                let sub = words[1];
                let service_completions: Vec<&str> = match (cmd, sub) {
                    ("services", "status" | "start" | "stop" | "restart") => SERVICE_NAMES
                        .iter()
                        .filter(|s| s.starts_with(last_word))
                        .copied()
                        .collect(),
                    _ => Vec::new(),
                };

                if !service_completions.is_empty() {
                    if service_completions.len() == 1 {
                        self.replace_last_word(service_completions[0]);
                    } else {
                        self.show_completions(term, &service_completions)?;
                    }
                    return Ok(());
                }
            }

            // Security subcommands (mac/caps/audit)
            if words.len() == 2 && (cmd == "security" || cmd == "sec") {
                let sec_completions: Vec<&str> = SECURITY_SUBCMDS
                    .iter()
                    .filter(|s| s.starts_with(last_word))
                    .copied()
                    .collect();
                if sec_completions.len() == 1 {
                    self.replace_last_word(sec_completions[0]);
                } else if !sec_completions.is_empty() {
                    self.show_completions(term, &sec_completions)?;
                }
                return Ok(());
            }

            // Third-level subcommands: security mac <tab>, security caps <tab>, security audit <tab>
            if words.len() == 3 && cmd == "mac" {
                let mac_completions: Vec<&str> = MAC_SUBCMDS
                    .iter()
                    .filter(|s| s.starts_with(last_word))
                    .copied()
                    .collect();
                if mac_completions.len() == 1 {
                    self.replace_last_word(mac_completions[0]);
                } else if !mac_completions.is_empty() {
                    self.show_completions(term, &mac_completions)?;
                }
                return Ok(());
            }
            if words.len() == 3 && cmd == "caps" {
                let caps_completions: Vec<&str> = CAPS_SUBCMDS
                    .iter()
                    .filter(|s| s.starts_with(last_word))
                    .copied()
                    .collect();
                if caps_completions.len() == 1 {
                    self.replace_last_word(caps_completions[0]);
                } else if !caps_completions.is_empty() {
                    self.show_completions(term, &caps_completions)?;
                }
                return Ok(());
            }
            if words.len() == 3 && cmd == "audit" {
                let audit_completions: Vec<&str> = AUDIT_SUBCMDS
                    .iter()
                    .filter(|s| s.starts_with(last_word))
                    .copied()
                    .collect();
                if audit_completions.len() == 1 {
                    self.replace_last_word(audit_completions[0]);
                } else if !audit_completions.is_empty() {
                    self.show_completions(term, &audit_completions)?;
                }
                return Ok(());
            }

            if completions.len() == 1 {
                self.replace_last_word(&completions[0]);
            } else if !completions.is_empty() {
                self.show_completions(term, &completions)?;
            }
        }

        Ok(())
    }

    /// Replace the last word in the line with a completion.
    fn replace_last_word(&mut self, completion: &str) {
        let line_str = String::from_utf8_lossy(&self.line);
        let trimmed = line_str.trim_end();

        // Find the start of the last word
        if let Some(pos) = trimmed.rfind(|c: char| c.is_whitespace()) {
            // Keep everything before the last word, insert completion
            let prefix = &trimmed[..=pos];
            let mut new_line = prefix.to_string();
            new_line.push_str(completion);
            new_line.push(' ');
            self.line = new_line.into_bytes();
        } else {
            // No whitespace — the whole line is the last word
            let mut new_line = completion.to_string();
            new_line.push(' ');
            self.line = new_line.into_bytes();
        }
        self.cursor = self.line.len();
    }

    /// Show completions below the prompt line.
    fn show_completions(&self, term: &mut Terminal, completions: &[&str]) -> io::Result<()> {
        if completions.is_empty() {
            return Ok(());
        }

        // Save cursor, print completions, restore cursor
        write!(term, "\r\n")?;

        if completions.len() <= MAX_INLINE_COMPLETIONS {
            // Show all on one or two lines
            write!(term, "{}", C_DIM)?;
            for (i, c) in completions.iter().enumerate() {
                if i > 0 {
                    write!(term, "  ")?;
                }
                write!(term, "{}", c)?;
            }
            writeln!(term, "{}", C_RESET)?;
        } else {
            // Show with count
            writeln!(term, "{}", C_DIM)?;
            for c in completions {
                writeln!(term, "  {}", c)?;
            }
            writeln!(term, "({} completions){}", completions.len(), C_RESET)?;
        }

        // Restore cursor position (it will be re-rendered on next iteration)
        Ok(())
    }

    // =======================================================================
    // Command execution
    // =======================================================================

    /// Handle the Enter key — parse and execute the command.
    fn handle_enter(&mut self, term: &mut Terminal) -> io::Result<()> {
        let line_str = String::from_utf8_lossy(&self.line);
        let trimmed = line_str.trim();

        // Print newline after the prompt line
        writeln!(term)?;

        if trimmed.is_empty() {
            // Empty line — just new prompt
            return Ok(());
        }

        // Add to history
        let cmd_line = trimmed.to_string();
        if self.history.first().map_or(true, |h| *h != cmd_line) {
            self.history.insert(0, cmd_line.clone());
            if self.history.len() > MAX_HISTORY {
                self.history.pop();
            }
        }
        self.history_idx = None;

        // Parse args
        let args: Vec<&str> = trimmed.split_whitespace().collect();
        if args.is_empty() {
            return Ok(());
        }

        // Handle built-in shell commands
        match args[0] {
            "exit" | "quit" => {
                writeln!(term, "{}Goodbye!{}", C_GREEN, C_RESET)?;
                // Signal exit
                std::process::exit(0);
            }
            "clear" => {
                term.clear();
                self.print_banner(term);
                self.line.clear();
                self.cursor = 0;
                return Ok(());
            }
            "help" => {
                self.print_interactive_help(term)?;
                self.line.clear();
                self.cursor = 0;
                return Ok(());
            }
            "version" => {
                writeln!(term, "minix-admin v{}", env!("CARGO_PKG_VERSION"))?;
                self.line.clear();
                self.cursor = 0;
                return Ok(());
            }
            _ => {}
        }

        // Dispatch to existing CLI functions through argument Vec
        let string_args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        let result = dispatch_command(&string_args);

        // Display result
        match result {
            Ok(()) => {}
            Err(msg) => {
                writeln!(term, "{}Error: {}{}", C_RED, msg, C_RESET)?;
            }
        }

        // Clear line for next input
        self.line.clear();
        self.cursor = 0;

        Ok(())
    }

    // =======================================================================
    // Rendering
    // =======================================================================

    /// Render the prompt and the current input line.
    fn render(&self, term: &mut Terminal) {
        // Build the line string
        let line_str = String::from_utf8_lossy(&self.line);
        let display_line = &*line_str;

        // Move to start of prompt line
        write!(term, "\r").ok();

        // Clear the line (ANSI erase line)
        write!(term, "\x1B[K").ok();

        // Print prompt (colored)
        write!(
            term,
            "{}{}{}{}",
            C_BRIGHT_GREEN, PROMPT, C_BRIGHT_CYAN, " > "
        )
        .ok();

        // Print the current line content
        write!(term, "{}{}{}", C_WHITE, display_line, C_RESET).ok();

        // Position cursor at the right spot
        // Cursor position = prompt_len + cursor_position
        let prompt_len = PROMPT.len() + 3; // "admin@gergios > "
        let cursor_col = 1 + prompt_len + self.cursor;
        write!(term, "\x1B[{}G", cursor_col).ok();
        term.flush().ok();
    }

    /// Print the welcome banner.
    fn print_banner(&self, term: &mut Terminal) {
        let banner = format!(
            "\r\n{}╔══════════════════════════════════════╗{}\r\n\
             {}║     {}GergiOS Admin Shell v{}{}      ║{}\r\n\
             {}╚══════════════════════════════════════╝{}\r\n",
            C_CYAN, C_RESET,
            C_CYAN, C_BOLD, env!("CARGO_PKG_VERSION"), C_CYAN, C_RESET,
            C_CYAN, C_RESET,
        );
        write!(term, "{}", banner).ok();
    }

    /// Print a one-line hint.
    fn print_help_hint(&self, term: &mut Terminal) {
        writeln!(
            term,
            "{}Type 'help' for commands, Tab for completion, Ctrl+D to exit{}",
            C_DIM, C_RESET
        )
        .ok();
    }

    /// Print interactive help.
    fn print_interactive_help(&self, term: &mut Terminal) -> io::Result<()> {
        writeln!(term)?;
        writeln!(term, "{}Available Commands:{}", C_BOLD, C_RESET)?;
        writeln!(term)?;
        writeln!(term, "  {}services{}        Service management (list, status, start, stop, restart)", C_BRIGHT_CYAN, C_RESET)?;
        writeln!(term, "  {}system{}          System monitoring (info, cpu, memory, disk, uptime)", C_BRIGHT_CYAN, C_RESET)?;
        writeln!(term, "  {}network{}         Network management (interfaces, stats, route, arp)", C_BRIGHT_CYAN, C_RESET)?;
        writeln!(term, "  {}security{}        Security management (mac, caps, audit)", C_BRIGHT_CYAN, C_RESET)?;
        writeln!(term, "  {}help{}            Show this help message", C_BRIGHT_CYAN, C_RESET)?;
        writeln!(term, "  {}version{}         Show version", C_BRIGHT_CYAN, C_RESET)?;
        writeln!(term, "  {}clear{}           Clear screen", C_BRIGHT_CYAN, C_RESET)?;
        writeln!(term, "  {}exit / quit{}     Exit the shell", C_BRIGHT_CYAN, C_RESET)?;
        writeln!(term)?;
        writeln!(term, "{}Keyboard Shortcuts:{}", C_BOLD, C_RESET)?;
        writeln!(term)?;
        writeln!(term, "  {}Tab{}              Auto-complete commands and arguments", C_DIM, C_RESET)?;
        writeln!(term, "  {}Up/Down{}          Command history", C_DIM, C_RESET)?;
        writeln!(term, "  {}Left/Right{}       Move cursor within line", C_DIM, C_RESET)?;
        writeln!(term, "  {}Home/End{}         Jump to start/end of line", C_DIM, C_RESET)?;
        writeln!(term, "  {}Ctrl+C{}           Cancel current command", C_DIM, C_RESET)?;
        writeln!(term, "  {}Ctrl+L{}           Clear screen", C_DIM, C_RESET)?;
        writeln!(term, "  {}Ctrl+D{}           Exit shell (on empty line)", C_DIM, C_RESET)?;
        writeln!(term)?;
        writeln!(term, "{}Tip:{} Use Tab to discover available subcommands!", C_YELLOW, C_RESET)?;
        writeln!(term)?;
        // Run CLI help too
        cli::print_usage("minix-admin");
        Ok(())
    }
}

// ===========================================================================
// Command dispatcher
// ===========================================================================

/// Dispatch a command to the appropriate handler.
/// This mirrors the logic in `main.rs` but works with owned String args.
fn dispatch_command(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("No command specified".to_string());
    }

    let command = &args[0];
    let subargs = &args[1..];

    match command.as_str() {
        "services" => handle_services(subargs),
        "system" => handle_system(subargs),
        "network" | "net" => handle_network(subargs),
        "security" | "sec" => handle_security(subargs),
        _ => Err(format!("Unknown command '{}'. Try 'help'.", command)),
    }
}

fn handle_services(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return services::list_services();
    }
    match args[0].as_str() {
        "list" => services::list_services(),
        "status" => {
            if args.len() < 2 {
                services::list_services()
            } else {
                services::service_status(&args[1])
            }
        }
        "start" => {
            if args.len() < 2 {
                return Err("Usage: services start <name>".to_string());
            }
            services::start_service(&args[1])
        }
        "stop" => {
            if args.len() < 2 {
                return Err("Usage: services stop <name>".to_string());
            }
            services::stop_service(&args[1])
        }
        "restart" => {
            if args.len() < 2 {
                return Err("Usage: services restart <name>".to_string());
            }
            services::restart_service(&args[1])
        }
        _ => Err(format!("Unknown service command '{}'.", args[0])),
    }
}

fn handle_system(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return system::show_system_info();
    }
    match args[0].as_str() {
        "info" => system::show_system_info(),
        "cpu" => system::show_cpu(),
        "memory" | "mem" => system::show_memory(),
        "disk" => system::show_disk(),
        "uptime" => system::show_uptime(),
        _ => Err(format!("Unknown system command '{}'.", args[0])),
    }
}

fn handle_network(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return network::show_interfaces();
    }
    match args[0].as_str() {
        "interfaces" | "list" | "ls" => network::show_interfaces(),
        "status" | "iface" | "show" => {
            if args.len() < 2 {
                network::show_interfaces()
            } else {
                network::show_iface(&args[1])
            }
        }
        "stats" | "statistics" => network::show_stats(),
        "route" | "routes" | "routing" => network::show_route(),
        "arp" => network::show_arp(),
        _ => Err(format!("Unknown network command '{}'.", args[0])),
    }
}

fn handle_security(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        security::mac_status()?;
        println!();
        return security::audit_status();
    }
    match args[0].as_str() {
        "mac" => handle_security_mac(&args[1..]),
        "caps" | "capabilities" => handle_security_caps(&args[1..]),
        "audit" => handle_security_audit(&args[1..]),
        _ => Err(format!("Unknown security command '{}'.", args[0])),
    }
}

fn handle_security_mac(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return security::mac_status();
    }
    match args[0].as_str() {
        "status" | "show" => security::mac_status(),
        "enable" | "on" => security::mac_enable(),
        "disable" | "off" => security::mac_disable(),
        "rules" => security::mac_show_rules(),
        _ => Err(format!("Unknown mac command '{}'.", args[0])),
    }
}

fn handle_security_caps(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("Usage: caps list <pid> | caps set <pid> <capability>".to_string());
    }
    match args[0].as_str() {
        "list" | "show" => {
            if args.len() < 2 {
                return Err("Usage: caps list <pid>".to_string());
            }
            let pid: i32 = args[1]
                .parse()
                .map_err(|_| format!("Invalid pid '{}'", args[1]))?;
            security::caps_list(pid)
        }
        "set" => {
            if args.len() < 3 {
                return Err("Usage: caps set <pid> <capability>".to_string());
            }
            let pid: i32 = args[1]
                .parse()
                .map_err(|_| format!("Invalid pid '{}'", args[1]))?;
            security::caps_set(pid, &args[2])
        }
        _ => Err(format!("Unknown caps command '{}'.", args[0])),
    }
}

fn handle_security_audit(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return security::audit_status();
    }
    match args[0].as_str() {
        "status" | "show" => security::audit_status(),
        "enable" | "on" => security::audit_enable(),
        "disable" | "off" => security::audit_disable(),
        "events" | "logs" => {
            let limit = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(20);
            security::audit_events(limit)
        }
        "stats" | "statistics" => security::audit_stats(),
        _ => Err(format!("Unknown audit command '{}'.", args[0])),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_new() {
        let sh = Shell::new(24, 80);
        assert!(sh.line.is_empty());
        assert_eq!(sh.cursor, 0);
        assert!(sh.history.is_empty());
        assert_eq!(sh.rows, 24);
        assert_eq!(sh.cols, 80);
    }

    #[test]
    fn test_history_add_and_browse() {
        let mut sh = Shell::new(24, 80);

        // Simulate adding commands to history
        sh.history.insert(0, "services list".to_string());
        sh.history.insert(0, "system info".to_string());
        assert_eq!(sh.history.len(), 2);

        // Browse up (newest first)
        sh.history_up();
        assert_eq!(String::from_utf8_lossy(&sh.line), "system info");
        assert_eq!(sh.cursor, 11);

        sh.history_up();
        assert_eq!(String::from_utf8_lossy(&sh.line), "services list");
        assert_eq!(sh.cursor, 13);

        // Browse down
        sh.history_down();
        assert_eq!(String::from_utf8_lossy(&sh.line), "system info");

        sh.history_down();
        // Back to saved line (which was empty "" before first history_up)
        assert!(sh.line.is_empty());
        assert_eq!(sh.cursor, 0);
    }

    #[test]
    fn test_line_insert_and_delete() {
        let mut sh = Shell::new(24, 80);

        // Insert characters
        let test_bytes: Vec<u8> = b"test".to_vec();
        for &b in &test_bytes {
            sh.line.insert(sh.cursor, b);
            sh.cursor += 1;
        }
        assert_eq!(String::from_utf8_lossy(&sh.line), "test");
        assert_eq!(sh.cursor, 4);

        // Backspace
        sh.cursor = 2;
        sh.line.remove(sh.cursor - 1);
        sh.cursor -= 1;
        assert_eq!(String::from_utf8_lossy(&sh.line), "tst"); // removed 'e'

        // Delete
        sh.cursor = 1;
        sh.line.remove(sh.cursor);
        assert_eq!(String::from_utf8_lossy(&sh.line), "tt"); // removed 's'
    }

    #[test]
    fn test_replace_last_word() {
        let mut sh = Shell::new(24, 80);

        // With space-separated words
        sh.line = b"services star".to_vec();
        sh.cursor = 14;
        sh.replace_last_word("start");
        assert_eq!(String::from_utf8_lossy(&sh.line), "services start ");
        // "services " (9) + "start" (5) + " " (1) = 15 bytes
        assert_eq!(sh.cursor, 15);

        // Single word
        let mut sh = Shell::new(24, 80);
        sh.line = b"net".to_vec();
        sh.cursor = 3;
        sh.replace_last_word("network");
        assert_eq!(String::from_utf8_lossy(&sh.line), "network ");
        assert_eq!(sh.cursor, 8);
    }

    #[test]
    fn test_command_list() {
        assert!(COMMANDS.contains(&"services"));
        assert!(COMMANDS.contains(&"system"));
        assert!(COMMANDS.contains(&"network"));
        assert!(COMMANDS.contains(&"security"));
        assert!(COMMANDS.contains(&"help"));
        assert!(COMMANDS.contains(&"exit"));
        assert!(COMMANDS.contains(&"clear"));
    }

    #[test]
    fn test_dispatch_unknown() {
        let args = vec!["nonexistent".to_string()];
        let result = dispatch_command(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown command"));
    }

    #[test]
    fn test_dispatch_services_empty() {
        let args = vec!["services".to_string(), "list".to_string()];
        assert!(dispatch_command(&args).is_ok());
    }

    #[test]
    fn test_dispatch_system_empty() {
        let args = vec!["system".to_string(), "info".to_string()];
        assert!(dispatch_command(&args).is_ok());
    }

    #[test]
    fn test_dispatch_network_empty() {
        let args = vec!["network".to_string(), "interfaces".to_string()];
        assert!(dispatch_command(&args).is_ok());
    }

    #[test]
    fn test_dispatch_security_empty() {
        let args = vec!["security".to_string(), "mac".to_string(), "status".to_string()];
        assert!(dispatch_command(&args).is_ok());
    }

    #[test]
    fn test_tab_complete_top_level() {
        let mut sh = Shell::new(24, 80);

        // 'ser' should complete to 'services '
        sh.line = b"ser".to_vec();
        sh.cursor = 3;
        sh.replace_last_word("services");
        assert_eq!(String::from_utf8_lossy(&sh.line), "services ");
    }
}
