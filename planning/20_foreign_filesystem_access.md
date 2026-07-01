# Foreign Filesystem Access — GergiOS File Importer

> **Part of**: GergiOS Modernization Roadmap
> **Related**: `planning/19_ext4_driver_architecture.md`, `planning/11_gui_architecture.md`, `planning/01_microkernel_architecture.md`
> **Status**: Design phase
> **Language Strategy**: Rust (core) + C (block I/O bridge)

---

## 1. Executive Summary

**Vision**: GergiOS becomes a "live rescue OS" that can boot on any multi-boot system, scan all attached disks and partitions, automatically detect foreign filesystems (ext4, NTFS, FAT32, APFS), mount them read-only, and allow the user to browse and selectively copy files into GergiOS's own storage.

**Killer use case**:
1. Boot GergiOS from USB on a dual-boot (or multi-boot) machine
2. GergiOS scans all disks — finds Linux (ext4), Windows (NTFS), macOS (APFS)
3. User sees a unified virtual tree: `/foreign/linux/`, `/foreign/windows/`, `/foreign/macos/`
4. `cp /foreign/linux/home/user/file.txt /home/gergios/` — file is imported
5. At shutdown, optionally sync selected changes back

**Key design decisions**:
| Aspect | Decision | Rationale |
|--------|----------|-----------|
| **Disk access** | Raw block device (`/dev/c0d0`, not partition) | Need to parse MBR/GPT ourselves |
| **Partition parser** | Rust (pure, no C deps) | Portable, testable on any host OS |
| **Foreign FS mounting** | In-process Rust drivers | No need for kernel-side VFS changes |
| **Import mechanism** | Userspace copy (VFS-level) | Simple, safe, no cross-FS hardlink complexity |
| **Write to foreign FS** | ❌ Not in scope (Phase 1) | Read-only to avoid accidental corruption |

---

## 2. Architecture Overview

### 2.1 System Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                        GergiOS VFS                                │
│                                                                   │
│   ┌──────────────┐  ┌──────────────┐  ┌────────────┐           │
│   │  GergiOS FS   │  │   ext4 FS    │  │  FAT32 FS  │  ...      │
│   │  (нативный)   │  │  (наш Rust)  │  │  (будущий) │           │
│   └──────┬───────┘  └──────┬───────┘  └─────┬──────┘           │
│          │                  │                │                    │
│          └──────────┬───────┴────────────────┘                    │
│                     │                                             │
│               ┌─────┴──────┐                                      │
│               │  Block I/O  │  (libbdev / bdev_open + read)       │
│               └─────┬──────┘                                      │
│                     │                                             │
│              ┌──────┴──────┐                                      │
│              │   Disk Scan  │  ← NEW: Partition Scanner           │
│              │   (Rust)     │                                      │
│              └──────┬──────┘                                      │
│                     │                                             │
└─────────────────────┼─────────────────────────────────────────────┘
                      │
              ┌───────┴────────┐
              │   /dev/c0d0     │  ← physical disk (whole, not partition)
              │                 │
              │  ┌───────────┐  │
              │  │  MBR/GPT  │  │  ← partition table parser (Rust)
              │  ├───────────┤  │
              │  │ part 1    │  │  ← ext4 (Linux /boot)
              │  │ part 2    │  │  ← ext4 (Linux root)
              │  │ part 3    │  │  ← NTFS (Windows)
              │  │ part 4    │  │  ← FAT32 (ESP)
              │  │ part 5    │  │  ← APFS (macOS)
              │  └───────────┘  │
              └─────────────────┘
