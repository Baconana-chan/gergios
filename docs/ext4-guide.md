# ext4 Build & Usage Guide — GergiOS

> **Last updated**: July 2026
> **Related**: `planning/19_ext4_driver_architecture.md` (architecture), `planning/03_migration_roadmap.md` §4 (roadmap)

## Overview

GergiOS uses **ext4** as its default filesystem (replacing the legacy Minix FS / MFS). The ext4 implementation is split into two layers:

1. **Rust core** (`rust/ext4-core/`) — pure Rust ext4 parser/writer with jbd2 journaling, metadata checksums, ACL, xattr, quota
2. **C FFI bridge** (`minix/fs/ext4/`) — thin C layer that connects the Rust core to MINIX's VFS via `libfsdriver`

This guide covers **practical usage**: building ext4 components, creating ext4 filesystems, and configuring release images.

---

## 1. Building ext4 Components

### 1.1 Prerequisites

- Rust toolchain (1.85+): `rustup install stable`
- For cross-compilation: MINIX toolchain + DESTDIR (see `releasetools/cmake-build.sh`)
- For release images: `fakeroot` + `e2fsprogs` (Linux host)

```bash
# Install host dependencies (Debian/Ubuntu):
sudo apt install fakeroot e2fsprogs
```

### 1.2 Build Script: `releasetools/build_ext4.sh`

The unified build script supports four modes:

```bash
# 1. Native build (for testing on host Linux/macOS):
./releasetools/build_ext4.sh native

# 2. Host tool (nbmkfs.ext4) for release image creation:
./releasetools/build_ext4.sh host

# 3. Cross-compile for MINIX x86_64:
export MINIX_TOOLCHAIN=/opt/minix/toolchain
export MINIX_DESTDIR=/opt/minix/destdir
./releasetools/build_ext4.sh cross x86_64

# 4. Cross-compile for MINIX ARM64:
./releasetools/build_ext4.sh cross aarch64

# 5. Clean artifacts:
./releasetools/build_ext4.sh clean
```

### 1.3 What Gets Built

| Command | Output | Purpose |
|---------|--------|---------|
| `native` | `rust/target/release/libext4_core.a` | Static library for host testing |
| `native` | `rust/target/release/mkfs.ext4` | Native mkfs.ext4 binary (debugging on host) |
| `host` | `build/tools/nbmkfs.ext4` | Host tool for release image creation |
| `cross x86_64` | `rust/target/x86_64-unknown-minix/release/libext4_core.a` | MINIX staticlib, copied to CMake build dir |

### 1.4 CMake Integration

The ext4 server (`minix/fs/ext4/`) is built via CMake. The build system auto-detects the Rust staticlib:

```cmake
# minix/fs/ext4/CMakeLists.txt:
# If libext4_core.a is found → link it (full ext4 support)
# If not found → build with EXT4_C_ONLY=1 (all FFI calls return ENOTSUP)
```

To build MINIX with ext4 support:

```bash
cmake --preset x86_64-debug
cmake --build --preset x86_64-debug
```

---

## 2. Creating ext4 Filesystems

### 2.1 On MINIX: `mkfs.ext4`

Once the ext4 server is running in MINIX, create ext4 filesystems with:

```bash
# Format a partition with default settings (4K blocks):
mkfs.ext4 /dev/c0d0p1

# With custom block size:
mkfs.ext4 -b 1024 /dev/c0d0p1
mkfs.ext4 -b 4096 /dev/c0d0p1

# Mount:
mount -t ext4 /dev/c0d0p1 /mnt
```

**What `mkfs.ext4` creates:**
- Single block group (~128MB max for 4K blocks)
- 4096-byte blocks (default)
- Extent trees (ext4 extents, no indirect blocks)
- Flexible block groups (flex_bg)
- Root directory (inode 2) + lost+found (inode 11)
- No journal (minimal implementation)
- UUID derived from timestamp

### 2.2 On Host: `nbmkfs.ext4`

The host tool `nbmkfs.ext4` creates empty ext4 filesystem images for use in release scripts:

```bash
# Build first:
./releasetools/build_ext4.sh host

# Create a 128MB ext4 image:
nbmkfs.ext4 -b 4096 output.ext4
```

`nbmkfs.ext4` determines the filesystem size from the file size (uses `lseek(SEEK_END)`). So create a file of the desired size first:

```bash
truncate -s 128M my_partition.ext4
nbmkfs.ext4 -b 4096 my_partition.ext4
```

---

## 3. Release Images: ext4 Partitions

### 3.1 How It Works

Release scripts (`x86_hdimage.sh`, `arm64_hdimage.sh`, `arm_sdimage.sh`) now create ext4 partition images instead of MFS. The process:

1. Files are extracted into `ROOT_DIR` from tarball sets
2. `create_ext4_fs_images()` in `image.functions` creates per-partition staging directories:
   - Root: everything except `usr/` and `home/`
   - USR: contents of `usr/`
   - HOME: contents of `home/`
3. Each staging dir is converted to an ext4 image using a strategy chain:

```
Strategy 1: fakeroot + mke2fs -d    ← BEST (preserves uid=0/gid=0)
  ↓ (if fakeroot or mke2fs not available)
Strategy 2: mke2fs -d               ← OK (files owned by build user)
  ↓ (if mke2fs not available)
Strategy 3: nbmkfs.ext4 + mount+copy ← FALLBACK (needs root)
```

4. Partition images are `dd`'d into the final disk image at the correct offsets
5. Global vars `_ROOT_SIZE`, `_USR_SIZE`, `_HOME_SIZE` are set (in 512-byte sectors)

### 3.2 Building a Release Image

