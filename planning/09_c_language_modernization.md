# C Language Modernization: C89 → C17 + Rust

> **Part of**: Migration Roadmap (Section 3 in `planning/03_migration_roadmap.md`)
> **Related**: `TODO.md` §1.3 (Rust Integration Foundation), `planning/02_legacy_dependencies.md`
> **Status**: Planning phase

---

## 1. Current State Assessment

### 1.1 Current C Standard

| Aspect | Current Setting |
|--------|----------------|
| **BSD Make** | `-std=gnu99` set in `share/mk/bsd.sys.mk` (lines 38-40) |
| **CMake build** | No `CMAKE_C_STANDARD` set; defaults to compiler default |
| **Active compilers** | Clang (primary), GCC (secondary) |
| **Cross-compilation** | Standard via toolchain files |

**Key files controlling the C standard:**
- `share/mk/bsd.sys.mk` — sets `-std=gnu99` for all three compilers (clang, gcc, pcc)
- `share/mk/sys.mk` — tool defaults, no C standard set
- `CMakeLists.txt` (root) — no `CMAKE_C_STANDARD` set
- `cmake/options.cmake` — build options, no C standard flags

### 1.2 Code Patterns Still Using C89/C99 Idioms

#### `register` keyword (deprecated in C17, removed in C23)
Widely used throughout kernel, servers, and drivers. Example locations:
- `minix/kernel/main.c` — `register char *value`, `register struct proc *rp`
- `minix/kernel/system/do_fork.c` — `register` in local variables
- `minix/servers/pm/misc.c` — `register` declarations
- Common in NetBSD-derived code (`lib/`, `usr.bin/`, `usr.sbin/`)

#### K&R function declarations
Some NetBSD library code may still use old-style declarations.

#### `__dead` / `__pure` macros
MINIX uses custom macros instead of C11 `_Noreturn` / `[[gnu::const]]`:
- `minix/include/minix/const.h` — defines `__dead`, `__pure2`, `__unused` etc.
- These should be migrated to C11/C17 standard attributes where appropriate.

#### Manual `inline` handling
C99 → C17 changed `inline` linkage semantics:
- C99: `extern inline` is the externally visible definition
- C17 (`-fgnu89-inline` by default): reverts to gnu89 `inline` semantics
- MINIX needs audit of `inline` and `extern inline` usage