```

### 2.2 Component Stack

```
┌─────────────────────────────────────────────┐
│         gergios-fs-import (CLI tool)          │
│  scan, ls-foreign, import, mount-foreign     │
├─────────────────────────────────────────────┤
│         Foreign FS Drivers (Rust)             │
│  ┌──────┐ ┌───────┐ ┌───────┐ ┌──────┐     │
│  │ ext4 │ │ NTFS  │ │ FAT32 │ │ APFS │ ...  │
│  │ (✅) │ │ (todo)│ │ (todo)│ │ (todo)│     │
│  └──────┘ └───────┘ └───────┘ └──────┘     │
├─────────────────────────────────────────────┤
│         Partition Table Parser (Rust)         │
│  ┌────────────────┐ ┌──────────────────┐     │
│  │  MBR parser     │ │  GPT parser      │     │
│  │  (Legacy BIOS)  │ │  (UEFI, >2TB)   │     │
│  └────────────────┘ └──────────────────┘     │
├─────────────────────────────────────────────┤
│         Block I/O Layer (C + Rust FFI)        │
│  ┌──────────────────────────────────────┐    │
│  │  bdev_open(dev, BDEV_R_BIT)         │    │
│  │  bdev_read(dev, buf, count, pos)    │    │
│  │  bdev_close(dev)                    │    │
│  └──────────────────────────────────────┘    │
└─────────────────────────────────────────────┘
```

### 2.3 Language Strategy

| Component | Language | Rationale |
|-----------|----------|-----------|
| **Partition parser (MBR/GPT)** | Rust | Pure logic, no FFI needed, testable on host |
| **ext4 driver** | Rust | Already implemented (`ext4-core`) |
| **NTFS driver** | Rust | `ntfs` crate exists (pure Rust) |
| **FAT32 driver** | Rust | `fatfs` crate exists (pure Rust) |
| **APFS driver** | Rust | `apfs` crate exists (experimental) |
| **Block I/O bridge** | C + Rust FFI | Wraps MINIX `bdev_*` calls |
| **Import CLI tool** | Rust | Ties everything together |
| **Foreign mount daemon** | Rust | Background service for auto-mount |

---

## 3. Component Design

### 3.1 Partition Table Parser (Rust)

A pure Rust library (`gergios-partition`) that reads raw disk data and returns a list of partitions.

#### MBR Format (Legacy BIOS)

```
Offset  Size  Field
─────────────────────────────────
0       446   Boot code
446      16   Partition entry 1
462      16   Partition entry 2
478      16   Partition entry 3
494      16   Partition entry 4
510       2   Signature 0x55AA

Each partition entry:
Offset  Size  Field
─────────────────────────────────
0        1    Boot indicator (0x80 = active)
1        1    CHS start (head)
2        2    CHS start (sector/cylinder)
4        1    Type (0x83 = Linux, 0x07 = NTFS, 0x0B = FAT32, 0xEE = GPT protective)
5        1    CHS end (head)
6        2    CHS end (sector/cylinder)
8        4    LBA start (little-endian)
12       4    Sector count (little-endian)
```

#### GPT Format (UEFI)

```
Offset  Size  Field
────────────────────────────────────
0       512   Protective MBR (same as MBR, type 0xEE)
512       8   GPT signature "EFI PART"
520       4   Revision (1.0 = 0x00010000)
524       4   Header size (usually 92)
528       4   CRC-32C of header (offset 0..header_size-1)
532       4   Reserved (0)
536       8   Current LBA (usually 1)
544       8   Backup LBA (last sector of disk)
552       8   First usable LBA
560       8   Last usable LBA
568      16   Disk GUID
584       8   Partition entry array start LBA
592       4   Number of partition entries
596       4   Size of each partition entry (usually 128)
600       4   CRC-32C of partition entry array
604       *   Reserved (header_size - 92 bytes)

Each partition entry (128 bytes):
Offset  Size  Field
────────────────────────────────────
0       16   Partition type GUID
16      16   Unique partition GUID
32       8   Starting LBA
40       8   Ending LBA (inclusive)
48       8   Attributes
56      72   Name (UTF-16LE, 36 characters max)
```

#### Design

```rust
// gergios-partition/src/lib.rs

/// Detected partition type
pub enum PartitionType {
    Empty,
    Mbr(MbrPartition),
    Gpt(GptPartition),
}

/// Partition table parsing result
pub enum PartitionTable {
    Mbr {
        partitions: Vec<Partition>,
        signature: u16,  // should be 0x55AA
    },
    Gpt {
        header: GptHeader,
        partitions: Vec<GptPartitionEntry>,
        backup_lba: u64,
    },
    None,  // No valid partition table found
}

/// Generic partition (common fields)
pub struct Partition {
    pub index: usize,
    pub start_lba: u64,
    pub sector_count: u64,
    pub fs_type: FilesystemType,
    pub label: Option<String>,
}

/// Detected filesystem type
pub enum FilesystemType {
    Ext2, Ext3, Ext4,
    Ntfs,
    Fat12, Fat16, Fat32,
    Apfs,
    Swap,
    Unrecognized(u8),  // MBR type byte
}