```bash
# x86_64 HDD image (ext4 partitions):
./releasetools/x86_hdimage.sh

# ARM64 HDD image:
./releasetools/arm64_hdimage.sh

# ARM SD card image:
./releasetools/arm_sdimage.sh
```

### 3.3 Dependencies for Release Image Creation

| Tool | Package | Required? |
|------|---------|-----------|
| `fakeroot` | `apt install fakeroot` | Recommended (strategy 1) |
| `mke2fs` | `apt install e2fsprogs` | Recommended (strategy 1 & 2) |
| `nbmkfs.ext4` | `./build_ext4.sh host` | Fallback (strategy 3) |
| `mount` (loop) | `util-linux` | Fallback only (needs root) |

---

## 4. ext4 Driver Architecture (TL;DR)

```
MINIX VFS
    │  IPC messages (REQ_READ, REQ_LOOKUP, …)
    ▼
minix/fs/ext4/          ← C bridge (libfsdriver interface)
  main.c                ← fsdriver_task(&ext4_table)
  table.c               ← 29/35 fsdriver callbacks
  ffi_bridge.c          ← Calls Rust extern "C" functions
  ffi.h                 ← FFI declarations
    │
    ▼  (extern "C" FFI)
rust/ext4-core/         ← Pure Rust ext4 implementation
  src/
    ffi.rs              ← ~40 exported FFI functions
    superblock.rs       ← Parse/serialize superblock
    group_desc.rs       ← Group descriptors (32/64-bit)
    inode.rs            ← Inode + extent tree (read/write/truncate)
    extent.rs           ← Extent tree operations + merge/split
    dir.rs              ← Directory (linear + htree)
    alloc.rs            ← Block allocator (flex_bg)
    ialloc.rs           ← Inode allocator
    journal.rs          ← jbd2 journal (recovery, commit, checkpoint)
    xattr.rs            ← Extended attributes (in-inode + external)
    acl.rs              ← POSIX ACL parsing
    quota.rs            ← V2 dqblk quota manager
```

### Key FFI Functions

```c
int  ext4_mount(dev_t dev, struct ext4_sb_info *sbi);
int  ext4_lookup(struct ext4_sb_info *sbi, ino_t dir_ino,
                 const char *name, ino_t *ino_out);
int  ext4_read_file(struct ext4_sb_info *sbi, ino_t ino,
                    void *buf, size_t count, off_t pos);
int  ext4_write_file(struct ext4_sb_info *sbi, ino_t ino,
                     const void *buf, size_t count, off_t pos);
int  ext4_create(struct ext4_sb_info *sbi, ino_t dir_ino,
                 const char *name, mode_t mode, uid_t uid, gid_t gid);
int  ext4_mkdir(struct ext4_sb_info *sbi, ino_t dir_ino,
                const char *name, mode_t mode, uid_t uid, gid_t gid);
int  ext4_mkfs(int fd, uint32_t block_size, uint64_t blocks_count);
// ... ~40 total
```

---

## 5. Configuration

### 5.1 Boot Configuration

The ext4 module is loaded at position `mod11` (replacing the old MFS module):

- `etc/limine.conf` — `MODULE_PATH=boot:///mod11_ext4`
- `etc/system.conf` — `service ext4` block with `devc`, `num`, `priv`, `uid`, `gid` settings
- `distrib/sets/lists/minix-kernel/mi` — `mod11_ext4` in kernel sets

### 5.2 fstab

All system fstab entries use `ext4` as the filesystem type:

```
/dev/c0d0p1  /usr    ext4  rw  0  2
/dev/c0d0p2  /home   ext4  rw  0  2
```

MFS is preserved for the boot ramdisk (ephemeral, USB installer).

---

## 6. Testing

### 6.1 Rust Tests

```bash
# All ext4-core tests:
cd rust && cargo test -p ext4-core

# Specific area:
cd rust && cargo test -p ext4-core -- journal
cd rust && cargo test -p ext4-core -- superblock
```

**58 unit tests + 1 doc-test** covering: superblock parsing, group descriptors, inode table, extent tree (read/write/merge/split), directory (linear + htree), block allocator, inode allocator, jbd2 journal (recovery/commit/checkpoint), CRC-32C checksums, xattr, ACL, quota.

### 6.2 Benchmarks

```bash
cd rust && cargo bench -p ext4-core
```

19 benchmarks across all subsystems. Key results (FX-8150):
- Superblock parse: ~258 ns
- Extent lookup: ~48 ns
- CRC-32C 4KB: ~15 µs
- Journal SB parse: ~112 ns

### 6.3 ffsb (optional)

```bash
# Run ffsb benchmark on ext4 partition (MINIX):
ffsb /mnt/ext4
```

---

## 7. File Layout Reference

```
rust/ext4-core/                      ← Rust ext4 core (~7,600 LOC)
  Cargo.toml
  src/
    lib.rs, types.rs
    superblock.rs, group_desc.rs
    inode.rs, extent.rs, dir.rs
    block.rs, alloc.rs, ialloc.rs
    journal.rs
    xattr.rs, acl.rs, quota.rs
    ffi.rs, mkfs.rs
  tests/                             ← 58 tests
  benches/                           ← 19 benchmarks

rust/mkfs_ext4/                      ← Rust mkfs.ext4 binary (dev utility)
  src/main.rs
  Cargo.toml

minix/fs/ext4/                       ← C FFI bridge (~200 LOC)
  main.c, table.c
  ffi_bridge.c, ffi.h
  CMakeLists.txt

minix/usr.sbin/mkfs.ext4/           ← C wrapper for MINIX mkfs.ext4
  main.c
  CMakeLists.txt

releasetools/
  build_ext4.sh                     ← Build script (native/cross/host)
  image.functions                   ← create_ext4_fs_images() helper
  x86_hdimage.sh, arm64_hdimage.sh, arm_sdimage.sh
```
