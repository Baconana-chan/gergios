//! # Security Manager
//!
//! Manages security subsystems: MAC (Mandatory Access Control),
//! process capabilities, and audit logging.
//!
//! ## Architecture
//!
//! On GergiOS/MINIX, security is managed through three interfaces:
//!
//! ### MAC (Mandatory Access Control) — macd daemon
//! ```text
//! minix-admin → ds_retrieve_label_endpt("macd") → get macd endpoint
//!            → sendrec(macd_ep, &m) with:
//!              m.m_type = MACD_RQ_STATUS      — get enforcement status
//!              m.m_type = MACD_RQ_ENABLE      — enable enforcement
//!              m.m_type = MACD_RQ_DISABLE     — disable enforcement
//! ```
//!
//! ### Audit — auditd daemon
//! ```text
//! minix-admin → ds_retrieve_label_endpt("auditd") → get auditd endpoint
//!            → sendrec(auditd_ep, &m) with:
//!              m.m_type = AUDITD_RQ_STATUS     — get daemon status
//!              m.m_type = AUDITD_RQ_ENABLE     — enable logging
//!              m.m_type = AUDITD_RQ_DISABLE    — disable logging
//!              m.m_type = AUDITD_RQ_POLL_NOW   — force immediate poll
//! ```
//!
//! ### Capabilities — SYS_CAPCTL kernel call
//! ```text
//! minix-admin → _kernel_call(SYS_CAPCTL, &m) with:
//!              CAPCTL_GET_CAPS    — read process capabilities
//!              CAPCTL_SET_CAPS    — set capabilities (privileged)
//!              CAPCTL_GET_BOUND   — get bounding set
//! ```

use std::fmt;

// ============================================================================
// Data types — MAC
// ============================================================================

/// MAC enforcement state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MacState {
    Enforcing,
    Permissive,
    Disabled,
}

impl fmt::Display for MacState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Enforcing => write!(f, "ENFORCING"),
            Self::Permissive => write!(f, "permissive"),
            Self::Disabled => write!(f, "disabled"),
        }
    }
}

/// MAC policy information.
#[derive(Clone, Debug)]
pub struct MacStatus {
    /// Current enforcement state.
    pub state: MacState,
    /// Number of loaded policy rules.
    pub rules_loaded: u32,
    /// Number of policy violations since boot.
    pub violations: u64,
}

/// A single MAC policy rule (human-readable description).
#[derive(Clone, Debug)]
pub struct MacRule {
    /// Rule action (allow/deny).
    pub action: String,
    /// Operation (ALL, IPC_SEND, FILE_ACCESS, etc.).
    pub operation: String,
    /// Source label.
    pub source: String,
    /// Destination label.
    pub destination: String,
}

// ============================================================================
// Data types — Capabilities
// ============================================================================

/// A process capability entry.
#[derive(Clone, Debug)]
pub struct CapabilityInfo {
    /// Capability name (e.g., "CAP_NET_ADMIN").
    pub name: String,
    /// Whether the capability is set.
    pub enabled: bool,
    /// Capability bit position.
    pub bit: u32,
}

/// Process capability set.
#[derive(Clone, Debug)]
pub struct ProcessCaps {
    /// Process ID.
    pub pid: i32,
    /// Process name (comm).
    pub name: String,
    /// Effective capability set.
    pub effective: Vec<CapabilityInfo>,
    /// Permitted capability set.
    pub permitted: Vec<CapabilityInfo>,
    /// Bounding set.
    pub bounding: Vec<CapabilityInfo>,
}

// ============================================================================
// Data types — Audit
// ============================================================================

/// Audit daemon status.
#[derive(Clone, Debug)]
pub struct AuditStatus {
    /// Whether audit logging is enabled.
    pub enabled: bool,
    /// Whether the log file is open.
    pub log_open: bool,
    /// Number of records currently in the kernel buffer.
    pub pending_records: u32,
    /// Total events logged since boot.
    pub total_events: u64,
}