/// Main parser
pub fn parse_partition_table(data: &[u8]) -> PartitionTable;
pub fn scan_disk(block_size: u64, read_block: &mut dyn FnMut(u64) -> Vec<u8>) -> ScanResult;
```

### 3.2 Foreign FS Detection (Auto-probe)

In addition to partition table types, probe each partition for FS signatures:

| Filesystem | Signature | Offset |
|------------|-----------|--------|
| **ext2/3/4** | 0xEF53 | SB offset 1080 (byte 56 of SB, LBA 0 with block_size=1024) |
| **NTFS** | "NTFS    " | byte 3 of boot sector |
| **FAT32** | "MSWIN4.1" or "FAT32   " | byte 3 of boot sector (usually) |
| **exFAT** | "EXFAT   " | byte 3 of boot sector |
| **APFS** | "NXSB" | byte 0 of APFS container superblock (usually at LBA 2) |
| **ZFS** | "BHYEBHYE" or ... | varies |
| **Btrfs** | "_BHRfS_M" | byte 65600 (offset 0x10040) |

Probe order: check partition table type first, then fall back to signature scanning for unknown types. This allows detection even on disks with corrupted partition tables.

### 3.3 Foreign FS Drivers

#### ext4 ✅ (Already implemented in `ext4-core`)

Full read/write support exists. For foreign FS access, we mount read-only by default.

```
rust/ext4-core/src/
  ├── lib.rs          ← Public API
  ├── superblock.rs   ← SB parsing
  ├── group_desc.rs   ← Group descriptors
  ├── inode.rs        ← Inode + extent tree
  ├── extent.rs       ← Extent traversal
  ├── dir.rs          ← Directory + htree
  ├── block.rs        ← Block addressing
  ├── alloc.rs        ← Block allocator
  ├── ialloc.rs       ← Inode allocator
  ├── journal.rs      ← JBD2 recovery
  ├── xattr.rs        ← Extended attributes
  ├── acl.rs          ← POSIX ACLs
  ├── quota.rs        ← Quota management
  ├── types.rs        ← Shared types
  └── ffi.rs          ← extern "C" for C bridge
```

#### NTFS (Rust — `ntfs` crate)

The [`ntfs`](https://crates.io/crates/ntfs) crate provides:
- Pure Rust NTFS parser (no C dependencies)
- Read support for $MFT, attributes, directories, files
- Support for compressed and sparse files
- `ntfs-shell` examples for reference

**What we need to build on top:**
```rust
// rust/gergios-ntfs/src/lib.rs
pub struct NtfsFilesystem { /* ... */ }

impl NtfsFilesystem {
    pub fn mount(block_size: u64, read_block: Box<dyn FnMut(u64, &mut [u8])>) -> Result<Self>;
    pub fn lookup(&self, dir_ino: u64, name: &str) -> Result<u64>;
    pub fn read(&self, ino: u64, buf: &mut [u8], offset: u64) -> Result<usize>;
    pub fn readdir(&self, dir_ino: u64) -> Result<Vec<DirEntry>>;
    pub fn stat(&self, ino: u64) -> Result<FileStat>;
}
```

Estimated: ~400-600 LOC wrapper around `ntfs` crate.

#### FAT32 (Rust — `fatfs` crate)

The [`fatfs`](https://crates.io/crates/fatfs) crate provides:
- Pure Rust FAT12/16/32 support
- Read + write support
- Long filename (LFN)
- No_std compatible

**What we need to build on top:**
```rust
// rust/gergios-fatfs/src/lib.rs
pub struct FatFilesystem { /* ... */ }

impl FatFilesystem {
    pub fn mount(block_size: u64, read_block: /* ... */) -> Result<Self>;
    pub fn lookup(&self, dir_ino: u64, name: &str) -> Result<u64>;
    pub fn read(&self, ino: u64, buf: &mut [u8], offset: u64) -> Result<usize>;
    pub fn readdir(&self, dir_ino: u64) -> Result<Vec<DirEntry>>;
    pub fn stat(&self, ino: u64) -> Result<FileStat>;
}
```

Estimated: ~300-400 LOC wrapper around `fatfs` crate.

#### APFS (Rust — experimental)

The [`apfs`](https://crates.io/crates/apfs) crate (or similar) provides:
- Apple File System parsing
- Container and volume support
- Limited read support

**Status**: Experimental, not production-ready. Deferred to Phase 2.

#### Unified Foreign FS Trait

All foreign FS drivers implement a common trait:

```rust
/// Unified interface for any foreign filesystem
pub trait ForeignFilesystem: Send {
    /// Human-readable name (e.g., "ext4", "NTFS", "FAT32")
    fn fs_name(&self) -> &'static str;

    /// Label/volume name, if available
    fn volume_label(&self) -> Option<&str>;

    /// Total capacity in bytes
    fn total_space(&self) -> u64;

    /// Available space in bytes
    fn free_space(&self) -> u64;

    /// Look up a directory entry by name
    fn lookup(&self, dir_id: FsId, name: &str) -> Result<FsId>;

    /// Read file contents at offset
    fn read(&self, file_id: FsId, offset: u64, buf: &mut [u8]) -> Result<usize>;

    /// Read directory entries
    fn readdir(&self, dir_id: FsId) -> Result<Vec<DirEntry>>;

    /// Get file/directory metadata
    fn stat(&self, id: FsId) -> Result<FileStat>;
}

/// Opaque identifier for a file/directory within a filesystem
pub struct FsId(u64);

