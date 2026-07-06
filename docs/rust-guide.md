# Rust Development Guide — GergiOS

> **Last updated**: July 2026
> **Related**: `planning/09_c_language_modernization.md` (Phases 3-6), `planning/19_ext4_driver_architecture.md` (Rust+C FFI pattern)

## Overview

GergiOS uses Rust for:
- **Userland utilities**: grep, basename, dirname, echo, true, false, yes, sleep, seq
- **Memory-safe components**: audio-buf (ring buffer), procfs-path (PID parsing), net-parse (TCP/UDP/DNS), minix-rs (IPC bindings)
- **Kernel-adjacent drivers**: minix-ahci (AHCI storage), minix-pci (PCI bus), minix-driver (MMIO/port I/O wrappers)
- **Filesystem core**: ext4-core (pure Rust ext4 with jbd2 journal)
- **FFI bridge**: minix-alloc (GlobalAlloc → C malloc/free)

All Rust code lives under `rust/` in the project root, organized as a Cargo workspace.

---

## 1. Workspace Structure

```
rust/
  Cargo.toml                    ← Workspace root (resolver = "2")
  Makefile                      ← BSD Make entry for subdirs
  Makefile.inc                  ← BSD Make Rust build rules
  .gitignore                    ← Excludes target/

  ext4-core/                    ← ext4 filesystem (staticlib)
  mkfs_ext4/                    ← mkfs.ext4 dev utility (bin)
  minix-rs/                     ← MINIX syscall FFI bindings (no_std)
  minix-driver/                 ← Safe MMIO/port I/O wrappers
  minix-alloc/                  ← GlobalAlloc → C malloc bridge
  minix-ahci/                   ← AHCI driver pilot (staticlib)
  minix-pci/                    ← PCI bus driver pilot (staticlib)
  audio-buf/                    ← Ring buffer (no_std)
  procfs-path/                  ← PID/path parsing (no_std)
  net-parse/                    ← TCP/UDP/DNS parsers (no_std)
  grep/                         ← grep utility (bin)
  basename/ dirname/ echo/      ← Core utilities (bin)
  true/ false/ yes/ sleep/ seq/ ← Core utilities (bin)
  fuzz/                         ← Fuzz targets (cargo-fuzz)
```

### 1.1 Crate Types

| Type | Example | Cargo.toml |
|------|---------|------------|
| **Binary** (`bin`) | grep, basename | `[[bin]] name = "grep"` |
| **Static library** (`staticlib`) | ext4-core, minix-ahci | `crate-type = ["staticlib"]` |
| **Library** (`lib`, `rlib`) | minix-rs, minix-driver | `crate-type = ["lib"]` |
| **Fuzz target** | fuzz | `cargo-fuzz` setup |

---

## 2. Adding a New Rust Component

### 2.1 Simple Binary Utility

```bash
# 1. Create the crate:
cargo new rust/my-utility
cd rust/my-utility

# 2. Add to workspace:
# Edit rust/Cargo.toml → add "my-utility" to [workspace.members]

# 3. Create BSD Makefile:
cat > Makefile << 'EOF'
.include <bsd.prog.mk>
EOF

# 4. Build & test:
cd rust && cargo build -p my-utility
cd rust && cargo test -p my-utility
```

### 2.2 Static Library (for C FFI)

```bash
# 1. Create crate with staticlib output:
cargo new --lib rust/my-lib
cd rust/my-lib

# 2. Edit Cargo.toml:
cat >> Cargo.toml << 'EOF'
[lib]
crate-type = ["staticlib"]
EOF

# 3. Add extern "C" exports:
# See src/lib.rs example below

# 4. Add to workspace + build:
cd rust && cargo build --release -p my-lib
```

### 2.3 CMake Integration

For components that need linking with MINIX C code:

```cmake
# In the CMakeLists.txt of the consuming target:

# Find the Rust staticlib
find_library(MY_LIB my_lib
    PATHS "${CMAKE_CURRENT_BINARY_DIR}/../rust/target/release"
          "${PROJECT_SOURCE_DIR}/rust/target/release"
)

if(MY_LIB)
    target_link_libraries(my_minix_component PRIVATE ${MY_LIB})
    target_compile_definitions(my_minix_component PRIVATE HAS_MY_LIB=1)
else()
    target_compile_definitions(my_minix_component PRIVATE HAS_MY_LIB=0)
endif()
```

