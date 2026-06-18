//! # procfs-path — MINIX ProcFS path parsing and validation
//!
//! This crate provides safe, `no_std` path parsing utilities for the MINIX
//! ProcFS filesystem server (`minix/fs/procfs/`). It handles:
//!
//! - **PID parsing**: Convert string directory names to `pid_t` values
//! - **Path validation**: Check if a path component is a valid PID directory
//! - **File name matching**: Match against known ProcFS file names
//! - **Process slot management**: Validate process slot indices
//!
//! ## Usage
//!
//! ```ignore
//! use procfs_path::*;
//!
//! let pid = parse_pid("1234").unwrap();
//! assert!(is_pid_name("1234"));
//! assert!(!is_pid_name("abc"));
//! ```

#![no_std]
#![deny(unsafe_code)]

use core::fmt;

// ---------------------------------------------------------------------------
// ProcFS file names
// ---------------------------------------------------------------------------

/// The set of known ProcFS files that appear in each `/proc/<pid>/` directory.
pub const PID_FILE_NAMES: &[&str] = &[
    "psinfo",
    "cmdline",
    "environ",
    "map",
];

/// The set of known root-level ProcFS files (static files).
pub const ROOT_FILE_NAMES: &[&str] = &[
    "hz",
    "uptime",
    "loadavg",
    "kinfo",
    "meminfo",
    "pci",
    "dmap",
    "cpuinfo",
    "ipcvecs",
    "mounts",
    "service",
];

// ---------------------------------------------------------------------------
// PID parsing
// ---------------------------------------------------------------------------

/// Errors that can occur during PID parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PidParseError {
    /// The string is empty.
    Empty,
    /// The string contains non-digit characters.
    InvalidCharacter,
    /// The string is too long for a valid PID.
    TooLong,
    /// The parsed value is zero (PID 0 is invalid).
    ZeroPid,
    /// The parsed value exceeds the maximum PID.
    Overflow,
}

impl fmt::Display for PidParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PidParseError::Empty => write!(f, "empty string"),
            PidParseError::InvalidCharacter => write!(f, "invalid character"),
            PidParseError::TooLong => write!(f, "too long"),
            PidParseError::ZeroPid => write!(f, "PID 0 is reserved"),
            PidParseError::Overflow => write!(f, "overflow"),
        }
    }
}

/// Maximum valid PID value on MINIX.
pub const PID_MAX: i32 = 32768;

/// Maximum number of digits for a valid PID.
pub const PID_MAX_DIGITS: usize = 5; // "32768" = 5 chars

/// Parse a string as a PID, returning the `pid_t` if valid.
///
/// A valid PID:
/// - Is non-empty
/// - Contains only ASCII digits
/// - Is not "0" (PID 0 is reserved for the idle task)
/// - Does not exceed `PID_MAX`
///
/// This performs no heap allocation.
pub fn parse_pid(s: &str) -> Result<i32, PidParseError> {
    if s.is_empty() {
        return Err(PidParseError::Empty);
    }
    if s.len() > PID_MAX_DIGITS {
        return Err(PidParseError::TooLong);
    }
    // MINIX uses plain ASCII, so this is simple
    let mut value: i32 = 0;
    for &b in s.as_bytes() {
        if !b.is_ascii_digit() {
            return Err(PidParseError::InvalidCharacter);
        }
        let digit = (b - b'0') as i32;
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(digit))
            .ok_or(PidParseError::Overflow)?;
        if value > PID_MAX {
            return Err(PidParseError::Overflow);
        }
    }
    if value == 0 {
        return Err(PidParseError::ZeroPid);
    }
    Ok(value)
}

/// Check if a string is a valid PID directory name.
///
/// A valid PID name is a non-zero number that could be a process ID.
pub fn is_pid_name(s: &str) -> bool {
    parse_pid(s).is_ok()
}

// ---------------------------------------------------------------------------
// File name matching
// ---------------------------------------------------------------------------

/// Check if a name matches one of the known PID files.
pub fn is_pid_file(name: &str) -> bool {
    PID_FILE_NAMES.contains(&name)
}

/// Check if a name matches one of the known root files.
pub fn is_root_file(name: &str) -> bool {
    ROOT_FILE_NAMES.contains(&name)
}