/// Directory entry
pub struct DirEntry {
    pub name: String,
    pub id: FsId,
    pub file_type: FileType,  // File, Dir, Symlink, etc.
}

/// File metadata
pub struct FileStat {
    pub size: u64,
    pub mode: u32,
    pub uid: u32,
    pub gid: u32,
    pub atime: u64,
    pub mtime: u64,
    pub ctime: u64,
    pub is_dir: bool,
}
```

### 3.4 Block I/O Layer

Two levels of block access:

#### Level 1: Raw disk (whole device)

```c
// Open the whole disk (not a partition)
int disk_fd = bdev_open("/dev/c0d0", BDEV_R_BIT);  // read-only

// Read sectors at raw LBA
bdev_read(disk_fd, buf, SECTOR_SIZE * count, sector_lba * SECTOR_SIZE);

// Close
bdev_close(disk_fd);
```

This is used by the partition table parser to read MBR (LBA 0) and GPT (LBA 1).

#### Level 2: Partition-relative block access

Once a partition is detected, we compute the partition offset and create a block reader that translates partition-relative blocks to absolute disk LBAs:

```rust
pub fn make_partition_reader(
    disk_dev: i32,
    part_start_lba: u64,
    block_size: u32,
) -> impl FnMut(u64, &mut [u8]) -> bool {
    move |block_nr: u64, buf: &mut [u8]| {
        let abs_lba = part_start_lba + block_nr;
        let abs_offset = abs_lba * 512;  // sector_size
        // bdev_read(disk_fd, buf, block_size, abs_offset) == OK
    }
}
```

This is passed directly to existing FS drivers (ext4, NTFS, FAT32) which expect `read_block(block_nr, buf) -> bool` callbacks.

### 3.5 Discovery Service / Mount Daemon

A background service (`gergios-fs-discovery`) that:

1. **Scans block devices** — enumerates `/dev/c0d0`, `/dev/c0d1`, etc. (all attached disks)
2. **Parses partition tables** — MBR and/or GPT on each disk
3. **Probes for filesystems** — checks signatures on each partition
4. **Mounts read-only** — creates in-memory FS handles
5. **Publishes a virtual tree** — at `/foreign/`:

```
/foreign/
├── disk-0/
│   ├── gpt-table/
│   ├── partition-1/
│   │   ├── _fs_type: ext4
│   │   ├── _label: "Linux Root"
│   │   ├── boot/
│   │   ├── home/
│   │   └── ...
│   ├── partition-2/
│   │   ├── _fs_type: ntfs
│   │   ├── _label: "Windows"
│   │   └── ...
│   ├── partition-3/
│   │   ├── _fs_type: fat32
│   │   ├── _label: "ESP"
│   │   └── ...
│   └── partition-5/
│       ├── _fs_type: apfs
│       ├── _label: "Macintosh HD"
│       └── ...
├── disk-1/
│   └── partition-1/
│       ├── _fs_type: ext4
│       ├── _label: "Data"
│       └── ...
└── disk-2/
    └── (USB stick)
        └── partition-1/
            ├── _fs_type: fat32
            ├── _label: "USB DRIVE"
            └── ...
```

This virtual tree can be:
- **A real VFS mount point** (using `libfsdriver` or a FUSE-like mechanism) — user can `ls /foreign/disk-0/partition-1/home/`
- **A CLI-only construct** (the `gergios import` tool navigates it internally)

**Recommendation**: Start with CLI-only (no VFS integration needed), add VFS mount in Phase 2.

### 3.6 Import Mechanism

The actual file copy — simplest part:

```rust
// gergios-import/src/lib.rs

/// Copy a file or directory from a foreign FS into GergiOS local FS
pub fn import(
    foreign_fs: &dyn ForeignFilesystem,
    remote_path: &str,
    local_path: &Path,
    options: ImportOptions,
) -> Result<()> {
    // 1. Resolve remote path to FsId (walk directory tree)
    // 2. Stat the remote file
    // 3. If file: read in chunks, write to local_path
    // 4. If dir: mkdir local_path, recurse for each entry
    // 5. Preserve metadata: mtime, permissions (mode), uid/gid (if possible)
}

pub struct ImportOptions {
    pub preserve_permissions: bool,
    pub preserve_timestamps: bool,
    pub follow_symlinks: bool,
    pub overwrite: bool,
    pub recursive: bool,
}
```

**CLI interface** (`gergios import`):

```
# Interactive mode — scan and browse
gergios scan                         # List all disks and partitions
gergios ls-foreign                   # Browse foreign filesystems
gergios ls-foreign /disk-0/part-1/   # Browse specific partition

