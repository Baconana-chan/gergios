# Gergi TODO List

This document consolidates all TODO items across the Minix codebase, including existing tasks from various TODO files and new strategic directions.

## Table of Contents
- [Action Plan for 2026](#action-plan-for-2026)
- [Phase 1: Foundation](#phase-1-foundation)
- [Phase 2: Core Modernization](#phase-2-core-modernization)
- [Phase 3: Advanced Features](#phase-3-advanced-features)
- [Phase 4: Cleanup & Deprecation](#phase-4-cleanup--deprecation)
- [Detailed Component Analysis](#detailed-component-analysis)

---

## Action Plan for 2026

This document provides a concrete action plan for modernizing Minix in 2026, focusing on what TO DO rather than what NOT to do.

### Key Principles
- **Build modern alternatives first**, then deprecate legacy
- **Focus on x86_64 and ARM64**, deprecate i386
- **Adopt existing open-source solutions** instead of building from scratch
- **Incremental migration** with clear phases
- **Security and stability** as top priorities

---

## Phase 1: Foundation

### 1.1 Infrastructure Setup
- [x] Set up modern CI/CD pipeline (GitHub Actions or similar)
- [x] Create automated testing framework for existing codebase
- [x] Establish code coverage metrics and reporting
- [x] Set up static analysis tools (clang-tidy, cppcheck)
- [x] Create security scanning pipeline (Coverity, OSS-Fuzz)

### 1.2 Architecture Planning
- [x] Document current Minix microkernel architecture
- [x] Identify all legacy technology dependencies
- [x] Create migration roadmap for each obsolete component
- [x] Define target architecture support (x86_64, ARM64)
- [x] Plan deprecation timeline for i386 support

### 1.3 Rust Integration — ✅ **132+ утилит**
> Весь usr.bin/ портирован на Rust

**Июнь 2026**: Rust workspace содержит **132+ утилиты** (core POSIX, 6.4.3–6.4.6).

**Что сделано:**
- [x] Phase 1-5: 15 crates + 55 core POSIX утилит (basename, cat, chmod, cp, ls, mv, ...)
- [x] Phase 6.4.3: 11 утилит (colrm, join, jot, pr, rev, tabs, tsort, ul, unifdef, unvis, vis)
- [x] Phase 6.4.4: 14 утилит (cal, col, colcrt, column, csplit, fmt, hexdump, lam, ...)
- [x] Phase 6.4.5: 45 утилит (mcookie, mesg, mkfifo, mktemp, banner, calendar, lock, logger, ...)
- [x] Phase 6.4.6: 6 build-time tools (genassym, mkcsmapper, mkdep, mkesdb, mklocale, xinstall)
- [x] CI/CD + ASan/MSan/TSan + fuzzing + benchmarks + code coverage

**Осталось в pkgsrc:** rsync, m4, bzip2, unzip, tput, infocmp, indent, man, netstat

### 1.4 Filesystem Strategy ✅ **COMPLETED**
- [x] **VFS cleanup** — удалены lfs, chfs, v7fs, ufs core (T20 ✅)
- [x] **Оставлены:** ffs, mfs, ext2fs, cd9660, msdosfs, udf, puffs
- [x] ext4 выбран и полностью реализован (Rust core ~7,600 LOC + C bridge 29/35 callbacks + mkfs.ext4)
- [x] Архитектура интеграции спроектирована (см. `planning/19_ext4_driver_architecture.md`)
- [x] Rust ext4-core + C bridge + release scripts — всё завершено
- 🟡 MINIX toolchain — остаётся линковка ext4-core staticlib под MINIX (см. `planning/17_remaining_tasks.md §T21`)

### 1.5 Authentication Modernization
- [ ] Research Zero Trust authentication models
- [ ] Design authentication system for container-based environments
- [ ] Evaluate OAuth2/OpenID Connect integration
- [ ] Plan migration from Kerberos to modern auth
- [ ] Create authentication service prototype

---

## Phase 2: Core Modernization

### 2.1 Kernel SMP Improvements
- [x] **x86_64 SMP** — собран и слинкован (CONFIG_SMP=ON, clang.exe GNU driver, --target=x86_64-elf, 0 errors)
- [ ] Fix ARM SMP issues in arch/earm/arch_clock.c
- [ ] Implement CPU-local watchdog for all CPUs
- [ ] Add cacheline padding for CPU-local structures
- [ ] Fix timer synchronization across CPUs
- [ ] Test SMP functionality on multi-core ARM hardware
- [ ] **ARM64 SMP** — **задача на будущее** (после стабилизации UP ARM64 ядра):
      PSCI CPU_ON, GICv3 SGI, ACPI/DTB CPU discovery

### 2.2 Modern Filesystem Implementation ✅ **COMPLETED**
- [x] **ext4 Rust core** (~7,600 LOC) — superblock, extent tree, htree, jbd2 journal, metadata_csum, xattr, ACL, quota
- [x] **ext4 C bridge** — 29/35 fsdriver callbacks, CMake integration, MINIX toolchain готов
- [x] **Journaling** — jbd2 recovery, commit, checkpoint, CRC-32C
- [x] **Extents** — r/w/trunc/merge/split, flex_bg alloc
- [x] **mkfs.ext4** — Rust tool, 4KB blocks, extents, filetype, flex_bg
- [x] **Release scripts** — x86_hdimage, arm64_hdimage, arm_sdimage → ext4
- [x] **Default filesystem** — MFS_READONLY + ext4 as default в limine.conf/system.conf
- [x] Create ext4 test suite and validation (58 unit tests + 19 benchmarks + 20 blocktest Catch2)

### 2.3 USB Stack Modernization ✅ **COMPLETED** (Phase 7.3)
- [x] **Native xHCI driver** (Rust, `minix-xhci` crate) — PCI probe, hub support, MSC/HID/Bluetooth class drivers
- [x] **USB Mass Storage** (BOT protocol, SCSI READ10/WRITE10, DMA bounce buffer)
- [x] **USB HID** (keyboard, mouse, gamepad — chardev `/dev/kbd0`, `/dev/mouse0`, `/dev/gamepad0`)
- [x] **USB Hub** with Transaction Translator (TT) for USB 1.0/2.0
- [x] **USB Device Framework** — `UsbClassDriver` trait, class driver registration

### 2.4 Bluetooth Stack ✅ **COMPLETED** (Phase 7.8)
- [x] **HCI USB transport** — native Rust (bulk IN/OUT, cmd/event/ACL/SCO)
- [x] **HCI UART transport** — H4/H5 protocol, SLIP, CRC-16, OOB GPIO wake
- [x] **Intel firmware loading** — .sfi/.ddc from /lib/firmware/intel/
- [x] **Broadcom .hcd patchram loading** — HCD record parser
- [x] **Qualcomm (QCA) NVM/.bin loading** — EDL TLV protocol
- [x] **xHCI integration** — BT as class driver inside xHCI crate
- [x] **/dev/hci0 chardev** — BlueZ-compatible ioctls (HCIDEVUP/DOWN/GDEVINFO)
- [ ] **BlueZ userspace port** (Phase 8) — D-Bus, HCI socket, GLib

### 2.5 VFS Performance Improvements
- [ ] Replace O(n) loop in smap.c with hash table
- [ ] Implement modern async I/O patterns
- [ ] Add lock-free data structures where appropriate
- [ ] Optimize path resolution in path.c
- [ ] Benchmark VFS performance improvements

### 2.5 Rust Component Development ✅ **COMPLETED** (132+ утилит в usr.bin/)
- [x] **132+ utilities** — весь usr.bin/ портирован на Rust
- [x] **Buffer parsing** — audio-buf (14 tests), procfs-path (16 tests)
- [x] **Network parsers** — net-parse (TCP/UDP/DNS, 23 tests), packet-filter
- [x] **FFI bindings** — minix-rs (Message, syscall, 100+ call numbers)
- [x] **grep** — Quick Search + regex + gzip + mmap
- [x] **Drivers** — minix-ahci, minix-pci, e1000, virtio-net, virtio-blk

---

## Phase 3: Advanced Features

### 3.1 Package Manager Modernization
- [ ] Evaluate modern package managers (apk, dpkg, rpm)
- [ ] Choose package manager for Minix (recommend apk for embedded focus)
- [ ] Design package manager integration architecture
- [ ] Implement package manager integration
- [ ] Migrate existing packages to new system

### 3.2 Display and Graphics Infrastructure
- [ ] Enhance framebuffer driver support
- [ ] Add hardware acceleration (VESA, VBE)
- [ ] Design display server architecture (Wayland-based)
- [ ] Implement basic 2D graphics primitives
- [ ] Add font rendering engine

### 3.3 Input Device Support
- [ ] Implement keyboard driver with modern protocols
- [ ] Add mouse/touchpad driver support
- [ ] Create input device abstraction layer
- [ ] Implement hot-plug support for input devices
- [ ] Add multi-touch support

### 3.4 Window Management System
- [ ] Design window manager for microkernel architecture
- [ ] Implement window composition
- [ ] Add window decoration and theming
- [ ] Implement drag-and-drop functionality
- [ ] Add clipboard management

### 3.5 Advanced Rust Integration
- [ ] Implement Rust-based device drivers
- [ ] Create Rust-based service servers
- [ ] Develop memory-safe filesystem utilities
- [ ] Add Rust-based networking components
- [ ] Implement Rust security frameworks

---

## Phase 4: Cleanup & Deprecation

### 4.1 Legacy Code Removal
- [x] Deprecate i386 architecture support (All phases complete — i386 removed)
- [x] **Remove legacy filesystems** — LFS, CHFS, v7fs, UFS core удалены (T20 ✅)
- [ ] Remove DDEKit-based USB drivers
- [ ] Remove Kerberos authentication system
- [ ] Remove legacy package management tools

### 4.2 Documentation Updates
- [ ] Update all documentation to reflect modern architecture
- [ ] Create migration guides for users
- [ ] Document new APIs and interfaces
- [ ] Create developer onboarding guides
- [ ] Update system architecture documentation

### 4.3 Testing Framework — ✅ **Phase 9 COMPLETED**

Phase 9 (Testing Framework Migration) полностью выполнена — 6 sub-phases, ~20000 LOC, ~670 тестов:
- **9.1 CTest + FFI**: ~500 тестов (Rust unit + C FFI) ✅
- **9.2 QEMU Integration**: 4 boot/FS suites ✅
- **9.3 Property-Based**: 36 proptests (IPC, ext4, net-parse, BT SDP) ✅
- **9.4 C Migration (Catch2)**: 129 тестов (TCP, RAW, IPv6, BPF, blocktest, DS, RMIB, safecopy) ✅
- **9.5 C Coverage & Benchmarks**: gcov/lcov + lcov + Codecov integration ✅
- **9.6 CI/CD Hardening**: TSan, fuzz 600s, earm matrix, dashboard, regression detection ✅

### 4.4 GergiOS Rebranding — ✅ **Завершено**

- [x] `config.h`: OS_NAME → "GergiOS", OS_RELEASE → "1.0.0", OS_VERSION → "GergiOS 1.0.0 (MINIX 3.4.0)"
- [x] `kernel/main.c`: announce() → GergiOS 1.0 banner
- [x] `etc/boot.cfg.default`: "Start GergiOS" / "Start GergiOS (single user mode)"
- [x] `etc/motd`: "Welcome to GergiOS 1.0!" + gergios.dev
- [x] Shutdown messages: "GergiOS has halted", "GergiOS will now reset"
- [ ] User-facing man pages and documentation — осталось

### 4.5 Testing and Validation
- [x] **Security audit** — 12-section docs, integration tests (cap/MAC/W^X/audit) ✅
- [x] Performance benchmarking (C + Rust) — hyperfine, 20+ variants, regression detection in CI
- [ ] Stress testing of new filesystem and USB stack (ждёт MINIX toolchain + QEMU)
- [x] Validation of Rust integration safety guarantees — ASan/MSan/TSan в CI
- [ ] End-to-end testing of GUI components (ждёт GUI)

---

## Detailed Component Analysis

### Components to Build (Modern Alternatives)

#### 1. Filesystem: ext4 Driver
**Why**: ext4 is the standard Linux filesystem, mature, well-documented, and widely supported
**Approach**: Port ext4 driver from Linux, adapt to Minix VFS layer
**Priority**: High (Q2 2026)
**Effort**: 3-4 months
**Dependencies**: VFS layer improvements

#### 2. USB Stack: Linux USB Stack Port
**Why**: Linux USB stack is mature, supports wide range of hardware, actively maintained
**Approach**: Port Linux USB core and host controller drivers to Minix
**Priority**: High (Q2 2026)
**Effort**: 4-5 months
**Dependencies**: Driver framework design

#### 3. Authentication: Zero Trust System
**Why**: Modern security model, suitable for container-based environments
**Approach**: Implement OAuth2/OpenID Connect service, integrate with system services
**Priority**: High (Q1 2026)
**Effort**: 2-3 months
**Dependencies**: None

#### 4. Package Manager: apk Integration
**Why**: Alpine's apk is lightweight, suitable for embedded systems, well-maintained
**Approach**: Integrate apk package manager, create Minix package repository
**Priority**: Medium (Q3 2026)
**Effort**: 2-3 months
**Dependencies**: None

#### 5. Display: Wayland Compositor
**Why**: Modern display server protocol, secure, compositing-friendly
**Approach**: Implement Wayland compositor for Minix, integrate with framebuffer
**Priority**: Medium (Q3 2026)
**Effort**: 4-6 months
**Dependencies**: Graphics infrastructure

#### 6. Rust Integration: Incremental Migration
**Why**: Memory safety, modern tooling, future-proof codebase
**Approach**: Start with userland utilities, move to drivers, then kernel components
**Priority**: High (Q1-Q4 2026)
**Effort**: Ongoing throughout 2026
**Dependencies**: Build system integration

### Components to Deprecate

#### 1. i386 Architecture Support
**Timeline**: Begin deprecation Q2 2026 (Phase 2), complete removal 2027
**Reason**: Legacy architecture, modern systems use x86_64 and ARM64
**Migration Path**: Focus on x86_64 and ARM64 support
**Phase 1 Status**: ✅ Complete (announcement, FAQ, troubleshooting guide, codebase audit, support channels)
**Phase 2 Status**: ✅ Complete (x86_64 default target, CI/CD prioritized, deprecated warnings, feature restrictions)
**Phase 3 Status**: ✅ Complete (i386 requires MKI386=ON, removed from CI/CD, community-only support, archive documented)

#### 2. DDEKit USB Framework
**Timeline**: Replace Q2 2026, remove Q4 2026
**Reason**: Unmaintained, API issues, limited hardware support
**Migration Path**: Port Linux USB stack

#### 3. Legacy Filesystems (LFS, UFS2, ext2, CHFS)
**Timeline**: Replace Q2-Q3 2026, remove Q4 2026
**Reason**: Obsolete, limited features, poor performance on modern hardware
**Migration Path**: Implement ext4 support

#### 4. Kerberos Authentication
**Timeline**: Replace Q1 2026, remove Q3 2026
**Reason**: Dying technology, unsuitable for modern container environments
**Migration Path**: Implement Zero Trust authentication

#### 5. Legacy Package Management (pkg_*, sysinst)
**Timeline**: Replace Q3 2026, remove Q4 2026
**Reason**: Custom system, limited features, not standard
**Migration Path**: Integrate apk package manager

---

## Success Metrics

### Phase 1
- CI/CD pipeline operational
- 80% code coverage on core components
- Rust toolchain integrated
- Migration roadmap complete

### Phase 2
- ARM SMP fully functional
- ext4 driver basic operations working
- USB core driver operational
- VFS performance improved by 30%
- First Rust component in production

### Phase 3
- Package manager integrated
- Basic GUI functional
- Input devices working
- Window manager operational
- 5+ Rust components in production

### Phase 4
- All legacy code removed
- Documentation complete
- Security audit passed
- Performance benchmarks met
- System fully modernized

---

## Notes

- This TODO focuses on concrete actions with clear deliverables
- Each phase builds on the previous one
- Regular reviews and adjustments recommended
- Community feedback should be incorporated
- Focus on incremental, testable changes