/// An audit event entry (human-readable).
#[derive(Clone, Debug)]
pub struct AuditEvent {
    /// Event serial number.
    pub serial: u64,
    /// Event type name (e.g., "IPC_DENIED", "AUTH_SUCCESS").
    pub event_type: String,
    /// Result (OK, EPERM, etc.).
    pub result: String,
    /// Subject endpoint (who triggered the event).
    pub subject: String,
    /// Object endpoint (who was targeted).
    pub object: String,
    /// Timestamp (ISO format or relative).
    pub timestamp: String,
}

// ============================================================================
// Public API — MAC
// ============================================================================

/// Show MAC enforcement status.
pub fn mac_status() -> Result<(), String> {
    let status = query_mac_status();

    let icon = match status.state {
        MacState::Enforcing => "▲",
        MacState::Permissive => "△",
        MacState::Disabled => "▼",
    };

    println!("MAC (Mandatory Access Control)");
    println!("──────────────────────────────");
    println!("State:      {} {} ({})", icon, status.state, match status.state {
        MacState::Enforcing => "all operations checked against policy",
        MacState::Permissive => "violations logged but not blocked",
        MacState::Disabled => "no MAC enforcement",
    });
    println!("Rules:      {} loaded", status.rules_loaded);
    println!("Violations: {}", status.violations);

    Ok(())
}

/// Enable MAC enforcement.
pub fn mac_enable() -> Result<(), String> {
    println!("Enabling MAC enforcement...");
    // TODO: IPC sendrec(macd_ep, MACD_RQ_ENABLE)
    println!("OK (not yet implemented — will send MACD_RQ_ENABLE to macd)");
    Ok(())
}

/// Disable MAC enforcement.
pub fn mac_disable() -> Result<(), String> {
    println!("Disabling MAC enforcement...");
    // TODO: IPC sendrec(macd_ep, MACD_RQ_DISABLE)
    println!("OK (not yet implemented — will send MACD_RQ_DISABLE to macd)");
    Ok(())
}

/// Show loaded MAC policy rules.
pub fn mac_show_rules() -> Result<(), String> {
    let rules = get_mac_rules();

    println!("{:<2} {:<6} {:<16} {:<16} {:<16}",
        "", "ACTION", "OPERATION", "SOURCE", "DESTINATION");
    println!("{}", "-".repeat(60));

    for r in &rules {
        let action_color = match r.action.as_str() {
            "allow" => "✓",
            "deny" => "✗",
            _ => "?",
        };

        println!("{:<2} {:<6} {:<16} {:<16} {:<16}",
            action_color,
            r.action,
            r.operation,
            r.source,
            r.destination,
        );
    }

    println!();
    println!("Total: {} rules", rules.len());

    Ok(())
}

// ============================================================================
// Public API — Capabilities
// ============================================================================

/// Show capabilities for a process.
pub fn caps_list(pid: i32) -> Result<(), String> {
    let caps = query_process_caps(pid);

    println!("Process capabilities for {} (pid {})", caps.name, caps.pid);
    println!("────────────────────────────────────────────");

    println!("\nEffective:");
    print_cap_set(&caps.effective);

    println!("\nPermitted:");
    print_cap_set(&caps.permitted);

    println!("\nBounding:");
    print_cap_set(&caps.bounding);

    Ok(())
}

/// Set a capability for a process.
pub fn caps_set(pid: i32, cap_name: &str) -> Result<(), String> {
    println!("Setting capability '{}' for pid {}...", cap_name, pid);
    // TODO: IPC _kernel_call(SYS_CAPCTL, CAPCTL_SET_CAPS)
    println!("OK (not yet implemented — will use SYS_CAPCTL kernel call)");
    Ok(())
}

// ============================================================================
// Public API — Audit
// ============================================================================

/// Show audit daemon status.
pub fn audit_status() -> Result<(), String> {
    let status = query_audit_status();

    println!("Audit Daemon (auditd)");
    println!("────────────────────");
    println!("Enabled:       {}", if status.enabled { "✓ yes" } else { "✗ no" });
    println!("Log file:      {}", if status.log_open { "✓ open" } else { "✗ closed" });
    println!("Pending:       {} records in kernel buffer", status.pending_records);
    println!("Total events:  {}", status.total_events);

    Ok(())
}