# Copy mode
gergios import /disk-0/part-1/home/user/docs /home/gergios/docs
gergios import --all                 # Import entire home directories from all found systems
gergios import --from ext4 --match "*.pdf" /home/gergios/pdf-imports/

# Mount mode (Phase 2)
gergios mount-foreign /disk-0/part-1 /mnt/linux  # Mount via VFS
gergios umount /mnt/linux
```

---

## 4. CLI Tool Design

### 4.1 `gergios` commands

```text
gergios scan                              # Discover all disks and partitions
gergios scan --verbose                    # Detailed partition info
gergios scan --json                       # Machine-readable output

gergios ls-foreign [path]                 # Browse foreign filesystems
gergios tree [path]                       # Tree view of foreign files

gergios import <source> <dest>            # Copy from foreign to local
gergios import --all [dest]               # Import all user data
gergios import --dry-run <source> <dest>  # Preview without copying
gergios import --progress <source> <dest> # Show progress bar

gergios detect-os [path]                  # Detect OS from partition (Linux/Windows/macOS)
gergios info <path>                       # Show FS metadata (label, UUID, size, free)
gergios mount-foreign <src> <dest>        # Mount via VFS (Phase 2)
```

### 4.2 Expected output examples

```
$ gergios scan
🔍 Scanning block devices...

/dev/c0d0 (ATA WDC WD1003FBYZ, 1.0 TiB)
├── 📐 GPT partition table
├── p1: EFI System (FAT32)       500 MiB [ESP]
├── p2: Linux filesystem (ext4)  256 GiB [Linux Root]
├── p3: Windows (NTFS)           512 GiB [Windows 11]
├── p4: Linux swap               16 GiB  [swap]
└── p5: Apple APFS               215 GiB [Macintosh HD]

/dev/c0d1 (ATA WDC WD40EFAX, 4.0 TiB)
├── 📐 GPT partition table
└── p1: Linux filesystem (ext4)  4.0 TiB [Media Server]

/dev/sda1 (SanDisk Ultra USB 3.0, 128 GiB)
├── 📐 MBR partition table
└── p1: FAT32                    128 GiB [USB DRIVE]

$ gergios ls-foreign /dev/c0d0/p2/
_runtime  _etc  _fs_type  _label
bin/      dev/  etc/      home/
lib/      mnt/  opt/      root/
sbin/     srv/  tmp/      usr/
var/      boot/

$ gergios import --dry-run /dev/c0d0/p2/home/alice/docs /home/gergios/imports/alice-docs
📋 Dry-run: would copy 47 files (128.4 MiB)
   ✓ /home/alice/docs/report.pdf (2.3 MiB)
   ✓ /home/alice/docs/photos/ (45 files, 124.1 MiB)
   ✓ /home/alice/docs/notes.txt (4.2 KiB)
   ...
```

---

## 5. Phases of Implementation

### Phase 1: Foundation — Partition Parser + CLI (6-8 weeks)

**Goal**: `gergios scan` works, `gergios ls-foreign` works for ext4 partitions.

#### Rust crates to create:

```
rust/
├── gergios-partition/          # MBR + GPT parser
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs              # Public API
│       ├── mbr.rs              # MBR parsing
│       ├── gpt.rs              # GPT parsing
│       └── probe.rs            # FS signature probing
│
├── gergios-fs-foreign/         # Unified foreign FS trait + registry
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs              # ForeignFilesystem trait
│       ├── registry.rs         # FS type ↔ driver mapping
│       └── types.rs            # DirEntry, FileStat, FsId
│
├── gergios-fs-ext4-adapter/    # ext4 → ForeignFilesystem adapter
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs              # Wraps ext4-core in ForeignFilesystem trait
│
└── gergios-import/             # CLI tool
    ├── Cargo.toml
    └── src/
        ├── main.rs             # CLI entry point
        ├── commands/
        │   ├── scan.rs         # gergios scan
        │   ├── ls.rs           # gergios ls-foreign
        │   ├── import.rs       # gergios import
        │   └── info.rs         # gergios info
        └── display.rs          # Rich terminal output (colors, tables)
