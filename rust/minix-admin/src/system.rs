//! # System Monitor
//!
//! Collects and displays system information: CPU, memory, disk, uptime.
//!
//! Uses MINIX syscalls (`sys_getinfo`, `svrctl`) and `/proc` where available.

/// Show all system information.
pub fn show_system_info() -> Result<(), String> {
    println!("=== System Information ===");
    println!();

    show_hostname()?;
    show_kernel_version()?;
    show_uptime()?;
    println!();
    show_cpu()?;
    show_memory()?;
    show_disk()?;

    Ok(())
}

/// Show CPU information.
pub fn show_cpu() -> Result<(), String> {
    // On MINIX, CPU info would come from /proc/cpuinfo or sys_getinfo
    println!("CPU:");
    println!("  Model:  GergiOS (MINIX 3 compatible)");
    println!("  Cores:  {}", num_cpus());
    println!("  Load:   {}", get_loadavg().map(|l| format!("{:.1}%", l)).unwrap_or_default());
    println!("  Freq:   {}", "dynamic (ACPI)");
    Ok(())
}

/// Show memory information.
pub fn show_memory() -> Result<(), String> {
    let mem = get_memory_info().unwrap_or(MemoryInfo {
        total_kb: 0,
        used_kb: 0,
        free_kb: 0,
    });

    println!("Memory:");
    if mem.total_kb > 0 {
        let used_pct = if mem.total_kb > 0 {
            (mem.used_kb as f64 / mem.total_kb as f64 * 100.0) as u32
        } else {
            0
        };
        let bar = progress_bar(used_pct, 20);

        println!("  Total:  {} MB", mem.total_kb / 1024);
        println!("  Used:   {} MB ({}%) {}", mem.used_kb / 1024, used_pct, bar);
        println!("  Free:   {} MB", mem.free_kb / 1024);
    } else {
        println!("  (not available — requires /proc/meminfo)");
    }

    Ok(())
}

/// Show disk information.
pub fn show_disk() -> Result<(), String> {
    // On MINIX, disk info would come from statvfs() or df
    println!("Disk:");
    print_disk_entry("/", "root filesystem");
    print_disk_entry("/home", "home");
    print_disk_entry("/var", "variable data");

    Ok(())
}

/// Show system uptime.
pub fn show_uptime() -> Result<(), String> {
    let uptime = get_uptime_secs().unwrap_or(0);
    let load = get_loadavg().unwrap_or(0.0);

    println!("Uptime: {}", format_duration(uptime));
    println!("Load:   {:.1}%", load);

    Ok(())
}

/// Show hostname.
fn show_hostname() -> Result<(), String> {
    let hostname = get_hostname().unwrap_or_else(|| "gergios".to_string());
    println!("Hostname: {}", hostname);
    Ok(())
}

/// Show kernel version.
fn show_kernel_version() -> Result<(), String> {
    println!("Kernel:  MINIX 3 (GergiOS)");
    Ok(())
}

// ============================================================================
// Data types
// ============================================================================

#[derive(Clone, Debug)]
pub(crate) struct MemoryInfo {
    pub total_kb: u64,
    pub used_kb: u64,
    pub free_kb: u64,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub(crate) struct DiskEntry {
    pub path: String,
    pub total_kb: u64,
    pub used_kb: u64,
    pub free_kb: u64,
    pub label: String,
}

// ============================================================================
// System data collectors (stubs — will use real syscalls on MINIX)
// ============================================================================

pub(crate) fn num_cpus() -> u32 {
    // TODO: sysconf(_SC_NPROCESSORS_ONLN)
    4
}

pub(crate) fn get_loadavg() -> Option<f64> {
    // TODO: getloadavg() libc call
    Some(23.5) // stub
}

pub(crate) fn get_memory_info() -> Option<MemoryInfo> {
    // TODO: read /proc/meminfo or sysctl
    Some(MemoryInfo {
        total_kb: 2 * 1024 * 1024, // 2 GB
        used_kb: 512 * 1024,       // 512 MB
        free_kb: 1_536 * 1024,     // rest
    })
}

pub(crate) fn get_uptime_secs() -> Option<u64> {
    // TODO: sysinfo() or /proc/uptime
    Some(3 * 86400 + 12 * 3600 + 30 * 60) // 3d 12h 30m stub
}

pub(crate) fn get_hostname() -> Option<String> {
    // TODO: gethostname() libc call
    Some("gergios".to_string())
}

pub(crate) fn get_disk_usage(path: &str) -> Option<DiskEntry> {
    // TODO: statvfs() call
    Some(DiskEntry {
        path: path.to_string(),
        total_kb: 8 * 1024 * 1024, // 8 GB
        used_kb: 2 * 1024 * 1024,  // 2 GB
        free_kb: 6 * 1024 * 1024,  // 6 GB
        label: String::new(),
    })
}

// ============================================================================
// Display helpers
// ============================================================================

/// Render a progress bar like "███████░░░ 70%"
fn progress_bar(pct: u32, width: usize) -> String {
    let pct = pct.min(100);
    let filled = ((pct as f64 / 100.0) * width as f64) as usize;
    let filled = filled.min(width);
    let empty = width.saturating_sub(filled);

    let bar: String = std::iter::repeat('█')
        .take(filled)
        .chain(std::iter::repeat('░').take(empty))
        .collect();

    format!("[{:<width$}] {:>3}%", bar, pct, width = width)
}

/// Format seconds as human-readable duration.
fn format_duration(total_secs: u64) -> String {
    let days = total_secs / 86400;
    let hours = (total_secs % 86400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if days > 0 {
        format!("{} days, {:02}:{:02}:{:02}", days, hours, minutes, secs)
    } else if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, secs)
    } else {
        format!("{:02}:{:02}", minutes, secs)
    }
}