/// Enable audit logging.
pub fn audit_enable() -> Result<(), String> {
    println!("Enabling audit logging...");
    // TODO: IPC sendrec(auditd_ep, AUDITD_RQ_ENABLE)
    println!("OK (not yet implemented — will send AUDITD_RQ_ENABLE to auditd)");
    Ok(())
}

/// Disable audit logging.
pub fn audit_disable() -> Result<(), String> {
    println!("Disabling audit logging...");
    // TODO: IPC sendrec(auditd_ep, AUDITD_RQ_DISABLE)
    println!("OK (not yet implemented — will send AUDITD_RQ_DISABLE to auditd)");
    Ok(())
}

/// Show recent audit events.
pub fn audit_events(limit: usize) -> Result<(), String> {
    let events = get_audit_events(limit);

    if events.is_empty() {
        println!("No audit events available.");
        return Ok(());
    }

    println!("{:<6} {:<20} {:<10} {:<12} {:<12} {}",
        "SERIAL", "TYPE", "RESULT", "SUBJECT", "OBJECT", "TIME");
    println!("{}", "-".repeat(85));

    for e in &events {
        println!("{:<6} {:<20} {:<10} {:<12} {:<12} {}",
            e.serial,
            e.event_type,
            e.result,
            e.subject,
            e.object,
            e.timestamp,
        );
    }

    println!();
    println!("Showing {} of {} events", events.len(), events.len());

    Ok(())
}

/// Show audit statistics summary.
pub fn audit_stats() -> Result<(), String> {
    let events = get_audit_events(100);

    let total = events.len();
    let mut by_type: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for e in &events {
        *by_type.entry(e.event_type.clone()).or_insert(0) += 1;
    }

    println!("Audit Statistics");
    println!("───────────────");
    println!("Total events: {}", if total > 0 { format!("{}", total) } else { "N/A (using stubs)".to_string() });

    if !by_type.is_empty() {
        println!("\nBreakdown by type:");
        for (event_type, count) in &by_type {
            let pct = if total > 0 {
                (*count as f64 / total as f64 * 100.0) as u32
            } else {
                0
            };
            let bar_len = (pct as f64 / 100.0 * 20.0) as usize;
            let bar: String = std::iter::repeat('█').take(bar_len)
                .chain(std::iter::repeat('░').take(20 - bar_len))
                .collect();
            println!("  {:<20} {:>5} ({:>2}%)  [{}]",
                event_type, count, pct, bar);
        }
    }

    Ok(())
}

// ============================================================================
// Data collectors (stubs — will use real IPC on MINIX)
// ============================================================================

/// Query MAC daemon status.
pub(crate) fn query_mac_status() -> MacStatus {
    // Stub — on MINIX:
    // 1. ds_retrieve_label_endpt("macd", &ep)
    // 2. m.m_type = MACD_RQ_STATUS
    // 3. sendrec(ep, &m)
    // 4. read MACD_STATUS_ENABLED, MACD_STATUS_NRULES from reply

    MacStatus {
        state: MacState::Enforcing,
        rules_loaded: 47,
        violations: 12,
    }
}

/// Get loaded MAC policy rules.
fn get_mac_rules() -> Vec<MacRule> {
    // Stub — on MINIX: read compiled policy or IPC with macd to enumerate rules
    vec![
        MacRule {
            action: "allow".to_string(),
            operation: "ALL".to_string(),
            source: "rs".to_string(),
            destination: "ANY".to_string(),
        },
        MacRule {
            action: "allow".to_string(),
            operation: "IPC_SEND".to_string(),
            source: "pm".to_string(),
            destination: "ANY".to_string(),
        },
        MacRule {
            action: "allow".to_string(),
            operation: "IPC_SEND".to_string(),
            source: "vfs".to_string(),
            destination: "mfs".to_string(),
        },
        MacRule {
            action: "allow".to_string(),
            operation: "IPC_SEND".to_string(),
            source: "vfs".to_string(),
            destination: "ext4".to_string(),
        },
        MacRule {
            action: "allow".to_string(),
            operation: "IPC_SEND".to_string(),
            source: "vfs".to_string(),
            destination: "pm".to_string(),
        },
        MacRule {
            action: "allow".to_string(),
            operation: "IPC_SEND".to_string(),
            source: "ANY".to_string(),
            destination: "auditd".to_string(),
        },
        MacRule {
            action: "allow".to_string(),
            operation: "IPC_SEND".to_string(),
            source: "ANY".to_string(),
            destination: "macd".to_string(),
        },
        MacRule {
            action: "deny".to_string(),
            operation: "RAWIO".to_string(),
            source: "init".to_string(),
            destination: "ANY".to_string(),
        },
        MacRule {
            action: "allow".to_string(),
            operation: "PRIVCTL_SET_SYS".to_string(),
            source: "rs".to_string(),
            destination: "ANY".to_string(),
        },
        MacRule {
            action: "allow".to_string(),
            operation: "PROC_KILL".to_string(),
            source: "pm".to_string(),
            destination: "ANY".to_string(),
        },
    ]
}

