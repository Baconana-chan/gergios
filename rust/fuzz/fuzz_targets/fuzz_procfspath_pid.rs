#![no_main]

use libfuzzer_sys::fuzz_target;
use procfs_path::*;

/// Fuzz the ProcFS path parsing functions with arbitrary byte sequences.
///
/// Exercises parse_pid, is_pid_name, validate_path_component,
/// and validate_slot with random inputs to ensure no panics.
fuzz_target!(|data: &[u8]| {
    if data.is_empty() {
        return;
    }

    // Interpret data as a potential PID string (ASCII)
    let s = core::str::from_utf8(data).unwrap_or("");

    // Run all parsing functions — must not panic
    let _ = parse_pid(s);
    let _ = is_pid_name(s);
    let _ = is_pid_file(s);
    let _ = is_root_file(s);
    let _ = is_procfs_file(s);
    let _ = validate_path_component(s, false);
    let _ = validate_path_component(s, true);

    // Fuzz slot validation with first byte as slot index
    let slot = data[0] as usize;
    let _ = validate_slot(slot);
    let _ = is_task_slot(slot);
    let _ = pid_slot_range();
});