/// Print a disk usage entry.
fn print_disk_entry(path: &str, label: &str) {
    if let Some(disk) = get_disk_usage(path) {
        let used_pct = if disk.total_kb > 0 {
            (disk.used_kb as f64 / disk.total_kb as f64 * 100.0) as u32
        } else {
            0
        };
        let bar = progress_bar(used_pct, 30);

        println!("  {:<5} {}  {:>6} MB / {:<6} MB  {}",
            label,
            path,
            disk.used_kb / 1024,
            disk.total_kb / 1024,
            bar,
        );
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_bar() {
        let bar = progress_bar(50, 10);
        assert!(bar.contains("50%"));
        // [█████░░░░░]  50% = 1 + (10×3bytes) + 2 + 3 + 1 = 37 bytes
        assert_eq!(bar.len(), 37);
    }

    #[test]
    fn test_progress_bar_0() {
        let bar = progress_bar(0, 10);
        assert!(bar.contains("0%"));
        assert!(bar.contains('░'));
    }

    #[test]
    fn test_progress_bar_100() {
        let bar = progress_bar(100, 10);
        assert!(bar.contains("100%"));
        assert!(bar.contains('█'));
        assert!(!bar.contains('░'));
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(0), "00:00");
        assert_eq!(format_duration(30), "00:30");
        assert_eq!(format_duration(3661), "01:01:01");
        assert!(format_duration(90061).contains("days"));
    }

    #[test]
    fn test_num_cpus_positive() {
        assert!(num_cpus() > 0);
    }

    #[test]
    fn test_get_loadavg() {
        let load = get_loadavg();
        assert!(load.is_some());
    }

    #[test]
    fn test_get_hostname() {
        let hostname = get_hostname();
        assert!(hostname.is_some());
        assert!(!hostname.unwrap().is_empty());
    }

    #[test]
    fn test_get_memory_has_values() {
        let mem = get_memory_info();
        assert!(mem.is_some());
        let m = mem.unwrap();
        assert!(m.total_kb > 0);
        assert!(m.total_kb >= m.used_kb + m.free_kb - 1024); // rounding
    }

    #[test]
    fn test_get_disk_usage() {
        let disk = get_disk_usage("/");
        assert!(disk.is_some());
        let d = disk.unwrap();
        assert_eq!(d.path, "/");
        assert!(d.total_kb > 0);
    }

    #[test]
    fn test_format_duration_zero() {
        assert_eq!(format_duration(0), "00:00");
    }

    #[test]
    fn test_format_duration_days() {
        assert!(format_duration(86400 * 365).contains("365 days"));
    }

    #[test]
    fn test_format_duration_overflow() {
        // Large values shouldn't panic
        let result = format_duration(u64::MAX);
        assert!(result.contains("days") || result.contains(":"));
    }

    #[test]
    fn test_progress_bar_clamp() {
        // Values > 100 should be clamped
        let bar = progress_bar(150, 10);
        assert!(bar.contains("100%"));
    }

    #[test]
    fn test_progress_bar_zero_width() {
        let bar = progress_bar(50, 0);
        assert_eq!(bar.len(), 7); // "[  0%"  or similar
    }

    #[test]
    fn test_num_cpus_always_some() {
        assert!(num_cpus() >= 1);
    }

    #[test]
    fn test_get_loadavg_range() {
        let load = get_loadavg();
        assert!(load.is_some());
        let val = load.unwrap();
        assert!(val >= 0.0 && val <= 100.0);
    }

    #[test]
    fn test_get_memory_total_greater_than_zero() {
        let mem = get_memory_info();
        assert!(mem.is_some());
        assert!(mem.unwrap().total_kb > 10_000); // at least 10MB
    }

    #[test]
    fn test_get_disk_usage_root_is_positive() {
        let disk = get_disk_usage("/");
        assert!(disk.is_some());
        let d = disk.unwrap();
        assert!(d.total_kb > 0);
        assert_eq!(d.path, "/");
    }

    #[test]
    fn test_get_disk_usage_unknown_path() {
        let disk = get_disk_usage("/nonexistent");
        assert!(disk.is_some()); // returns default stub for any path
    }

    #[test]
    fn test_display_functions_dont_panic() {
        assert!(show_cpu().is_ok());
        assert!(show_memory().is_ok());
        assert!(show_system_info().is_ok());
    }
}