---

## 3. Rust-C FFI Patterns

### 3.1 Exporting from Rust

```rust
// rust/my-lib/src/lib.rs
use core::ffi::{c_char, c_int, c_void};

/// Safe wrapper: no panics cross the FFI boundary.
#[no_mangle]
pub unsafe extern "C" fn my_func(input: c_int) -> c_int {
    std::panic::catch_unwind(|| {
        // ... actual implementation ...
        0 // success
    })
    .unwrap_or(-1) // panic caught → error
}
```

### 3.2 Importing from C

```c
// C caller:
extern int my_func(int input);

void call_rust(void) {
    int result = my_func(42);
    if (result < 0) {
        printf("Rust function failed\n");
    }
}
```

### 3.3 Callbacks (Rust calling C)

```rust
// Rust side: C callback type
type ReadBlockFn = unsafe extern "C" fn(
    ctx: *mut c_void,
    block_nr: u64,
    buf: *mut u8,
    block_size: u32,
) -> c_int;

// Usage in Rust:
pub fn read_with_callback(
    cb: ReadBlockFn,
    ctx: *mut c_void,
    block_nr: u64,
) -> Result<Vec<u8>, Error> {
    let mut buf = vec![0u8; 4096];
    let ret = unsafe { cb(ctx, block_nr, buf.as_mut_ptr(), 4096) };
    if ret < 0 {
        return Err(Error::Io);
    }
    Ok(buf)
}
```

### 3.4 Safety Rules

1. **All `extern "C"` functions must catch panics** — use `std::panic::catch_unwind`
2. **All shared structs must be `#[repr(C)]`** — ensure C ABI layout
3. **No `&[T]` in FFI** — pass pointer + length separately
4. **No `Box<T>` to C** — use raw pointers + manual allocation
5. **Ownership**: C owns raw memory, Rust borrows it temporarily
6. **Strings**: use `*const c_char` + `CStr` on Rust side

### 3.5 Real Example: ext4-core FFI

```rust
// rust/ext4-core/src/ffi.rs (simplified)
use core::ffi::{c_char, c_int, c_void};

pub type ext4_read_block_cb = unsafe extern "C" fn(
    ctx: *mut c_void,
    block_nr: u64,
    buf: *mut u8,
    block_size: u32,
) -> c_int;

pub type ext4_write_block_cb = unsafe extern "C" fn(
    ctx: *mut c_void,
    block_nr: u64,
    buf: *const u8,
    block_size: u32,
) -> c_int;

#[no_mangle]
pub unsafe extern "C" fn ext4_mount(
    ctx: *mut c_void,
    read_block: Option<ext4_read_block_cb>,
    sbi: *mut Ext4SbInfo,
) -> c_int {
    std::panic::catch_unwind(|| {
        let sbi = unsafe { &mut *sbi };
        let reader = make_block_reader(ctx, read_block.unwrap());
        match mount_ext4(sbi, reader) {
            Ok(()) => 0,
            Err(e) => e.to_errno(),
        }
    }).unwrap_or(-EINTR)
}
```

---

## 4. Cross-Compilation

### 4.1 Target Spec

Rust does not have an official `x86_64-unknown-minix` target. A custom JSON spec is provided:

```json
// rust/x86_64-unknown-minix.json
{
    "arch": "x86_64",
    "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
    "executables": true,
    "linker-flavor": "gcc",
    "linker": "x86_64-elf64-minix-gcc",
    "llvm-target": "x86_64-unknown-unknown",
    "os": "minix",
    "target-endian": "little",
    "target-pointer-width": "64",
    "panic-strategy": "abort",
    "features": "-mmx,-sse,-sse2,-sse3,-ssse3,-sse4.1,-sse4.2,-avx,-avx2"
}
```

### 4.2 Cross-Compiling

