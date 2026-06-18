# PM & VFS Rust Rewrite Evaluation

> **Phase**: 5 (Kernel-Adjacent)
> **Status**: Evaluation Complete
> **Date**: June 2026

## 1. Executive Summary

After analyzing PM (`minix/servers/pm/`) and VFS (`minix/servers/vfs/`), a
**full Rust rewrite is not recommended at this stage**. Both servers are:

- **Deeply coupled** to the MINIX IPC framework (SEF, chardriver, syscalls)
- **~40KB+ each** of C code with complex state machines
- **Tightly integrated** with kernel data structures via `#include "kernel/*.h"`

Instead, the recommendation is **incremental Rust adoption** of specific
safety-critical subsystems within PM/VFS.

## 2. PM (Process Manager) Analysis

### Size & Complexity

| Aspect | Value |
|--------|-------|
| Files | ~25 `.c` + ~15 `.h` |
| Lines of code | ~10,000 |
| Main loop | `while(TRUE) { sef_receive → dispatch → reply }` |
| Calls | 48 system calls (`NR_PM_CALLS`) |
| State | Per-process `mproc[]` table with flags, signals, timers |

### Critical Subsystems

| Subsystem | Rust Feasibility | Effort | Priority |
|-----------|-----------------|--------|----------|
| Signal mask management | **High** | Small | 🔴 High |
| Process state flags | **High** | Small | 🔴 High |
| PID allocation | **Medium** | Medium | 🟡 Medium |
| Timers/alarms | **Medium** | Medium | 🟡 Medium |
| Core IPC loop | **Low** | Very Large | ⚪ Low |

### Recommended Rust Candidates

1. **Signal mask ops** (`sigemptyset`, `sigaddset`, `sigismember`):
   - Pure bit manipulation, no IPC dependency
   - Easy to verify correctness
   - Can be a no_std helper crate

2. **PID bitmap allocator**:
   - Currently uses linear search in `get_free_pid()`
   - Rust `bitvec` or custom bitmap is trivially safe
   - Can be a no_std crate with property-based testing

## 3. VFS (Virtual File System) Analysis

### Size & Complexity

| Aspect | Value |
|--------|-------|
| Files | ~40 `.c` + ~20 `.h` |
| Lines of code | ~15,000 |
| Main loop | Multi-threaded worker pool with `worker_start/worker_yield` |
| Calls | 64 system calls (`NR_VFS_CALLS`) |
| State | Per-process `fproc[]`, per-mount `vmnt[]`, per-vnode `vnode[]` |

### Critical Subsystems

| Subsystem | Rust Feasibility | Effort | Priority |
|-----------|-----------------|--------|----------|
| Path validation/normalization | **High** | Small | 🔴 High |
| Permission checking | **High** | Small | 🔴 High |
| File descriptor table | **Medium** | Medium | 🟡 Medium |
| Lock management | **Medium** | Medium | 🟡 Medium |
| Worker thread pool | **Low** | Large | ⚪ Low |
| Device I/O dispatch | **Low** | Very Large | ⚪ Low |

### Recommended Rust Candidates

1. **Path validation**:
   - Check absolute vs relative paths, `..` traversal
   - MAX_PATH_LEN enforcement, null byte prevention
   - Pure string/navigation logic, no IPC
   - Already partially covered in `procfs-path`

2. **Permission check** (`allowed()`):
   - Compare uid/gid against file mode
   - Pure integer logic, no side effects
   - Trivially testable with property-based testing

## 4. Integration Strategy

```
┌─────────────────────────────────────────────┐
│                  PM / VFS                    │
│  ┌─────────────────┐  ┌──────────────────┐  │
│  │   C main loop   │  │  C main loop     │  │
│  │  (IPC dispatch) │  │  (worker pool)   │  │
│  └────────┬────────┘  └────────┬─────────┘  │
│           │                     │            │
│  ┌────────▼────────┐  ┌────────▼─────────┐  │
│  │  Rust helpers   │  │  Rust helpers    │  │
│  │  (signal, PID)  │  │  (path, perms)   │  │
│  └─────────────────┘  └──────────────────┘  │
└─────────────────────────────────────────────┘
```

Each Rust helper crate:
- Is `no_std` with `#![deny(unsafe_code)]`
- Gets linked as a `.a` static library via CMake
- Uses `extern "C"` ABI for the C→Rust boundary
- Has exhaustive property-based tests

## 5. ASan/MSan/TSan Assessment

### Current State

The MINIX build tree already includes LLVM sanitizer infrastructure:
- `external/bsd/llvm/dist/clang/runtime/` — compiler-rt with ASan/TSan/MSan
- `external/bsd/llvm/dist/llvm/cmake/` — `HandleLLVMOptions.cmake` with
  `-fsanitize=address`, `-fsanitize=memory`, `-fsanitize=undefined`
- `external/gpl3/gcc/lib/libasan/` — GCC ASan as well

### Implementation Status

Sanitizer support exists in the **LLVM build tree** but is not yet
integrated into the MINIX Rust CI pipeline. To enable:

1. Add `-fsanitize=address` to Rust `RUSTFLAGS` for FFI boundary code
2. Link with `compiler-rt` ASan runtime during MINIX build
3. Add `#[cfg(test)]` ASan-enabled test targets

### Recommendation

Defer full sanitizer integration to Phase 6 (CI/CD), where a dedicated
QEMU-based test runner can execute ASan-instrumented binaries.

## 6. Recommended Phase 5 Deliverables

| Deliverable | Status | Priority |
|-------------|--------|----------|
| MMIO safe wrappers (`minix-driver`) | ✅ Complete | Critical |
| Port I/O safe wrappers (`minix-driver`) | ✅ Complete | Critical |
| GlobalAlloc → C malloc/free (`minix-alloc`) | ✅ Complete | Critical |
| Rust-adjacent PM signal handling crate | 🔜 Planned | High |
| Rust-adjacent VFS path validation crate | 🔜 Planned | High |
| ASan CI integration | 📋 Deferred to Phase 6 | Medium |

## 7. Related Documents

- `planning/09_c_language_modernization.md` — Phase 5 overview
- `rust/minix-driver/` — Safe MMIO and port I/O wrappers
- `rust/minix-alloc/` — GlobalAlloc → C allocator bridge
- `TODO.md` — Overall project TODO list
