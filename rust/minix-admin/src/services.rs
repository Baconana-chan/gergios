//! # Service Manager
//!
//! Manages MINIX system services via IPC with the Reincarnation Server (RS).
//!
//! ## Architecture
//!
//! ```text
//! minix-admin → [sendrec(RS, RS_UP/RS_DOWN/RS_GETSYSINFO)] → RS → service
//!           ← [reply: endpoint, pid, status] ←
//! ```
//!
//! Services are identified by their registered label in the Data Store (DS).
//! RS manages the service lifecycle: start (`RS_UP`), stop (`RS_DOWN`),
//! and status inquiry (`RS_GETSYSINFO`).

use std::fmt;

/// Service state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(dead_code)]
pub enum ServiceState {
    Running,
    Stopped,
    Crashed,    // used in future real IPC
    Unknown,
}

impl fmt::Display for ServiceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Stopped => write!(f, "stopped"),
            Self::Crashed => write!(f, "crashed"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// A system service entry.
#[derive(Clone, Debug)]
pub struct ServiceInfo {
    /// Service name (label).
    pub name: String,
    /// Current state.
    pub state: ServiceState,
    /// Endpoint / process number.
    #[allow(dead_code)]
    pub endpoint: i32,
    /// PID (if running).
    pub pid: i32,
    /// Time since last state change (seconds).
    pub uptime_secs: u64,
}

/// Known system services and their labels.
pub(crate) const KNOWN_SERVICES: &[&str] = &[
    "rs",        // Reincarnation Server
    "pm",        // Process Manager
    "vfs",       // Virtual File System
    "vm",        // Virtual Memory
    "ds",        // Data Store
    "sched",     // Scheduler
    "auditd",    // Audit Daemon
    "macd",      // MAC Policy Daemon
    "bluetoothd", // Bluetooth Daemon
    "devman",    // Device Manager
    "pci",       // PCI config
    "ahci",      // AHCI SATA
    "e1000",     // Intel e1000 NIC
    "virtio_blk",// VirtIO Block
    "virtio_net",// VirtIO Net
    "input",     // Input Server
    "is",        // Input System
    "tty",       // TTY driver
    "log",       // System log daemon
    "inetd",     // Internet super-server
];

/// List all system services and their status.
pub fn list_services() -> Result<(), String> {
    println!("{:<16} {:<12} {:>6} {:>8} {}",
        "SERVICE", "STATE", "ENDPT", "PID", "UPTIME");
    println!("{}", "-".repeat(60));

    for &name in KNOWN_SERVICES {
        let info = query_service_status(name);
        let state_icon = match info.state {
            ServiceState::Running => "▲",
            ServiceState::Stopped => "▼",
            ServiceState::Crashed => "✗",
            ServiceState::Unknown => "?",
        };
        let pid_str = if info.pid > 0 {
            format!("{}", info.pid)
        } else {
            "-".to_string()
        };
        let uptime_str = if info.uptime_secs > 0 {
            format_uptime(info.uptime_secs)
        } else {
            "-".to_string()
        };

        println!("{:<2} {:<14} {:<12} {:>6} {:>8} {}",
            state_icon,
            name,
            format!("{}", info.state),
            info.endpoint,
            pid_str,
            uptime_str,
        );
    }

    Ok(())
}

/// Show detailed status for a single service.
pub fn service_status(name: &str) -> Result<(), String> {
    let info = query_service_status(name);

    println!("Service:     {}", name);
    println!("State:       {} ({})", info.state, match info.state {
        ServiceState::Running => "▲",
        ServiceState::Stopped => "▼",
        ServiceState::Crashed => "✗",
        ServiceState::Unknown => "?",
    });
    println!("Endpoint:    {}", info.endpoint);
    println!("PID:         {}", if info.pid > 0 { format!("{}", info.pid) } else { "-".to_string() });
    println!("Uptime:      {}", if info.uptime_secs > 0 { format_uptime(info.uptime_secs) } else { "-".to_string() });

    Ok(())
}

/// Start a service via RS_UP.
pub fn start_service(name: &str) -> Result<(), String> {
    println!("Starting '{}'... ", name);
    // TODO: implement RS_UP IPC
    // For now, simulate
    println!("OK (not yet implemented — will use RS_UP IPC)");
    Ok(())
}

/// Stop a service via RS_DOWN.
pub fn stop_service(name: &str) -> Result<(), String> {
    println!("Stopping '{}'... ", name);
    // TODO: implement RS_DOWN IPC
    println!("OK (not yet implemented — will use RS_DOWN IPC)");
    Ok(())
}

/// Restart a service (stop + start).
pub fn restart_service(name: &str) -> Result<(), String> {
    println!("Restarting '{}'... ", name);
    stop_service(name)?;
    start_service(name)?;
    println!("Done.");
    Ok(())
}

/// Query service status from the kernel/RS.
///
/// Uses `ds_retrieve_label_endpt()` to find the service endpoint,
/// then `RS_GETSYSINFO` or `/proc/<pid>/psinfo` for details.
///
/// For now, returns simulated data as a stub.
pub(crate) fn query_service_status(name: &str) -> ServiceInfo {
    // Stub implementation — will be replaced with real IPC
    // On MINIX, this would:
    // 1. ds_retrieve_label_endpt(name, &ep) — find service
    // 2. getsysinfo(ep, ...) — get process info
    // 3. read /proc/<ep>/psinfo — get uptime, state

    if name == "rs" || name == "pm" || name == "vfs" || name == "vm"
        || name == "ds" || name == "sched" || name == "auditd"
        || name == "macd" || name == "input" || name == "tty"
        || name == "pci" || name == "devman"
    {
        ServiceInfo {
            name: name.to_string(),
            state: ServiceState::Running,
            endpoint: 0,  // filled by lookup
            pid: 100,     // placeholder
            uptime_secs: 86400,
        }
    } else if name == "bluetoothd" {
        ServiceInfo {
            name: name.to_string(),
            state: ServiceState::Stopped,
            endpoint: -1,
            pid: 0,
            uptime_secs: 0,
        }
    } else {
        ServiceInfo {
            name: name.to_string(),
            state: ServiceState::Unknown,
            endpoint: -1,
            pid: 0,
            uptime_secs: 0,
        }
    }
}

/// Format seconds as a human-readable string.
fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let minutes = (secs % 3600) / 60;
    let secs = secs % 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_state_display() {
        assert_eq!(format!("{}", ServiceState::Running), "running");
        assert_eq!(format!("{}", ServiceState::Stopped), "stopped");
        assert_eq!(format!("{}", ServiceState::Crashed), "crashed");
    }

    #[test]
    fn test_format_uptime() {
        assert_eq!(format_uptime(30), "30s");
        assert_eq!(format_uptime(120), "2m 0s");
        assert_eq!(format_uptime(3661), "1h 1m");
        assert_eq!(format_uptime(90061), "1d 1h 1m");
    }

    #[test]
    fn test_query_known_service() {
        let info = query_service_status("vfs");
        assert_eq!(info.state, ServiceState::Running);
        assert_eq!(info.name, "vfs");
        assert!(info.pid > 0);
    }

    #[test]
    fn test_query_unknown_service() {
        let info = query_service_status("nonexistent");
        assert_eq!(info.state, ServiceState::Unknown);
    }

    #[test]
    fn test_list_services_doesnt_panic() {
        // Just verify the function runs without error
        assert!(list_services().is_ok());
    }

    #[test]
    fn test_service_status_known() {
        assert!(service_status("vfs").is_ok());
        assert!(service_status("bluetoothd").is_ok());
    }

    #[test]
    fn test_format_uptime_zero() {
        assert_eq!(format_uptime(0), "0s");
    }

    #[test]
    fn test_format_uptime_large() {
        let result = format_uptime(86400 * 30 + 3600 * 5 + 60 * 30 + 15);
        assert!(result.contains("d"));
    }

    #[test]
    fn test_query_all_known_services() {
        for &name in KNOWN_SERVICES {
            let info = query_service_status(name);
            assert_eq!(info.name, name);
            // Every known service should have a valid state
            assert!(
                info.state == ServiceState::Running
                || info.state == ServiceState::Stopped
                || info.state == ServiceState::Unknown
            );
        }
    }

    #[test]
    fn test_service_status_case_sensitive() {
        // Service names are case-sensitive
        let info = query_service_status("VFS");
        assert_eq!(info.state, ServiceState::Unknown);
    }

    #[test]
    fn test_list_services_empty() {
        // list_services should handle being called once (not crash)
        assert!(list_services().is_ok());
    }

    #[test]
    fn test_start_stop_restart_dont_panic() {
        assert!(start_service("test").is_ok());
        assert!(stop_service("test").is_ok());
        assert!(restart_service("test").is_ok());
    }

    #[test]
    fn test_known_services_list() {
        assert!(KNOWN_SERVICES.contains(&"rs"));
        assert!(KNOWN_SERVICES.contains(&"pm"));
        assert!(KNOWN_SERVICES.contains(&"vfs"));
        assert!(KNOWN_SERVICES.contains(&"auditd"));
        assert!(KNOWN_SERVICES.contains(&"bluetoothd"));
        assert_eq!(KNOWN_SERVICES.len(), 20);
    }
}