```bash
# Set up toolchain:
export MINIX_TOOLCHAIN=/opt/minix/toolchain
export MINIX_DESTDIR=/opt/minix/destdir

# Build:
cd rust/ext4-core
RUSTFLAGS="-C linker=${MINIX_TOOLCHAIN}/bin/x86_64-elf64-minix-gcc" \
cargo build --release --target ../x86_64-unknown-minix.json

# Output:
# rust/ext4-core/target/x86_64-unknown-minix/release/libext4_core.a
```

### 4.3 Using `build_ext4.sh`

```bash
# Simplified cross-compile:
./releasetools/build_ext4.sh cross x86_64
./releasetools/build_ext4.sh cross aarch64
```

---

## 5. Testing

### 5.1 Unit Tests

```bash
# Single crate:
cd rust && cargo test -p ext4-core

# All crates:
cd rust && cargo test

# Specific test:
cd rust && cargo test -p ext4-core -- journal::tests::recover_clean
```

### 5.2 Fuzz Testing

```bash
# Run all fuzz targets:
cd rust && cargo fuzz run -p fuzz fuzz_target_1 -- -runs=100000

# Available targets: minix_message, ring_buffer, pid_path,
#   tcp_segment, udp_datagram, dns_message
```

### 5.3 CTest Integration (via CMake)

```cmake
# In CMakeLists.txt:
add_rust_test(ext4-core)   # runs `cargo test -p ext4-core`
add_rust_test(grep)        # runs `cargo test -p grep`
```

```bash
# Run all Rust tests via CTest:
cd build && ctest -R rust
```

---

## 6. Conventions

### 6.1 Cargo.toml

```toml
[package]
name = "my-utility"
version = "0.1.0"
edition = "2024"

# For no_std crates:
[lib]
crate-type = ["lib"]

[dependencies]
# MINIX crates use path dependencies within the workspace
```

### 6.2 Edition

All crates use **Rust 2024 edition** (stable since Rust 1.85.0). Key differences from 2021:
- `unsafe extern` blocks required for FFI
- `static mut` references denied by default
- `unsafe` attributes (`no_mangle`, `export_name`) require `unsafe`
- `unsafe_op_in_unsafe_fn` enabled by default

### 6.3 FFI Naming

| Pattern | Example | Description |
|---------|---------|-------------|
| `ext4_*` | `ext4_mount()`, `ext4_lookup()` | ext4 Rust core FFI |
| `EXT4_*` | `EXT4_ROOT_INO`, `EXT4_MAX_BLOCK_SIZE` | Constants |
| `ext4_*_cb` | `ext4_read_block_cb` | Callback types |
| `Ext4*` | `Ext4SbInfo`, `Ext4Inode` | FFI-safe structs |

### 6.4 Error Handling

```rust
// Return errno-compatible ints from FFI:
fn to_errno(e: Error) -> c_int {
    match e {
        Error::NotFound => -ENOENT,
        Error::NoSpace => -ENOSPC,
        Error::InvalidInput => -EINVAL,
        Error::Io => -EIO,
        // ...
    }
}
```

---

## 7. File Layout Reference

```
rust/
  Cargo.toml                     ← Workspace members
  Makefile                       ← BSD Make subdir
  Makefile.inc                   ← Rust build rules
  x86_64-unknown-minix.json      ← Custom target spec

  ext4-core/
    Cargo.toml                   ← [lib] crate-type = ["staticlib"]
    src/lib.rs, ffi.rs, ...      ← ~7,600 LOC
    tests/, benches/             ← 58 tests, 19 benches

  grep/                          ← POSIX grep (~600 LOC)
    Cargo.toml
    src/main.rs

  minix-rs/                      ← FFI bindings
    Cargo.toml                   ← [lib], no_std
    src/lib.rs                   ← Message, endpoint_t, syscall()

  minix-driver/                  ← MMIO/port I/O
    Cargo.toml                   ← [lib], no_std
    src/{lib,mmio,port}.rs

  minix-alloc/                   ← GlobalAlloc bridge
    Cargo.toml
    src/lib.rs                   ← FFI to malloc/free

  fuzz/                          ← Fuzz targets
    Cargo.toml
    fuzz_targets/*.rs            ← 6 targets
```
