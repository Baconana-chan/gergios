//! # TUI Dashboard
//!
//! Real-time monitoring dashboard for GergiOS.
//! Shows services, system stats, network, and security in a split-panel layout.
//! Auto-refreshes every 2 seconds.
//!
//! ## Key bindings
//!
//! - `q` / `Esc` — quit dashboard
//! - `r` — force refresh
//!
//! ## Layout
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │               GergiOS Dashboard  (refreshing in 2s)         │
//! ├──────────────┬──────────────────┬───────────────────────────┤
//! │  Services    │     System       │    Network + Security     │
//! │  ▲ rs  run   │  CPU: 24% ████  │  eth0  ▲ 1.2 MB/s        │
//! │  ▲ pm  run   │  Mem: 512M/2G   │  lo0   ▲ 0 B/s           │
//! │  ...         │  Disk: 2G/8G    │  MAC: ▲ ENFORCING         │
//! └──────────────┴──────────────────┴───────────────────────────┘
//! q=quit  r=refresh
//! ```

use crate::{network, security, services, system};
use minix_term::{Key, Terminal};
use std::io::{self, Write};
use std::time::Duration;

// ===========================================================================
// ANSI color constants
// ===========================================================================

const C_RESET: &str = "\x1B[0m";
const C_BOLD: &str = "\x1B[1m";
const C_DIM: &str = "\x1B[2m";
const C_RED: &str = "\x1B[38;5;1m";
const C_GREEN: &str = "\x1B[38;5;2m";
const C_YELLOW: &str = "\x1B[38;5;3m";
const C_CYAN: &str = "\x1B[38;5;6m";
const C_WHITE: &str = "\x1B[38;5;7m";
const C_BRIGHT_GREEN: &str = "\x1B[38;5;10m";
const C_BRIGHT_YELLOW: &str = "\x1B[38;5;11m";
const C_BRIGHT_RED: &str = "\x1B[38;5;9m";
const C_BRIGHT_CYAN: &str = "\x1B[38;5;14m";
const C_BG_BLUE: &str = "\x1B[48;5;17m";
const C_BG_DARK: &str = "\x1B[48;5;235m";

// ===========================================================================
// Dashboard data snapshot
// ===========================================================================

/// All data collected for one dashboard refresh cycle.
struct DashboardData {
    // Services
    services: Vec<services::ServiceInfo>,
    services_running: u32,
    services_stopped: u32,
    // System
    cpu_pct: u32,
    num_cores: u32,
    mem_total_mb: u64,
    mem_used_mb: u64,
    mem_pct: u32,
    disk_total_mb: u64,
    disk_used_mb: u64,
    disk_pct: u32,
    uptime_secs: u64,
    hostname: String,
    // Network
    iface_count: u32,
    iface_up: u32,
    net_rx_bytes: u64,
    net_tx_bytes: u64,
    // Security
    mac_state: String,
    mac_rules: u32,
    mac_violations: u64,
    audit_enabled: bool,
    audit_events: u64,

}