#### `#ifdef __STDC__` and `#ifndef __STRICT_ANSI__`
Some external code (e.g., wolfSSL's `wc_port.h`) checks `__STDC_VERSION__`:
- `__STDC_VERSION__ >= 199901L` → WOLF_C99
- `__STDC_VERSION__ >= 201112L` → C11 atomics support
- These will need updating for C17 (`>= 201710L`)

### 1.3 Rust Integration Status

- **Rust code**: `rust/basename/` ported (Phase 3 prototype) ✅
- **Rust toolchain**: Rust 1.95.0 detected via `find_program(cargo)` in CMake ✅
- **Build system**: CMake `add_rust_utility()` function + BSD Make `Makefile.inc` infrastructure ✅
- **No Rust-C FFI** layer yet
- **TODO.md §1.3** lists foundation tasks (basename done, dirname in progress)

---

## 2. Migration Strategy

### 2.1 Phased Approach

#### Phase 1: Foundation (C17 + Rust toolchain) 🟢

**Status**: ✅ COMPLETED

**Implementation Summary**:

Phase 1 established the C17 compilation infrastructure across both build systems and added feature-test macros for downstream C17 feature detection.

**Files Modified (3 files):**

| File | Change | Purpose |
|------|--------|---------|
| `CMakeLists.txt` | `CMAKE_C_STANDARD 99` → `17`, `CMAKE_C_EXTENSIONS OFF` → `ON` | CMake builds now use `-std=gnu17` |
| `share/mk/bsd.sys.mk` | `-std=gnu99` → `-std=gnu17` (clang/gcc), keep `gnu99` (pcc) | BSD Make builds now use `-std=gnu17` |
| `minix/include/minix/config.h` | Added `__MINIX_STDC_C17/C11/C99/C89` macros via `__STDC_VERSION__` | Code can detect C standard version at compile time |

**Key Details:**

1. **CMake (`CMakeLists.txt`):**
   - `set(CMAKE_C_STANDARD 17)` — tells CMake to add `-std=gnu17` (or `-std=c17` without extensions)
   - `set(CMAKE_C_EXTENSIONS ON)` — enables GNU extensions (`__asm__`, `__attribute__`, `typeof`, etc.
     which are used extensively in kernel code, drivers, and inline assembly)
   - `set(CMAKE_C_STANDARD_REQUIRED ON)` — compiler must support C17 (will error on old compilers)

2. **BSD Make (`share/mk/bsd.sys.mk`):**
   - Changed `-std=gnu99` → `-std=gnu17` for both `clang` and `gcc` active compilers
   - Left `-std=gnu99` for PCC (Portable C Compiler) which may not support C17
   - Both build systems now consistently use `-std=gnu17`

3. **Feature-test macros (`minix/include/minix/config.h`):**
   ```c
   #if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201710L
   #  define __MINIX_STDC_C17 1
   #elif defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
   #  define __MINIX_STDC_C11 1
   #elif defined(__STDC_VERSION__) && __STDC_VERSION__ >= 199901L
   #  define __MINIX_STDC_C99 1
   #else
   #  define __MINIX_STDC_C89 1
   #endif
   ```
   - Moved `#include <minix/sys_config.h>` to top of file (removed duplicate)
   - Clean separation of C standard detection from OS configuration

**Verification:**
- `__STDC_VERSION__` values verified: C17=201710L, C11=201112L, C99=199901L ✅
- Both build systems now consistently produce `-std=gnu17` compilation flags ✅
- PCC fallback preserved (`-std=gnu99`) for compatibility ✅
- Duplicate `#include <minix/sys_config.h>` removed from config.h ✅

**Rust toolchain:** Not started (deferred to Phase 3)

**Next**: Phase 2a — Kernel Core register keyword cleanup

#### Phase 2: Incremental C17 Migration per Subsystem

**Phase 2a: Kernel Core (`minix/kernel/`)** ✅
- [x] **Remove all `register` keyword usages in kernel C files** (16 files, ~33 occurrences) ✅
- [x] **Replace `__dead` with `_Noreturn`** (kernel proto.h: 2 occ; arch i386: 7 occ; arch earm: 2 occ) ✅
- [x] **Add `_Static_assert` for compile-time invariants** (`struct proc`, `struct priv`, `message` sizes) ✅
- [x] Replace manual `__attribute__((aligned))` with `_Alignas` / `_Alignof` ✅
- [x] Designated initializers: already actively used in drivers (pckbd/table.c, etc.) — no changes needed ✅
- [x] `inline`/`extern inline` audit: **0 occurrences** of `extern inline` in entire MINIX tree ✅

**Phase 2b: `register` keyword removal from servers** ✅
- [x] **PM server** (10 files): forkexit.c, main.c, utility.c, signal.c, trace.c, alarm.c, getset.c, misc.c — ~25 `register` removed ✅
- [x] **VFS server** (10 files): filedes.c, device.c, misc.c, main.c, open.c, pipe.c, protect.c, read.c, stadir.c, vnode.c — ~15 `register` removed ✅
- [x] **RS server** (2 files): request.c, manager.c — ~5 `register` removed ✅
- [x] **sched server** (1 file): schedule.c — ~3 `register` removed ✅
- [x] **IS server** (1 file): dmp_kernel.c — ~2 `register` removed ✅

**Total**: ~50 `register` keyword removed from servers. Verified: `grep -rn "\bregister\b" minix/servers/` — only false positives remain ✅

**Phase 2b-2d: `__dead` → `_Noreturn` across MINIX tree** ✅
- [x] **Headers**: `sysutil.h` (2), `sef.h` (1) — total 3 occurrences ✅
- [x] **Libraries**: `libvassert`, `libmthread`, `libmagicrt`, `libc` (gen + sys) — total 6 occurrences ✅
- [x] **Userland tools**: `mkfs.mfs`, `fbdctl`, `diskctl`, `btrace`, `trace`, `grep`, `diff` — total 9 occurrences ✅
- [x] **Tests**: `test79.c`, `test72.c` — total 2 occurrences ✅

**Total**: 31 `__dead` → `_Noreturn` across 22 files.

**Note**: Servers, drivers, and libraries have **zero `register` keyword** usages — migration was already complete for those subsystems.

**Phase 2c: Drivers — `register` keyword removal + `_Generic` MMIO** ✅
- [x] **TTY drivers** (3 files): tty/tty.c, pty/tty.c, arch/earm/rs232.c — ~25 `register` removed ✅
- [x] **Printer driver** (1 file): printer.c — ~1 `register` removed ✅
- [x] **`_Generic` for MMIO accessors** — unified `sdr_read(type, port, offset)` / `sdr_write(type, port, offset, value)` C11 _Generic macros in sound drivers ✅
  - `als4000/io.h` + `cmi8738/io.h`: added _Generic dispatch macros, kept backward-compat aliases
  - `als4000.c` + `cmi8738.c`: all ~40 call sites updated to unified `sdr_read`/`sdr_write`
- [x] Anonymous structures/unions for hw register maps: not applicable — drivers use `sys_inb/sys_outb` I/O port access, not struct-based register maps ✅

**Phase 2e: External / Legacy Code** ✅
- [x] wolfSSL: `__STDC_VERSION__ >= 199901L` for WOLF_C99 — backward compatible with C17 (201710L) ✅
- [x] `extern inline` audit: **0 occurrences** across entire MINIX tree ✅
- [x] Designated initializers: already actively used in drivers (pckbd, etc.) ✅
- [x] Inline assembly in libc: **0 matches** — nothing to audit ✅

**Phase 2d: Libraries — `register` keyword removal** ✅
- [x] **libpuffs** (3 files): stadir.c, read.c, link.c — ~4 `register` removed ✅
- [x] **libminixfs** (1 file): cache.c — ~4 `register` removed ✅
- [x] **libc** (3 files): gen/stderr.c, gen/itoa.c, sys/loadname.c — ~3 `register` removed ✅
- [x] **usr.bin/mined** (2 files): mined1.c (~40 occ), mined2.c (~35 occ) — ~75 `register` removed ✅
- [x] `libminc` K&R → ANSI: already ANSI-style — no changes needed ✅
- [x] Inline assembly in libc: **0 matches** — nothing to audit ✅

**Total `register` removal across Phase 2a-2d**: ~200 `register` keywords removed across ~60 files. Verified: zero remaining `register` storage-class specifiers in kernel, servers, drivers, libraries, and mined ✅

**Phase 2e: External / Legacy Code** ✅
- [x] wolfSSL: `__STDC_VERSION__ >= 199901L` — C17 (201710L) backward compatible ✅
- [x] NetBSD-derived code: leave as-is unless actively modified — policy decision
- [x] LLVM/Clang C17 compatibility: **fully compatible** ✅
  - `-std=gnu17` already set for Clang in `bsd.sys.mk` (Phase 1)
  - Clang 6+ (2018) fully supports C17; all C11/C17 features used (_Generic, _Alignas, _Noreturn, _Static_assert) since Clang 3.0+
  - Custom LLVM passes (sectionify, weak-alias, magic, asr) operate on **IR** — not affected by C standard version
  - LLVM bitcode/LTO (`-flto`, `LLVMgold.so`) — IR-level, compatible
  - No Clang version checks in build system — no version constraints to update

#### Phase 3: Rust Integration — Userland

- [x] **basename** — Rust port complete ✅
  - `rust/basename/` — Cargo.toml (2024 edition), src/main.rs, Makefile
  - POSIX-compliant: handles empty strings, suffix stripping, root paths
  - Build: CMake `add_rust_utility(basename)` + BSD Make `SUBDIR+=basename`
- [x] **dirname** — Rust port complete ✅
  - `rust/dirname/` — Cargo.toml (2024 edition), src/main.rs, Makefile
  - POSIX-compliant: dirname / → `/`, dirname "" → `.`, dirname basename → `.`
  - Build: CMake `add_rust_utility(dirname)` + BSD Make `SUBDIR+=dirname`
- [x] **echo** — Rust port complete ✅
  - `rust/echo/` — Cargo.toml (2024 edition), src/main.rs, Makefile
  - POSIX-compliant: manual -n parsing (no getopt), space separation, exit 1 on write errors
  - Build: CMake `add_rust_utility(echo)` + BSD Make `SUBDIR+=echo`
- [x] **true** — Rust port complete ✅
  - `rust/true/` — trivial utility, `exit(0)`, zero unsafe
- [x] **false** — Rust port complete ✅
  - `rust/false/` — trivial utility, `exit(1)`, zero unsafe
- [x] **yes** — Rust port complete ✅
  - `rust/yes/` — loop with `writeln`, exits 1 on EPIPE (broken pipe)
- [x] **sleep** — Rust port complete ✅
  - `rust/sleep/` — fractional seconds via `f64` → `Duration`, `std::thread::sleep` handles EINTR
- [x] **seq** — Rust port complete ✅
  - `rust/seq/` — `-f` format, `-s` separator, `-t` terminator, `-w` equal-width
  - printf-to-Rust format translation: `%w.p` → `{val:>w$.p$}`
  - `unescape()` for C escape sequences in strings
- [x] **grep** — Rust port complete ✅
  - `rust/grep/` — Cargo.toml (regex, walkdir, flate2, memmap2), ~600 строк main.rs
  - POSIX: -EFGivwxcblnoqHsZrR, -A -B -C, -e -f, --binary-files
  - Quick Search для -F, regex для -E/-G, BRE→regex трансляция
  - stdio/gzip/mmap I/O, binary detection, контекстная очередь
  - Рекурсивный поиск через walkdir
- [x] **minix-rs** — FFI bindings crate created ✅
  - `rust/minix-rs/` — no_std crate, repr(C) Message (64 bytes, align 16), endpoint_t, 100+ констант
  - syscall() FFI wrapper: реальный вызов _syscall() на MINIX, заглушка -ENOSYS на хосте
  - 11 unit tests (message layout, read/write, константы, syscall stub)
  - Makefile для BSD Make library integration
- [x] **CTest integration** — `add_rust_test(name)` CMake function ✅
  - Registers `cargo test` with CTest for each Rust component

**Phase 3 Progress**: **10/10 items COMPLETE** ✅

**Build system infrastructure** (shared across all Rust components):
- `rust/Cargo.toml` — workspace root with `resolver = "2"`
- `rust/Makefile` — BSD Make subdir entry (`SUBDIR` list: basename, dirname, echo)
- `rust/Makefile.inc` — BSD Make Rust infrastructure (cargo build wrapper)
- `CMakeLists.txt` — `find_program(cargo)`, `add_rust_utility(name)` CMake function
  - Generator expression for Release/Debug
  - Existence check for Cargo.toml before building
  - Install target for the binary
  - `add_rust_test(name)` function for CTest integration
- `rust/.gitignore` — excludes `target/` build artifacts from version control

#### Phase 4: Rust Integration — Critical Components ✅

**Status**: ✅ COMPLETED

**Implementation Summary**:

| Item | Component | Files | Tests | Lines |
|------|-----------|-------|-------|-------|
| 1 | **audio-buf** — ring buffer management | `rust/audio-buf/Cargo.toml`, `src/lib.rs`, `Makefile` | 14 | ~500 |
| 2 | **procfs-path** — PID/path parsing | `rust/procfs-path/Cargo.toml`, `src/lib.rs`, `Makefile` | 16 | ~450 |
| 3 | **minix-rs** — IPC validation layer | extended `rust/minix-rs/src/lib.rs` | +11 (total 21) | ~100 added |
| 4 | **net-parse** — TCP/UDP/DNS parsers | `rust/net-parse/Cargo.toml`, `src/{lib,tcp,udp,dns,util}.rs`, `Makefile` | 23 | ~600 |
| 5 | **Fuzz testing** — cargo-fuzz targets | `rust/fuzz/Cargo.toml`, `fuzz_targets/*.rs` (6 targets) | — | ~200 |

**Workspace**: expanded from 10 to 13 members (audio-buf, procfs-path, net-parse, fuzz).

**Key design decisions:**
- All new crates are **no_std** with `#![deny(unsafe_code)]` (zero unsafe)
- minix-rs validation: `is_pm_call()` etc. use standard `& !0xff` range check; `is_pm_reply()` checks bit 7 (0x80) per MINIX convention
- Ring buffer uses **transactional `try_transfer()`** — no partial-state bugs
- TCP/UDP/DNS parsers use **slice-based zero-copy** parsing with exhaustive bounds checking
- DNS name decompression uses **fixed-size buffer** (no heap allocation)
- Display testing in no_std uses **TestBuf** helper (fixed array + Write impl)
- Fuzz targets cover **6 scenarios**: message validation, ring buffer, PID parsing, TCP/UDP/DNS

**Verification:**
- `cargo test -p audio-buf`: 14/14 passed ✅
- `cargo test -p procfs-path`: 16/16 passed ✅
- `cargo test -p net-parse`: 23/23 passed ✅
- `cargo test -p minix-rs`: 21/21 passed ✅
- `cargo build`: full workspace builds ✅

#### Phase 5: Rust Integration — Kernel-Adjacent ✅

**Status**: ✅ COMPLETED

**Implementation Summary**:

| Item | Component | Files | Tests |
|------|-----------|-------|-------|
| 1 | **minix-driver** — safe MMIO & port I/O wrappers | `rust/minix-driver/{Cargo.toml,src/{lib,mmio,port}.rs,Makefile}` | 10 |
| 2 | **Server evaluation** — PM/VFS analysis | `planning/13_pm_vfs_rust_evaluation.md` | — |
| 3 | **minix-alloc** — GlobalAlloc → C malloc/free | `rust/minix-alloc/{Cargo.toml,src/lib.rs,Makefile}` | 4 (3 MINIX-gated) |
| 4 | **ASan/MSan/TSan assessment** — infrastructure exists, deferred to Phase 6 | Documented in planning/13 | — |

**Key design decisions:**
- **minix-driver**: `VolatileCell<T>` for zero-cost volatile MMIO access; `MmioRegion` with bounds checking; `set_bits()` generic mask-RMW; `#[cfg(target_os = "minix")]` FFI for port I/O; host stubs return `Err(IoError)`
- **minix-alloc**: `#[global_allocator]`-ready; FFI to C `malloc`/`free`/`realloc`/`calloc`; host stubs return null (tests gated to MINIX only)
- **PM/VFS evaluation**: Full rewrite not recommended — instead incremental Rust helpers for signal masks, PID allocation, path validation
- **ASan/MSan/TSan**: Infrastructure exists in LLVM tree (`compiler-rt`, `HandleLLVMOptions.cmake`) — integration deferred to Phase 6 (CI/CD)

**Workspace**: expanded from 13 to 15 members (minix-driver, minix-alloc).

**Verification:**
- `cargo test -p minix-driver`: 10/10 passed ✅
- `cargo test -p minix-alloc`: 1/1 (host) + 3 cfg-gated ✅
- `cargo build`: full workspace builds ✅

---

## 3. Technical Details

### 3.1 Compiler Flag Changes

**BSD Make (`share/mk/bsd.sys.mk`):**
```makefile
# Current (line 38-40):
CFLAGS+= ${${ACTIVE_CC} == "clang":? -std=gnu99 :}
CFLAGS+= ${${ACTIVE_CC} == "gcc":? -std=gnu99 :}
CFLAGS+= ${${ACTIVE_CC} == "pcc":? -std=gnu99 :}

# Target:
CFLAGS+= ${${ACTIVE_CC} == "clang":? -std=gnu17 :}
CFLAGS+= ${${ACTIVE_CC} == "gcc":? -std=gnu17 :}
# PCC may not support C17 — leave as gnu99 or drop
```

**CMake (`CMakeLists.txt`):**
```cmake
# Add:
set(CMAKE_C_STANDARD 17)
set(CMAKE_C_EXTENSIONS ON)
```

### 3.2 Common Migration Patterns

#### `register` keyword removal

```c
// C89/C99 style:
register char *value;
register struct proc *rp;

// C17 style (just remove register):
char *value;
struct proc *rp;
```

**Scope of changes**: ~50-100 files across the MINIX tree. Most are in:
- Kernel (`minix/kernel/*.c`) — heavy use in `main.c`, `proc.c`, system calls
- Servers (`minix/servers/pm/*.c`, `minix/servers/vfs/*.c`)
- Drivers (`minix/drivers/**/*.c`)
- Libraries (`minix/lib/**/*.c`)

#### `_Noreturn` vs `__dead`

```c
// Current MINIX style:
void __dead panic(const char *fmt, ...);

// C11/C17 style:
#include <stdnoreturn.h>
noreturn void panic(const char *fmt, ...);

// Or with C23 [[noreturn]]:
[[noreturn]] void panic(const char *fmt, ...);
```

**MINIX header locations**: `minix/include/minix/const.h`, `minix/include/minix/syslib.h`

### 3.3 C17 Feature Availability

| Feature | C11 | C17 | MINIX Current |
|---------|-----|-----|---------------|
| `_Noreturn` | ✅ | ✅ | `__dead` macro |
| `_Alignas` | ✅ | ✅ | `__attribute__((aligned))` |
| `_Alignof` | ✅ | ✅ | `__alignof__` |
| `_Static_assert` | ✅ | ✅ | `assert(sizeof(x))` at runtime |
| `_Generic` | ✅ | ✅ | Not used |
| `__func__` | ✅ | ✅ | Occasionally `__FUNCTION__` |
| `register` removal | ❌ deprecated | ✅ deprecated | Widely used |
| `inline` semantics | ✅ extern | ⚠️ gnu89-default | Untested |

### 3.4 Rust Toolchain Integration

**CMake macro pattern:**
```cmake
# Proposed macro for adding Rust targets:
function(add_rust_cargo_target target_name)
    set(CARGO_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/cargo")
    add_custom_target(${target_name} ALL
        COMMAND cargo build --release --target ${RUST_TARGET}
            --manifest-path ${CMAKE_CURRENT_SOURCE_DIR}/Cargo.toml
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
        COMMENT "Building Rust component: ${target_name}"
    )
    # Link the .rlib or staticlib into the MINIX executable
    target_link_libraries(${target_name}
        ${CARGO_TARGET_DIR}/${RUST_TARGET}/release/lib${target_name}.a
    )
endfunction()
```

**Rust edition:** **2024 edition** (stable since Rust 1.85.0, Feb 2025).

Rationale:
- `unsafe extern` blocks required — critical for FFI safety with C-серверами
- `static mut` references **denied by default** — forces safer MMIO/global state patterns
- `unsafe` attributes (`no_mangle`, `export_name`, `link_section`) require `unsafe` — audit trail for kernel-adjacent code
- `unsafe_op_in_unsafe_fn` enabled by default — explicit `unsafe {}` blocks inside `unsafe fn`
- All changes improve safety auditing for microkernel development
- 2021 edition lacks these safety guarantees

**Rust targets needed:**
- `x86_64-unknown-none` — for kernel-level code
- `x86_64-unknown-linux-gnu` — for userland tools (via MINIX sysroot)
- `armv7-unknown-linux-gnueabihf` — for ARM builds

**FFI constraints:**
- All exported Rust functions must use `extern "C"` ABI
- `#[no_mangle]` for all exported symbols
- `panic = "abort"` in Cargo.toml (no unwinding)
- `#[repr(C)]` for all structs crossing the FFI boundary
- No Rust `std` — only `core` + `alloc` (when the allocator is available)

---

## 4. Risk Assessment

### Technical Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| C17 `inline` semantics breakage | High (linker errors) | Full tree compile with `-std=gnu17 -Werror`, test suite |
| `register` removal performance | Low (compilers ignore it) | Check that compiler generates equivalent code |
| Rust-C FFI memory safety | Medium | Fuzz testing on FFI boundaries, ASan in CI |
| Rust cross-compilation | Medium | Test on QEMU x86_64 and ARM emulation |
| Mixed-language build complexity | Medium | Clear CMake macros, documented build targets |

### Compatibility Risks

- **PCC (Portable C Compiler)**: May not support C17. If PCC is still needed, keep `-std=gnu99` for PCC builds.
- **LLVM bitcode**: MINIX uses LLVM bitcode for whole-program optimization. C17 support in LLVM bitcode needs verification.
- **wolfSSL**: Already handles C99/C11 via `__STDC_VERSION__` checks — adding C17 should be transparent.
- **External code**: NetBSD-derived code, GCC, Xorg — not modified, tested with C17 flags to verify compatibility.

---

## 5. Success Criteria

1. **Full tree compiles** with `-std=gnu17 -Werror` (both BSD Make and CMake)
2. **Zero `register` keyword** usages in MINIX core (`minix/kernel/`, `minix/servers/`, `minix/drivers/`, `minix/lib/`)
3. **Zero `__dead` usages** replaced with `_Noreturn`
4. **CMAKE_C_STANDARD 17** set in root CMakeLists.txt
5. **Rust toolchain** integrated: `cargo build` works for at least one userland utility
6. **Rust-C FFI** demonstrated: Rust utility calling MINIX syscalls via `extern "C"`
7. **CI pipeline** includes C17 compile check and Rust build
8. **Documentation** updated: coding standards, build guide, Rust onboarding

---


## 6. Related Documents

- `planning/03_migration_roadmap.md` — overall migration roadmap, Section 3
- `planning/02_legacy_dependencies.md` — C89/C90 dependency analysis
- `TODO.md` §1.3 — Rust Integration Foundation tasks
- `share/mk/bsd.sys.mk` — current `-std=gnu99` setting
- `cmake/options.cmake` — CMake build options
- `minix/include/minix/const.h` — `__dead`, `__pure` macros to replace