```

**Deliverables**:
- [x] **0.1**: MBR parser — reads partition entries from LBA 0
- [x] **0.2**: GPT parser — reads GPT header + partition entry array
- [x] **0.3**: FS probe — signature detection for ext4, NTFS, FAT32, APFS
- [x] **0.4**: `gergios scan` — scans all `/dev/c0d*` devices, shows partition table
- [x] **0.5**: `ForeignFilesystem` trait + `ext4` adapter
- [x] **0.6**: Block I/O — raw disk open + partition-relative reader
- [x] **0.7**: `gergios ls-foreign` — browse ext4 partitions
- [x] **0.8**: `gergios import` — copy files from foreign ext4 to local FS

### Phase 2: NTFS + FAT32 Drivers (4-6 weeks)

**Goal**: Support for Windows and EFI System Partitions.

- [ ] **2.1**: NTFS adapter (`gergios-fs-ntfs`) wrapping `ntfs` crate
  - Read support: MFT, directories, files, attributes
  - ACL/permission mapping (NTFS → POSIX)
  - Filename encoding (UTF-16LE → UTF-8)
- [ ] **2.2**: FAT32 adapter (`gergios-fs-fatfs`) wrapping `fatfs` crate
  - Long filename support
  - 8.3 fallback
  - Cluster chain traversal
- [ ] **2.3**: `gergios scan` detects NTFS/FAT32 partitions
- [ ] **2.4**: `gergios ls-foreign` works for NTFS/FAT32
- [ ] **2.5**: `gergios import` works for NTFS/FAT32

### Phase 3: OS Detection + Smart Import (3-4 weeks)

**Goal**: GergiOS automatically identifies operating systems and knows where user data lives.

- [ ] **3.1**: OS detection heuristics:
  - **Linux**: `/etc/os-release`, `/etc/*release`, `/home/*`
  - **Windows**: `\Users\*`, `\Program Files`, registry files
  - **macOS**: `/Users/*`, `/Applications`, `.DS_Store` indicators
- [ ] **3.2**: Smart import presets:
  - `gergios import --from linux` — copies `/home/*`
  - `gergios import --from windows` — copies `\Users\*\Documents`, `\Users\*\Desktop`
  - `gergios import --from macos` — copies `/Users/*/Documents`
- [ ] **3.3**: Deduplication — skip files already imported (by name + size + mtime)
- [ ] **3.4**: Conflict resolution — rename vs overwrite vs skip
- [ ] **3.5**: Progress bar with ETA (using `indicatif` crate)

### Phase 4: VFS Integration (4-6 weeks)

**Goal**: Foreign filesystems appear as real mount points in the filesystem tree.

- [ ] **4.1**: Create a VFS proxy server that delegates to `ForeignFilesystem` trait
  - Implement `struct fsdriver` with read-only callbacks
  - Mount at `/foreign/` or user-specified path
- [ ] **4.2**: Automatic mount on boot (configurable in `/etc/system.conf`)
- [ ] **4.3**: Symlink farm — create `/foreign/linux/`, `/foreign/windows/` symlinks
- [ ] **4.4**: `mountpoint` support — allow `cd /foreign/disk-0/part-1/home`
- [ ] **4.5**: Integration with `gergios-gui` (file manager sees foreign FS too)

### Phase 5: APFS + Advanced FS (stretch — 4-8 weeks)

**Goal**: Support for macOS filesystems.

- [ ] **5.1**: APFS adapter (wrapping experimental Rust crates)
  - Container parsing (NXSB)
  - Volume management
  - Encryption detection (FileVault → prompt for password)
- [ ] **5.2**: HFS+ adapter (legacy macOS)
- [ ] **5.3**: exFAT adapter (large USB drives)
- [ ] **5.4**: Btrfs read support (experimental)
- [ ] **5.5**: ZFS read support (experimental)

---

## 6. Block I/O Architecture Details

### 6.1 Current MINIX Block Device Interface

MINIX provides these system calls through `libbdev`:

```c
// Open a block device
int bdev_open(const char *dev, int flags);
// flags: BDEV_R_BIT, BDEV_W_BIT, BDEV_RW_BITS

// Close
int bdev_close(int fd);

// Read/write at byte offset
ssize_t bdev_read(int fd, void *buf, size_t count, off_t offset);
ssize_t bdev_write(int fd, const void *buf, size_t count, off_t offset);

// I/O control
int bdev_ioctl(int fd, unsigned long request, void *data);
```

### 6.2 Rust FFI Wrapper

```rust
// rust/gergios-import/src/block_io.rs

use std::os::raw::{c_int, c_void};

extern "C" {
    fn bdev_open(dev: *const libc::c_char, flags: c_int) -> c_int;
    fn bdev_close(fd: c_int) -> c_int;
    fn bdev_read(fd: c_int, buf: *mut c_void, count: usize, offset: i64) -> isize;
}

pub struct BlockDevice {
    fd: c_int,
    sector_size: u64,
    total_sectors: u64,
}

impl BlockDevice {
    pub fn open(dev_path: &str, readonly: bool) -> Result<Self> {
        let flags = if readonly { BDEV_R_BIT } else { BDEV_R_BIT | BDEV_W_BIT };
        let dev_cstr = CString::new(dev_path)?;
        let fd = unsafe { bdev_open(dev_cstr.as_ptr(), flags) };
        if fd < 0 { return Err(Error::OpenFailed); }
        // Query sector size and total size via ioctl (DIOCGSECTORSIZE, DIOCGMEDIASIZE)
        Ok(Self { fd, sector_size: 512, total_sectors: 0 })
    }