impl DashboardData {
    /// Collect a fresh snapshot from all system modules.
    fn collect() -> Self {
        // Services
        let services_list: Vec<services::ServiceInfo> = services::KNOWN_SERVICES
            .iter()
            .map(|name| services::query_service_status(name))
            .collect();
        let running = services_list.iter().filter(|s| s.state == services::ServiceState::Running).count() as u32;
        let stopped = services_list.iter().filter(|s| s.state == services::ServiceState::Stopped).count() as u32;

        // System
        let cpu_pct = system::get_loadavg().unwrap_or(0.0) as u32;
        let num_cores = system::num_cpus();
        let mem = system::get_memory_info().unwrap_or(system::MemoryInfo { total_kb: 0, used_kb: 0, free_kb: 0 });
        let mem_total_mb = mem.total_kb / 1024;
        let mem_used_mb = mem.used_kb / 1024;
        let mem_pct = if mem.total_kb > 0 { (mem.used_kb as f64 / mem.total_kb as f64 * 100.0) as u32 } else { 0 };
        let uptime_secs = system::get_uptime_secs().unwrap_or(0);
        let hostname = system::get_hostname().unwrap_or_else(|| "gergios".to_string());

        // Disk (root)
        let disk = system::get_disk_usage("/").unwrap_or(system::DiskEntry {
            path: "/".into(), total_kb: 0, used_kb: 0, free_kb: 0, label: "root".into(),
        });
        let disk_total_mb = disk.total_kb / 1024;
        let disk_used_mb = disk.used_kb / 1024;
        let disk_pct = if disk.total_kb > 0 { (disk.used_kb as f64 / disk.total_kb as f64 * 100.0) as u32 } else { 0 };

        // Network
        let mut iface_up = 0u32;
        let mut net_rx = 0u64;
        let mut net_tx = 0u64;
        for &(name, _) in network::KNOWN_INTERFACES {
            let info = network::query_interface(name);
            if info.is_up && info.is_running {
                iface_up += 1;
            }
            let stats = network::query_stats(name);
            net_rx += stats.rx_bytes;
            net_tx += stats.tx_bytes;
        }

        // Security
        let mac = security::query_mac_status();
        let audit = security::query_audit_status();
        let mac_state_str = match mac.state {
            security::MacState::Enforcing => "ENFORCING".to_string(),
            security::MacState::Permissive => "permissive".to_string(),
            security::MacState::Disabled => "disabled".to_string(),
        };

        DashboardData {
            services: services_list,
            services_running: running,
            services_stopped: stopped,
            cpu_pct,
            num_cores,
            mem_total_mb,
            mem_used_mb,
            mem_pct,
            disk_total_mb,
            disk_used_mb,
            disk_pct,
            uptime_secs,
            hostname,
            iface_count: network::KNOWN_INTERFACES.len() as u32,
            iface_up,
            net_rx_bytes: net_rx,
            net_tx_bytes: net_tx,
            mac_state: mac_state_str,
            mac_rules: mac.rules_loaded,
            mac_violations: mac.violations,
            audit_enabled: audit.enabled,
            audit_events: audit.total_events,
        }
    }
}

// ===========================================================================
// Dashboard
// ===========================================================================

/// The real-time TUI dashboard.
pub struct Dashboard {
    /// Cached data from last refresh.
    data: DashboardData,
    /// Terminal dimensions.
    cols: u16,
    rows: u16,
    /// Seconds until next auto-refresh.
    refresh_countdown: u32,

}

