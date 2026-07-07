//! C-compatible FFI exports for the BPF verifier.
//!
//! These functions can be called from C code (specifically `bpf_filter.c`)
//! to validate BPF filter programs before execution.  They are exported as
//! `extern "C"` with `no_mangle` for direct linking.

#![allow(unsafe_code)]

use crate::{BpfInsn, BPF_MAXINSNS};

/// Validate a BPF filter program.
///
/// # Safety
///
/// - `insns` must be either NULL or point to at least `count` valid
///   `BpfInsn` structures.
///
/// Returns 1 if the program is valid, 0 otherwise.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn packet_filter_validate(
    insns: *const BpfInsn,
    count: i32,
) -> i32 {
    if insns.is_null() || count <= 0 || (count as usize) > BPF_MAXINSNS {
        return 0;
    }

    // SAFETY: Caller guarantees `count` valid `BpfInsn` entries at `insns`.
    let slice = unsafe { core::slice::from_raw_parts(insns, count as usize) };

    if crate::bpf_validate(slice) { 1 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BpfInsn;

    fn stmt(code: u16, k: u32) -> BpfInsn {
        BpfInsn { code, jt: 0, jf: 0, k }
    }

    #[test]
    fn ffi_null_pointer() {
        unsafe {
            assert_eq!(packet_filter_validate(core::ptr::null(), 1), 0);
        }
    }

    #[test]
    fn ffi_zero_count() {
        let insn = stmt(0x06, 0);
        unsafe {
            assert_eq!(packet_filter_validate(&insn, 0), 0);
        }
    }

    #[test]
    fn ffi_negative_count() {
        let insn = stmt(0x06, 0);
        unsafe {
            assert_eq!(packet_filter_validate(&insn, -1), 0);
        }
    }

    #[test]
    fn ffi_too_many_insns() {
        let insns = [stmt(0x06, 0); 513];
        unsafe {
            assert_eq!(packet_filter_validate(insns.as_ptr(), (BPF_MAXINSNS + 1) as i32), 0);
        }
    }

    #[test]
    fn ffi_valid_program() {
        // BPF_STMT(BPF_RET+BPF_K, 17)
        let insn = stmt(0x06, 17);
        unsafe {
            assert_eq!(packet_filter_validate(&insn, 1), 1);
        }
    }

    #[test]
    fn ffi_invalid_program() {
        // BPF_STMT(BPF_ALU+BPF_DIV+BPF_K, 0) — division by zero
        let insns = [stmt(0x04 | 0x30, 0), stmt(0x06, 0)];
        unsafe {
            assert_eq!(packet_filter_validate(insns.as_ptr(), 2), 0);
        }
    }
}