    pub fn read_sectors(&self, lba: u64, buf: &mut [u8]) -> Result<()> {
        let offset = (lba * self.sector_size) as i64;
        let ret = unsafe {
            bdev_read(self.fd, buf.as_mut_ptr() as *mut c_void, buf.len(), offset)
        };
        if ret < 0 { Err(Error::ReadFailed) } else { Ok(()) }
    }

    pub fn read_exact(&self, offset: u64, buf: &mut [u8]) -> Result<()> {
        let ret = unsafe {
            bdev_read(self.fd, buf.as_mut_ptr() as *mut c_void, buf.len(), offset as i64)
        };
        if ret as usize != buf.len() { Err(Error::ReadFailed) } else { Ok(()) }
    }
}

impl Drop for BlockDevice {
    fn drop(&mut self) {
        unsafe { bdev_close(self.fd); }
    }
}
```

### 6.3 Device Enumeration

MINIX block devices follow the naming convention:

```
/dev/c0d0      — first controller, first disk (whole disk)
/dev/c0d0p0    — first partition on first disk
/dev/c0d0p1    — second partition
/dev/c0d1      — first controller, second disk
/dev/c1d0      — second controller, first disk
/dev/sda       — SCSI/SATA disk (alternative naming)
/dev/vnd0      — vnode disk (loopback)
```

**Enumeration strategy**: Iterate `/dev/c*d*` glob + `/dev/sd*` + check if device is a block device via `stat()`.

---

## 7. Dependencies (Rust Crates)

### Phase 1 (Foundation):
```toml
# gergios-partition
# No external deps — pure Rust MBR/GPT parsing from raw bytes

# gergios-fs-foreign
# No external deps — just traits and types

# gergios-fs-ext4-adapter
ext4-core = { path = "../ext4-core" }

# gergios-import
clap = { version = "4", features = ["derive"] }     # CLI argument parsing
indicatif = "0.17"                                    # Progress bars
colored = "2"                                         # Terminal colors
serde = { version = "1", features = ["derive"] }      # JSON output
serde_json = "1"
```

### Phase 2 (NTFS + FAT32):
```toml
ntfs = "0.5"              # Pure Rust NTFS parser
fatfs = "0.4"             # Pure Rust FAT filesystem
```

### Phase 5 (APFS):
```toml
apfs = "0.1"              # Experimental, likely fork from GitHub
```

---

## 8. Testing Strategy

### Unit Tests (host, no MINIX needed)

| Test | Location | What it tests |
|------|----------|---------------|
| MBR parsing | `gergios-partition/tests/mbr.rs` | Valid MBR, extended partitions, protective GPT MBR |
| GPT parsing | `gergios-partition/tests/gpt.rs` | Valid GPT, backup header, CRC validation, corrupted entries |
| FS probing | `gergios-partition/tests/probe.rs` | Signature detection for all supported FS types |
| Foreign FS trait | `gergios-fs-foreign/tests/` | Trait object dispatch, registry lookup |
| ext4 adapter | `gergios-fs-ext4-adapter/tests/` | Directory listing, file read, stat |
| CLI output | `gergios-import/tests/` | Command parsing, JSON output format |

### Integration Tests

- **With test disk images**: Create small ext4/NTFS/FAT32 disk images (using `dd` + `mkfs.*` or pre-created test fixtures)
- **Verify end-to-end**: `gergios scan` → detect partitions → `gergios ls-foreign` → `gergios import` → verify files are correct

### Test fixtures

```
tests/fixtures/
├── mbr-disk.img          # 100 MB MBR disk with single ext4 partition
├── gpt-disk.img          # 200 MB GPT disk with ext4 + FAT32 partitions
├── multi-disk-setup/     # Simulates multiple disks
│   ├── disk0.img         # ext4 only
│   └── disk1.img         # NTFS + FAT32
└── raw-partitions/
    ├── ext4-partition.bin    # Raw ext4 partition (no partition table)
    ├── fat32-partition.bin   # Raw FAT32 partition
    └── ntfs-partition.bin    # Raw NTFS partition