impl Dashboard {
    /// Run the dashboard. Returns when the user presses `q` or `Esc`.
    pub fn run() -> io::Result<()> {
        let mut term = Terminal::new()?;
        let (rows, cols) = term.size();
        let mut dash = Dashboard {
            data: DashboardData::collect(),
            cols,
            rows,
            refresh_countdown: 2,

        };

        term.hide_cursor();
        term.clear();

        loop {
            // Collect fresh data
            dash.data = DashboardData::collect();


            // Render the full dashboard
            dash.render(&mut term);

            // Wait up to 2 seconds for a key press (poll every 100ms)
            for tick in 0..20 {
                dash.refresh_countdown = (20u32 - tick).saturating_div(10).saturating_add(1);
                dash.render_status_bar(&mut term);

                if term.poll_key(Duration::from_millis(100))? {
                    match term.read_key()? {
                        Key::Char('q') | Key::Esc => {
                            // Cleanup
                            term.show_cursor();
                            term.reset_style();
                            term.clear();
                            writeln!(term, "Dashboard closed.")?;
                            return Ok(());
                        }
                        Key::Char('r') | Key::Char('R') => {
                            dash.refresh_countdown = 0;
                            break; // force refresh
                        }
                        Key::Char('?') => {
                            dash.render_help(&mut term)?;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    /// Render the full dashboard.
    fn render(&self, term: &mut Terminal) {
        // Move to home position
        write!(term, "\x1B[H").ok();

        // Header
        self.render_header(term);
        writeln!(term).ok();

        // Panels row
        // Calculate column widths (80-column optimized)
        let col1_w = 24u16.min(self.cols.saturating_sub(2) / 3);
        let col2_w = 22u16.min(self.cols.saturating_sub(2) / 3);
        let col3_w = self.cols.saturating_sub(2).saturating_sub(col1_w).saturating_sub(col2_w);

        self.render_services_panel(term, col1_w);
        write!(term, " ").ok();
        self.render_system_panel(term, col2_w);
        write!(term, " ").ok();
        self.render_network_security_panel(term, col3_w);
        writeln!(term).ok();

        // Status bar
        self.render_status_bar(term);

        // Flush all output
        term.flush().ok();
    }

    /// Render the header bar.
    fn render_header(&self, term: &mut Terminal) {
        let title = format!("GergiOS Dashboard v{}", env!("CARGO_PKG_VERSION"));
        let spacer = " ".repeat(
            self.cols.saturating_sub(title.len() as u16 + 4) as usize / 2
        );

        write!(term, "{}{} ", C_BG_BLUE, C_BOLD).ok();
        write!(term, "{}{}{}", spacer, title, C_RESET).ok();

        // Fill remaining with background
        let fill = " ".repeat(
            self.cols.saturating_sub(title.len() as u16 + 4) as usize
                - spacer.len()
        );
        write!(term, "{}{}{}", C_BG_BLUE, fill, C_RESET).ok();
        writeln!(term).ok();

        // Sub-header: hostname + uptime
        write!(term, "{}{}", C_DIM, C_BG_DARK).ok();
        let sub = format!(
            "  {}  |  Up: {}  |  {} cores",
            self.data.hostname,
            self.format_duration_short(self.data.uptime_secs),
            self.data.num_cores,
        );
        write!(term, "{:<width$}", sub, width = self.cols as usize).ok();
        writeln!(term, "{}{}", C_RESET, C_RESET).ok();
    }

    /// Render the services panel.
    fn render_services_panel(&self, term: &mut Terminal, width: u16) {
        let w = width as usize;
        write!(term, "{}", C_BOLD).ok();
        write!(term, "{:<width$}", " Services", width = w).ok();
        writeln!(term, "{}", C_RESET).ok();

        let divider = "─".repeat(w.saturating_sub(1));
        write!(term, " {}{}{}", C_DIM, divider, C_RESET).ok();
        writeln!(term).ok();

        // Show first (rows - 4) services
        let max_services = (self.rows.saturating_sub(6)) as usize;
        for svc in self.data.services.iter().take(max_services) {
            let (icon, color) = match svc.state {
                services::ServiceState::Running => ("▲", C_GREEN),
                services::ServiceState::Stopped => ("▼", C_RED),
                services::ServiceState::Crashed => ("✗", C_BRIGHT_RED),
                services::ServiceState::Unknown => ("?", C_YELLOW),
            };

            let name = &svc.name;
            let max_name_w = 12usize.min(w.saturating_sub(10) as usize);
            let display_name = if name.len() > max_name_w {
                format!("{:.width$}", name, width = max_name_w)
            } else {
                format!("{:<width$}", name, width = max_name_w)
            };

            write!(term, " {}{} {}{}", color, icon, C_RESET, display_name).ok();
            writeln!(term).ok();
        }
    }

    /// Render the system panel.
    fn render_system_panel(&self, term: &mut Terminal, width: u16) {
        let w = width as usize;
        write!(term, "{}", C_BOLD).ok();
        write!(term, "{:<width$}", " System", width = w).ok();
        writeln!(term, "{}", C_RESET).ok();

        let divider = "─".repeat(w.saturating_sub(1));
        write!(term, " {}{}{}", C_DIM, divider, C_RESET).ok();
        writeln!(term).ok();

        // CPU
        write!(term, " CPU: ").ok();
        self.render_progress_bar_inline(term, self.data.cpu_pct, w.saturating_sub(8) as usize);
        writeln!(term, " {}%", self.data.cpu_pct).ok();

        // Memory
        write!(term, " Mem: ").ok();
        self.render_progress_bar_inline(term, self.data.mem_pct, w.saturating_sub(8) as usize);
        writeln!(term, " {}/{}MB", self.data.mem_used_mb, self.data.mem_total_mb).ok();

        // Disk
        write!(term, " Disk: ").ok();
        self.render_progress_bar_inline(term, self.data.disk_pct, w.saturating_sub(8) as usize);
        writeln!(term, " {}/{}MB", self.data.disk_used_mb, self.data.disk_total_mb).ok();

        // Uptime
        writeln!(term, " Up:   {}", self.format_duration_long(self.data.uptime_secs)).ok();
    }

    /// Render the network + security panel.
    fn render_network_security_panel(&self, term: &mut Terminal, width: u16) {
        let w = width as usize;
        write!(term, "{}", C_BOLD).ok();
        write!(term, "{:<width$}", " Network", width = w).ok();
        writeln!(term, "{}", C_RESET).ok();

        let divider = "─".repeat(w.saturating_sub(1));
        write!(term, " {}{}{}", C_DIM, divider, C_RESET).ok();
        writeln!(term).ok();

        // Interfaces
        write!(term, " Ifaces: {}/{} up", self.data.iface_up, self.data.iface_count).ok();
        writeln!(term).ok();

        // RX/TX (simplified — raw bytes since we don't have deltas yet)
        write!(term, " RX: {}  ", self.format_bytes(self.data.net_rx_bytes)).ok();
        writeln!(term).ok();
        write!(term, " TX: {}  ", self.format_bytes(self.data.net_tx_bytes)).ok();
        writeln!(term).ok();

        // Security section
        writeln!(term).ok();
        write!(term, "{}", C_BOLD).ok();
        write!(term, "{:<width$}", " Security", width = w).ok();
        writeln!(term, "{}", C_RESET).ok();

        let divider = "─".repeat(w.saturating_sub(1));
        write!(term, " {}{}{}", C_DIM, divider, C_RESET).ok();
        writeln!(term).ok();

        // MAC
        let mac_color = if self.data.mac_state == "ENFORCING" { C_GREEN } else { C_YELLOW };
        write!(term, " MAC: {}{}{}", mac_color, self.data.mac_state, C_RESET).ok();
        writeln!(term).ok();
        writeln!(term, " Rules: {} loaded", self.data.mac_rules).ok();
        writeln!(term, " Violations: {}", self.data.mac_violations).ok();

        // Audit
        writeln!(term).ok();
        let audit_icon = if self.data.audit_enabled { "✓ enabled" } else { "✗ disabled" };
        let audit_color = if self.data.audit_enabled { C_GREEN } else { C_RED };
        writeln!(term, " Audit: {}{}{}", audit_color, audit_icon, C_RESET).ok();
        writeln!(term, " Events: {}", self.format_count(self.data.audit_events)).ok();
    }

    /// Render the bottom status bar.
    fn render_status_bar(&self, term: &mut Terminal) {
        // Move to last line
        write!(term, "\x1B[{};1H", self.rows).ok();

        let status = format!(
            "  q=quit  r=refresh  refresh in {}s  |  {} services ({}▲ {}▼)",
            self.refresh_countdown,
            self.data.services.len(),
            self.data.services_running,
            self.data.services_stopped,
        );

        let fill_width = self.cols as usize;
        write!(term, "{}{} {:<width$} {}", C_BG_DARK, C_DIM, status, C_RESET, width = fill_width).ok();
    }

    /// Render the help overlay.
    fn render_help(&self, term: &mut Terminal) -> io::Result<()> {
        // Save cursor, clear screen, show help
        write!(term, "\x1B[s").ok();
        term.clear();

        writeln!(term, "{}GergiOS Dashboard Help{}", C_BOLD, C_RESET)?;
        writeln!(term)?;
        writeln!(term, "  {}q{} / {}Esc{}    — Quit dashboard", C_BRIGHT_CYAN, C_RESET, C_BRIGHT_CYAN, C_RESET)?;
        writeln!(term, "  {}r{}         — Force refresh", C_BRIGHT_CYAN, C_RESET)?;
        writeln!(term, "  {}?{}         — Show this help", C_BRIGHT_CYAN, C_RESET)?;
        writeln!(term)?;
        writeln!(term, "{}Data Sources:{}", C_BOLD, C_RESET)?;
        writeln!(term, "  Services — RS (Reincarnation Server) IPC")?;
        writeln!(term, "  System   — procfs, sysctl, svrctl")?;
        writeln!(term, "  Network  — ioctl(SIOCGIF*), getifaddrs()")?;
        writeln!(term, "  Security — IPC with macd, auditd")?;
        writeln!(term)?;
        writeln!(term, "Press any key to return to dashboard...")?;

        term.read_key()?;
        write!(term, "\x1B[u").ok();
        Ok(())
    }

    // =======================================================================
    // Display helpers
    // =======================================================================

    /// Render a small progress bar inline (e.g., "████░░░░ 50%").
    fn render_progress_bar_inline(&self, term: &mut Terminal, pct: u32, width: usize) {
        let bar_width = width.saturating_sub(2).min(10).max(3);
        let filled = ((pct as f64 / 100.0) * bar_width as f64) as usize;
        let filled = filled.min(bar_width);
        let empty = bar_width.saturating_sub(filled);

        let color = if pct >= 90 {
            C_BRIGHT_RED
        } else if pct >= 70 {
            C_YELLOW
        } else {
            C_GREEN
        };

        write!(term, "{}", color).ok();
        for _ in 0..filled {
            write!(term, "█").ok();
        }
        write!(term, "{}", C_DIM).ok();
        for _ in 0..empty {
            write!(term, "░").ok();
        }
        write!(term, "{}", C_RESET).ok();
    }

    /// Format byte count for display.
    fn format_bytes(&self, bytes: u64) -> String {
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if bytes >= GB {
            format!("{:.1} GB", bytes as f64 / GB as f64)
        } else if bytes >= MB {
            format!("{:.1} MB", bytes as f64 / MB as f64)
        } else if bytes >= KB {
            format!("{} KB", bytes / KB)
        } else {
            format!("{} B", bytes)
        }
    }

    /// Format count with K/M suffix.
    fn format_count(&self, n: u64) -> String {
        if n >= 1_000_000 {
            format!("{:.1}M", n as f64 / 1_000_000.0)
        } else if n >= 1_000 {
            format!("{:.1}K", n as f64 / 1_000.0)
        } else {
            format!("{}", n)
        }
    }

    /// Short duration (for status bar).
    fn format_duration_short(&self, secs: u64) -> String {
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        let minutes = (secs % 3600) / 60;
        if days > 0 {
            format!("{}d {}h {}m", days, hours, minutes)
        } else if hours > 0 {
            format!("{}h {}m", hours, minutes)
        } else {
            format!("{}m {}s", minutes, secs % 60)
        }
    }

    /// Long duration (for system panel).
    fn format_duration_long(&self, secs: u64) -> String {
        let days = secs / 86400;
        let hours = (secs % 86400) / 3600;
        let minutes = (secs % 3600) / 60;
        let secs = secs % 60;
        if days > 0 {
            format!("{}d {:02}:{:02}:{:02}", days, hours, minutes, secs)
        } else {
            format!("{:02}:{:02}:{:02}", hours, minutes, secs)
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_data_collect() {
        let data = DashboardData::collect();
        assert!(!data.services.is_empty());
        assert!(data.num_cores > 0);
        assert!(!data.hostname.is_empty());
        assert!(!data.mac_state.is_empty());
    }

    #[test]
    fn test_format_bytes() {
        let d = Dashboard { data: DashboardData::collect(), cols: 80, rows: 24, refresh_countdown: 0 };
        assert_eq!(d.format_bytes(0), "0 B");
        assert!(d.format_bytes(1024).contains("KB") || d.format_bytes(1024).contains("1.0"));
        assert!(d.format_bytes(1_048_576).contains("MB"));
        assert!(d.format_bytes(1_073_741_824).contains("GB"));
    }

    #[test]
    fn test_format_count() {
        let d = Dashboard { data: DashboardData::collect(), cols: 80, rows: 24, refresh_countdown: 0 };
        assert!(d.format_count(500).contains("500"));
        assert!(d.format_count(1_500).contains("K"));
        assert!(d.format_count(2_500_000).contains("M"));
    }

    #[test]
    fn test_format_duration() {
        let d = Dashboard { data: DashboardData::collect(), cols: 80, rows: 24, refresh_countdown: 0 };
        assert_eq!(d.format_duration_short(0), "0m 0s");
        assert!(d.format_duration_short(3661).contains("1h"));
        assert!(d.format_duration_short(90061).contains("d"));

        assert!(d.format_duration_long(3661).contains("01:01:01"));
        assert!(d.format_duration_long(90061).contains("d"));
    }

    #[test]
    fn test_services_count() {
        let data = DashboardData::collect();
        let total = data.services_running + data.services_stopped;
        assert!(data.services.len() as u32 >= total);
    }

    #[test]
    fn test_memory_sanity() {
        let data = DashboardData::collect();
        if data.mem_total_mb > 0 {
            assert!(data.mem_used_mb <= data.mem_total_mb);
            assert!(data.mem_pct <= 100);
        }
    }
}