/// Query process capabilities.
fn query_process_caps(pid: i32) -> ProcessCaps {
    // Stub — on MINIX:
    // 1. _kernel_call(SYS_CAPCTL, &m) with CAPCTL_GET_CAPS
    // 2. or read /proc/<pid>/psinfo for process name

    let name = if pid <= 0 {
        "kernel".to_string()
    } else {
        match pid {
            1 => "init".to_string(),
            100 => "vfs".to_string(),
            101 => "pm".to_string(),
            102 => "rs".to_string(),
            103 => "vm".to_string(),
            _ => format!("process_{}", pid),
        }
    };

    let (effective, permitted, bounding) = if pid == 100 || pid == 102 {
        // RS and VFS have full capabilities
        (cap_set_full(), cap_set_full(), cap_set_full())
    } else if pid > 0 && pid <= 100 {
        // Core services have elevated capabilities
        (cap_set_elevated(), cap_set_elevated(), cap_set_base())
    } else {
        // User processes have base capabilities
        (cap_set_base(), cap_set_base(), cap_set_base())
    };

    ProcessCaps {
        pid,
        name,
        effective,
        permitted,
        bounding,
    }
}

/// Generate a full capability set.
fn cap_set_full() -> Vec<CapabilityInfo> {
    vec![
        CapabilityInfo { name: "CAP_SYS_RAWIO".to_string(), enabled: true, bit: 0 },
        CapabilityInfo { name: "CAP_NET_ADMIN".to_string(), enabled: true, bit: 1 },
        CapabilityInfo { name: "CAP_SYS_ADMIN".to_string(), enabled: true, bit: 2 },
        CapabilityInfo { name: "CAP_IPC_OWNER".to_string(), enabled: true, bit: 3 },
        CapabilityInfo { name: "CAP_FS_MANAGE".to_string(), enabled: true, bit: 4 },
        CapabilityInfo { name: "CAP_DEV_MANAGE".to_string(), enabled: true, bit: 5 },
        CapabilityInfo { name: "CAP_SYS_DEBUG".to_string(), enabled: true, bit: 6 },
        CapabilityInfo { name: "CAP_SECURITY".to_string(), enabled: true, bit: 7 },
    ]
}

/// Generate an elevated capability set (for core services).
fn cap_set_elevated() -> Vec<CapabilityInfo> {
    vec![
        CapabilityInfo { name: "CAP_NET_ADMIN".to_string(), enabled: true, bit: 1 },
        CapabilityInfo { name: "CAP_IPC_OWNER".to_string(), enabled: true, bit: 3 },
        CapabilityInfo { name: "CAP_FS_MANAGE".to_string(), enabled: true, bit: 4 },
    ]
}

/// Generate a base capability set (for user processes).
fn cap_set_base() -> Vec<CapabilityInfo> {
    vec![
        CapabilityInfo { name: "CAP_IPC_OWNER".to_string(), enabled: true, bit: 3 },
    ]
}

/// Query audit daemon status.
pub(crate) fn query_audit_status() -> AuditStatus {
    // Stub — on MINIX:
    // 1. ds_retrieve_label_endpt("auditd", &ep)
    // 2. m.m_type = AUDITD_RQ_STATUS
    // 3. sendrec(ep, &m)
    // 4. read AUDITD_STATUS_LOG, etc. from reply

    AuditStatus {
        enabled: true,
        log_open: true,
        pending_records: 42,
        total_events: 12345,
    }
}