```

The fixtures can be generated with a build script:

```bash
# scripts/generate-test-fixtures.sh
dd if=/dev/zero of=tests/fixtures/mbr-disk.img bs=1M count=100
parted tests/fixtures/mbr-disk.img mklabel msdos
parted tests/fixtures/mbr-disk.img mkpart primary ext4 2048s 100%
# ... loop mount, mkfs.ext4, populate with test files
```

---

## 9. Risks and Mitigations

| Risk | Impact | Probability | Mitigation |
|------|--------|------------|------------|
| **Raw disk access crashes system** | 🔴 High | Low | Read-only access only; validate all bounds before reading |
| **NTFS read corruption** | 🟡 Medium | Low | Leverage battle-tested `ntfs` crate; extensive test fixtures |
| **APFS encryption (FileVault)** | 🟡 Medium | Medium | Detect encrypted state at probe time; prompt for password; skip if unavailable |
| **MINIX device enumeration incomplete** | 🟡 Medium | Low | Start with hardcoded `/dev/c0d0`; add enumeration with glob fallback |
| **No GPT support in old MBR tools** | 🟢 Low | Low | Rust parser doesn't depend on MINIX userspace tools |
| **Performance: large file import** | 🟢 Low | Medium | Streaming read/write with buffered I/O; progress reporting |
| **Cross-FS symlink breakage** | 🟢 Low | Medium | Copy symlinks as files or skip; document behavior |

---

## 10. Success Criteria

1. **Phase 1**:
   - `gergios scan` correctly parses MBR and GPT on real disk images
   - `gergios ls-foreign` lists ext4 partition contents
   - `gergios import` copies files from ext4 partition to local FS with correct contents
   - All operations are read-only (no writes to foreign partitions)

2. **Phase 2**:
   - `gergios ls-foreign` works on NTFS and FAT32 partitions
   - Files with long filenames and special characters import correctly
   - NTFS alternate data streams are detected (or skipped with warning)

3. **Phase 3**:
   - GergiOS correctly identifies Linux, Windows, and macOS installations
   - `gergios import --from linux` imports home directories without manual path specification

4. **Phase 4**:
   - Foreign filesystem appears as a real VFS mount point
   - Any POSIX tool (`ls`, `cat`, `cp`, `find`) can access foreign files via `/foreign/`

5. **Phase 5**:
   - APFS containers and volumes are detected
   - Encrypted volumes are detected with clear user prompt

---

## 11. Integration with Existing GergiOS Components

### ext4-core (already implemented)

The existing `ext4-core` crate serves as the model and reference for all future FS adapters. It provides:
- Extent tree traversal
- Directory + htree parsing
- Block addressing
- All the primitives needed by the foreign FS layer

A thin adapter (`gergios-fs-ext4-adapter`) wraps `ext4-core`'s functions into the `ForeignFilesystem` trait.

### GergiOS GUI (planning/11_gui_architecture.md)

In Phase 4+, the GUI file manager will show foreign FS partitions alongside local storage:
- Sidebar: "Foreign Systems" section
- Icons for Linux (penguin), Windows (window), macOS (apple)
- Drag-and-drop import from foreign FS to local storage
- Right-click → "Import to GergiOS..."

### GergiOS CLI (future)

The `gergios` CLI grows new verbs (`scan`, `ls-foreign`, `import`, `mount-foreign`, `umount`) alongside existing system management commands.

---

## 12. Directory Structure (Proposed)

```
rust/
├── gergios-partition/          # MBR + GPT parser
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── mbr.rs
│       ├── gpt.rs
│       └── probe.rs
│
├── gergios-fs-foreign/         # Unified ForeignFS trait
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── registry.rs
│       └── types.rs
│
├── gergios-fs-ext4-adapter/    # ext4 → ForeignFilesystem adapter
│   ├── Cargo.toml
│   └── src/lib.rs
│
├── gergios-fs-ntfs/            # NTFS → ForeignFilesystem adapter (Phase 2)
│   ├── Cargo.toml
│   └── src/lib.rs
│
├── gergios-fs-fatfs/           # FAT32 → ForeignFilesystem adapter (Phase 2)
│   ├── Cargo.toml
│   └── src/lib.rs
│
├── gergios-fs-apfs/            # APFS → ForeignFilesystem adapter (Phase 5)
│   ├── Cargo.toml
│   └── src/lib.rs
│
├── gergios-import/             # CLI tool
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── commands/
│       │   ├── scan.rs
│       │   ├── ls.rs
│       │   ├── import.rs
│       │   └── info.rs
│       ├── display.rs
│       └── block_io.rs
│
└── ext4-core/                  # Existing ext4 parser (dependency)
```

---

## 13. Related Documents

- `planning/19_ext4_driver_architecture.md` — ext4 driver implementation (read/write/journal)
- `planning/11_gui_architecture.md` — GUI file manager (future integration)
- `planning/01_microkernel_architecture.md` — MINIX architecture overview
- `minix/lib/libbdev/` — Block device library interface
- `minix/drivers/` — Block device drivers (ahci, at_wini, virtio_blk)
- `usr.sbin/vnconfig/vnconfig.c` — Vnode disk configurator (for loopback testing)