/// Check if a name is a known ProcFS file.
pub fn is_procfs_file(name: &str) -> bool {
    is_pid_file(name) || is_root_file(name)
}

// ---------------------------------------------------------------------------
// Process slot management
// ---------------------------------------------------------------------------

/// Result of validating a process slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotStatus {
    /// The slot is a kernel task (slot < NR_TASKS).
    Task,
    /// The slot is a user process (slot >= NR_TASKS).
    Process,
    /// The slot is out of range.
    Invalid,
}

/// Default number of processes (from MINIX config).
pub const NR_TASKS: usize = 1023;

/// Default number of process slots.
pub const NR_PROCS: usize = 256;

/// Maximum slot index.
pub const MAX_SLOT: usize = NR_TASKS + NR_PROCS;

/// Validate a process slot index.
pub fn validate_slot(slot: usize) -> SlotStatus {
    if slot >= MAX_SLOT {
        SlotStatus::Invalid
    } else if slot < NR_TASKS {
        SlotStatus::Task
    } else {
        SlotStatus::Process
    }
}

/// Check if a slot belongs to a kernel task.
pub fn is_task_slot(slot: usize) -> bool {
    slot < NR_TASKS
}

/// Convert a PID to the corresponding process slot.
///
/// Returns `None` if the PID is out of range.
pub fn pid_to_slot(pid: i32) -> Option<usize> {
    if pid <= 0 || pid > PID_MAX {
        return None;
    }
    // Kernel tasks occupy slots 0..NR_TASKS-1 with negative-like PIDs
    // User processes start at slot NR_TASKS
    // The actual mapping depends on the kernel's assignment
    Some(NR_TASKS + (pid as usize - 1))
}

/// Compute the index for per-PID directory entries.
pub fn pid_slot_range() -> core::ops::Range<usize> {
    0..(NR_TASKS + NR_PROCS)
}

// ---------------------------------------------------------------------------
// ProcFS path validation
// ---------------------------------------------------------------------------

/// Validation result for a ProcFS path lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupResult {
    /// The name is a valid PID directory.
    PidDirectory(i32),
    /// The name is a known root file.
    RootFile,
    /// The name is a known PID file (within a PID directory).
    PidFile,
    /// The name is the "service" directory.
    ServiceDir,
    /// The name is invalid or unknown.
    Unknown,
}