/// Get recent audit events.
fn get_audit_events(limit: usize) -> Vec<AuditEvent> {
    // Stub — on MINIX:
    // 1. SYS_AUDIT(ep, AUDIT_OP_GET_COUNT) — get available records
    // 2. SYS_AUDIT(ep, AUDIT_OP_RETRIEVE, buffer, count) — read records
    // 3. Format each audit_record into AuditEvent

    let sample_events = vec![
        AuditEvent {
            serial: 101, event_type: "AUTH_SUCCESS".to_string(), result: "OK".to_string(),
            subject: "init".to_string(), object: "pm".to_string(),
            timestamp: "12:34:56".to_string(),
        },
        AuditEvent {
            serial: 102, event_type: "SERVICE_START".to_string(), result: "OK".to_string(),
            subject: "rs".to_string(), object: "bluetoothd".to_string(),
            timestamp: "12:35:01".to_string(),
        },
        AuditEvent {
            serial: 103, event_type: "IPC_DENIED".to_string(), result: "EPERM".to_string(),
            subject: "init".to_string(), object: "mfs".to_string(),
            timestamp: "12:35:12".to_string(),
        },
        AuditEvent {
            serial: 104, event_type: "PRIV_CHANGE".to_string(), result: "OK".to_string(),
            subject: "rs".to_string(), object: "vfs".to_string(),
            timestamp: "12:36:00".to_string(),
        },
        AuditEvent {
            serial: 105, event_type: "MAC_VIOLATION".to_string(), result: "EPERM".to_string(),
            subject: "init".to_string(), object: "devman".to_string(),
            timestamp: "12:36:30".to_string(),
        },
        AuditEvent {
            serial: 106, event_type: "AUTH_FAILURE".to_string(), result: "EACCES".to_string(),
            subject: "sshd".to_string(), object: "root".to_string(),
            timestamp: "12:37:00".to_string(),
        },
        AuditEvent {
            serial: 107, event_type: "SERVICE_CRASH".to_string(), result: "SIGSEGV".to_string(),
            subject: "bluetoothd".to_string(), object: "rs".to_string(),
            timestamp: "12:38:15".to_string(),
        },
        AuditEvent {
            serial: 108, event_type: "SYSCALL_AUTH".to_string(), result: "OK".to_string(),
            subject: "pm".to_string(), object: "init".to_string(),
            timestamp: "12:39:00".to_string(),
        },
        AuditEvent {
            serial: 109, event_type: "DEVICE_BIND".to_string(), result: "OK".to_string(),
            subject: "rs".to_string(), object: "ahci".to_string(),
            timestamp: "12:40:00".to_string(),
        },
        AuditEvent {
            serial: 110, event_type: "FILE_DENIED".to_string(), result: "EACCES".to_string(),
            subject: "init".to_string(), object: "/etc/shadow".to_string(),
            timestamp: "12:41:00".to_string(),
        },
    ];

    sample_events.into_iter().take(limit).collect()
}

// ============================================================================
// Display helpers
// ============================================================================