/// Validate a path component in the ProcFS tree.
///
/// - At the root level: check for known root files, service, or PID name.
/// - At the PID level: check for known PID files.
pub fn validate_path_component(name: &str, is_pid_context: bool) -> LookupResult {
    if is_pid_context {
        if is_pid_file(name) {
            LookupResult::PidFile
        } else {
            LookupResult::Unknown
        }
    } else if let Ok(pid) = parse_pid(name) {
        LookupResult::PidDirectory(pid)
    } else if name == "service" {
        LookupResult::ServiceDir
    } else if is_root_file(name) {
        LookupResult::RootFile
    } else {
        LookupResult::Unknown
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_pid() {
        assert_eq!(parse_pid("1").unwrap(), 1);
        assert_eq!(parse_pid("1234").unwrap(), 1234);
        assert_eq!(parse_pid("32768").unwrap(), 32768);
    }

    #[test]
    fn parse_invalid_pid_empty() {
        assert_eq!(parse_pid(""), Err(PidParseError::Empty));
    }

    #[test]
    fn parse_invalid_pid_zero() {
        assert_eq!(parse_pid("0"), Err(PidParseError::ZeroPid));
    }

    #[test]
    fn parse_invalid_pid_non_digit() {
        assert_eq!(parse_pid("abc"), Err(PidParseError::InvalidCharacter));
        assert_eq!(parse_pid("12a4"), Err(PidParseError::InvalidCharacter));
        assert_eq!(parse_pid("-1"), Err(PidParseError::InvalidCharacter));
    }

    #[test]
    fn parse_invalid_pid_too_long() {
        assert_eq!(parse_pid("123456"), Err(PidParseError::TooLong));
    }

    #[test]
    fn parse_invalid_pid_overflow() {
        assert_eq!(parse_pid("99999"), Err(PidParseError::Overflow));
    }

    #[test]
    fn is_pid_name_valid() {
        assert!(is_pid_name("1"));
        assert!(is_pid_name("42"));
        assert!(is_pid_name("32768"));
    }

    #[test]
    fn is_pid_name_invalid() {
        assert!(!is_pid_name("0"));
        assert!(!is_pid_name(""));
        assert!(!is_pid_name("abc"));
    }

    #[test]
    fn test_pid_file_names() {
        assert!(is_pid_file("psinfo"));
        assert!(is_pid_file("cmdline"));
        assert!(is_pid_file("environ"));
        assert!(is_pid_file("map"));
        assert!(!is_pid_file("nonexistent"));
    }

    #[test]
    fn test_root_file_names() {
        assert!(is_root_file("hz"));
        assert!(is_root_file("uptime"));
        assert!(is_root_file("cpuinfo"));
        assert!(is_root_file("service"));
        assert!(!is_root_file("nonexistent"));
    }

    #[test]
    fn test_slot_validation() {
        assert_eq!(validate_slot(0), SlotStatus::Task);
        assert_eq!(validate_slot(100), SlotStatus::Task);
        assert_eq!(validate_slot(1022), SlotStatus::Task);
        assert_eq!(validate_slot(1023), SlotStatus::Process);
        assert_eq!(validate_slot(1100), SlotStatus::Process);
        assert_eq!(validate_slot(1278), SlotStatus::Process);
        assert_eq!(validate_slot(1279), SlotStatus::Invalid);
        assert_eq!(validate_slot(2000), SlotStatus::Invalid);
    }

    #[test]
    fn test_is_task_slot() {
        assert!(is_task_slot(0));
        assert!(is_task_slot(1022));
        assert!(!is_task_slot(1023));
    }

    #[test]
    fn test_path_validation_root() {
        assert_eq!(validate_path_component("hz", false), LookupResult::RootFile);
        assert_eq!(validate_path_component("service", false), LookupResult::ServiceDir);
        assert_eq!(validate_path_component("1234", false), LookupResult::PidDirectory(1234));
        assert_eq!(validate_path_component("unknown", false), LookupResult::Unknown);
    }

    #[test]
    fn test_path_validation_pid_context() {
        assert_eq!(validate_path_component("psinfo", true), LookupResult::PidFile);
        assert_eq!(validate_path_component("cmdline", true), LookupResult::PidFile);
        assert_eq!(validate_path_component("map", true), LookupResult::PidFile);
        assert_eq!(validate_path_component("unknown", true), LookupResult::Unknown);
        // Root files are not valid in PID context
        assert_eq!(validate_path_component("hz", true), LookupResult::Unknown);
    }

    #[test]
    fn test_pid_file_names_constant() {
        assert_eq!(PID_FILE_NAMES.len(), 4);
        assert_eq!(PID_FILE_NAMES[0], "psinfo");
        assert_eq!(PID_FILE_NAMES[3], "map");
    }

    // Helper to test Display without std::format!
    struct TestBuf<const N: usize>([u8; N], usize);
    impl<const N: usize> TestBuf<N> {
        fn new() -> Self { Self([0u8; N], 0) }
        fn as_str(&self) -> &str { core::str::from_utf8(&self.0[..self.1]).unwrap() }
    }
    impl<const N: usize> core::fmt::Write for TestBuf<N> {
        fn write_str(&mut self, s: &str) -> core::fmt::Result {
            let b = s.as_bytes();
            if self.1 + b.len() > N { return Err(core::fmt::Error); }
            self.0[self.1..self.1 + b.len()].copy_from_slice(b);
            self.1 += b.len();
            Ok(())
        }
    }

    #[test]
    fn test_error_display() {
        use core::fmt::Write;
        let mut buf = TestBuf::<64>::new();
        write!(buf, "{}", PidParseError::Empty).unwrap();
        assert_eq!(buf.as_str(), "empty string");
        let mut buf = TestBuf::<64>::new();
        write!(buf, "{}", PidParseError::InvalidCharacter).unwrap();
        assert_eq!(buf.as_str(), "invalid character");
        let mut buf = TestBuf::<64>::new();
        write!(buf, "{}", PidParseError::Overflow).unwrap();
        assert_eq!(buf.as_str(), "overflow");
    }
}