/// Print a capability set as a formatted table.
fn print_cap_set(caps: &[CapabilityInfo]) {
    for c in caps {
        let icon = if c.enabled { "✓" } else { " " };
        println!("  {} {} (bit {})", icon, c.name, c.bit);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mac_state_display() {
        assert_eq!(format!("{}", MacState::Enforcing), "ENFORCING");
        assert_eq!(format!("{}", MacState::Disabled), "disabled");
    }

    #[test]
    fn test_mac_status() {
        let s = query_mac_status();
        assert_eq!(s.state, MacState::Enforcing);
        assert!(s.rules_loaded > 0);
    }

    #[test]
    fn test_mac_rules_nonempty() {
        let rules = get_mac_rules();
        assert!(!rules.is_empty());
        assert!(rules.len() >= 10);
        assert!(rules.iter().any(|r| r.action == "allow"));
        assert!(rules.iter().any(|r| r.action == "deny"));
    }

    #[test]
    fn test_mac_public_functions() {
        assert!(mac_status().is_ok());
        assert!(mac_enable().is_ok());
        assert!(mac_disable().is_ok());
        assert!(mac_show_rules().is_ok());
    }

    #[test]
    fn test_caps_root_process() {
        let caps = query_process_caps(102); // rs
        assert_eq!(caps.name, "rs");
        assert!(caps.effective.len() >= 3);
        assert!(caps.effective.iter().any(|c| c.name == "CAP_SYS_RAWIO"));
    }

    #[test]
    fn test_caps_user_process() {
        let caps = query_process_caps(200);
        assert!(caps.effective.len() <= caps.permitted.len());
        assert!(caps.effective.iter().all(|c| c.enabled));
    }

    #[test]
    fn test_caps_kernel() {
        let caps = query_process_caps(0);
        assert_eq!(caps.name, "kernel");
    }

    #[test]
    fn test_caps_list_public() {
        assert!(caps_list(100).is_ok());
        assert!(caps_list(200).is_ok());
        assert!(caps_list(0).is_ok());
    }

    #[test]
    fn test_caps_set_stub() {
        assert!(caps_set(100, "CAP_NET_ADMIN").is_ok());
    }

    #[test]
    fn test_audit_status() {
        let s = query_audit_status();
        assert!(s.enabled);
        assert!(s.total_events > 0);
    }

    #[test]
    fn test_audit_events_nonempty() {
        let events = get_audit_events(100);
        assert!(!events.is_empty());
        assert_eq!(events.len(), 10);
    }

    #[test]
    fn test_audit_events_limit() {
        let events = get_audit_events(3);
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_audit_events_all_types() {
        let events = get_audit_events(100);
        let types: std::collections::HashSet<&str> =
            events.iter().map(|e| e.event_type.as_str()).collect();
        assert!(types.contains("IPC_DENIED"));
        assert!(types.contains("AUTH_SUCCESS"));
        assert!(types.contains("MAC_VIOLATION"));
        assert!(types.contains("SERVICE_CRASH"));
    }

    #[test]
    fn test_audit_public_functions() {
        assert!(audit_status().is_ok());
        assert!(audit_enable().is_ok());
        assert!(audit_disable().is_ok());
        assert!(audit_events(5).is_ok());
        assert!(audit_events(0).is_ok());
        assert!(audit_stats().is_ok());
    }

    #[test]
    fn test_print_cap_set_dont_panic() {
        let caps = cap_set_base();
        // Just verify the display function doesn't panic
        // (it prints to stdout, which is fine in tests)
        print_cap_set(&caps);
    }

    #[test]
    fn test_mac_state_display_all() {
        assert_eq!(format!("{}", MacState::Enforcing), "ENFORCING");
        assert_eq!(format!("{}", MacState::Permissive), "permissive");
        assert_eq!(format!("{}", MacState::Disabled), "disabled");
    }

    #[test]
    fn test_mac_status_positive_values() {
        let s = query_mac_status();
        assert!(s.rules_loaded > 0);
        assert!(s.violations > 0);
    }

    #[test]
    fn test_audit_events_limit_zero() {
        let events = get_audit_events(0);
        assert!(events.is_empty());
    }

    #[test]
    fn test_audit_events_limit_less_than_available() {
        let events = get_audit_events(3);
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_audit_events_limit_greater_than_available() {
        let all = get_audit_events(100);
        let over = get_audit_events(200);
        assert_eq!(all.len(), over.len()); // capped at available
    }

    #[test]
    fn test_audit_stats_dont_panic() {
        assert!(audit_stats().is_ok());
        assert!(audit_stats().is_ok()); // call twice — no state mutation
    }

    #[test]
    fn test_query_process_caps_zero() {
        let caps = query_process_caps(0);
        assert_eq!(caps.name, "kernel");
    }

    #[test]
    fn test_query_process_caps_negative() {
        let caps = query_process_caps(-1);
        assert_eq!(caps.name, "kernel");
    }

    #[test]
    fn test_cap_set_sizes() {
        assert_eq!(cap_set_full().len(), 8);
        assert_eq!(cap_set_elevated().len(), 3);
        assert_eq!(cap_set_base().len(), 1);
    }
}
