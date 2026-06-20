# Migration Roadmap for Obsolete Components

## Overview

This document provides detailed migration roadmaps for each obsolete component identified in the legacy dependencies analysis.

## Component Migration Roadmaps

### 1. Build System Migration (BSD Make → CMake)

#### Current State
- BSD Make (bmake) with recursive makefiles
- Limited parallelization
- Complex dependency tracking

#### Target State
- CMake build system
- Ninja backend for fast builds
- Better dependency tracking
- Modern IDE integration

#### Migration Steps

**Phase 1: Preparation**
- [x] Evaluate CMake structure for Minix
- [x] Create CMake prototype for kernel
- [x] Set up CMake testing infrastructure
- [x] Document CMake best practices

**Status**: COMPLETED

**Implementation Summary**:
The Phase 1 preparation is complete. The following artifacts were created:

1. **`cmake/EVALUATION.md`** — Full analysis of current build system, target CMake structure, migration strategy, and risk assessment. Compares BSD Make vs CMake features.

2. **Root `CMakeLists.txt`** — Project definition, architecture detection (i386/earm), MINIX option handling, global flags, subdirectory includes.

3. **`cmake/toolchain-minix.cmake`** — Toolchain file for native and cross-compilation. Handles MINIX DESTDIR as sysroot, tool prefix, static linking, ARM unaligned access flags.

4. **`cmake/arch/i386.cmake` + `cmake/arch/earm.cmake`** — Architecture definitions (CPU model, GNU arch, platform triple, compile flags, link flags).

5. **`cmake/options.cmake`** — MINIX MK*/USE* option handling. Maps all major options from bsd.own.mk with proper dependent options.

6. **`cmake/macros.cmake`** — Reusable macros: `add_minix_executable()`, `add_minix_library()`, `add_minix_service()`, `add_unpaged_objects()`, `generate_kernel_offsets()`. Replicates bsd.prog.mk + minix.service.mk patterns.

7. **`minix/kernel/CMakeLists.txt`** — Full kernel build prototype with:
   - Core sources + architecture-specific sources (i386 and earm)
   - System call implementations with i386-conditional do_devio.c
   - Unpaged object handling via OBJCOPY symbol prefixing
   - Offset header generation (procoffsets.h, extracted-errno.h, etc.)
   - Optional CONFIG_SMP, USE_WATCHDOG, USE_ACPI, USE_APIC, USE_DEBUGREG
   - Minix library unpaged objects from libc, libsys, libminc
   - Bitcode/LTO support (disabled by default)

8. **`crypto/external/gpl2/wolfssl/CMakeLists.txt`** — wolfSSL build prototype with core/openssl-compat sources, MINIX-specific compile definitions.

9. **`tests/CMakeLists.txt`** — CTest configuration with kernel and wolfSSL smoke tests, ATF integration placeholders.

10. **`docs/cmake-migration-guide.md`** — Comprehensive migration guide with BSD Make → CMake mapping table, step-by-step component migration, best practices, common pitfalls, and testing instructions.

**Phase 2: Core Migration**
- [x] Migrate kernel build to CMake (Phase 1 prototype, refined)
- [x] Migrate servers build to CMake
- [x] Migrate drivers build to CMake
- [x] Migrate libraries build to CMake

**Status**: COMPLETED

**Implementation Summary**:
Phase 2 migrated the core MINIX components — servers, drivers, and libraries — to CMake:

**Servers (11 CMakeLists.txt files)**
- Top-level `minix/servers/CMakeLists.txt` with MKIMAGEONLY-conditional subdirectories
- All servers use `add_minix_service()` macro with proper linking (-lsys, -ltimers, -lexec, etc.)
- PM server: per-file include paths matching CPPFLAGS.*.c+= -I patterns
- VFS server: conditional gcov.c for coverage, -Wall -Wextra warnings
- VM server: arch-specific linker script (earm/vm.lds), PAE support, MAGIC flags
- IS server: APIC conditional compile, multi-tree include paths for dmp_*.c
- RS server: PCI support flag, exec library
- DS server: regcomp/regfree workaround for weak symbols
- IPC server: config file install to /etc/system.conf.d
- devman server: vtreefs + fsdriver + sys linkage
- sched, input, mib servers: simple add_minix_service() calls

**Drivers (10 CMakeLists.txt files)**
- Top-level `minix/drivers/CMakeLists.txt` with MKIMAGEONLY + .WAIT storage ordering
- Architecture-conditional build: i386-only PCI, at_wini, floppy, audio, net drivers
- Architecture-conditional build: earm-only mmc, lan8710a, i2c drivers
- PCI driver: multi-tree includes (sys/dev/pci, sys/dev), _PCI_SERVER define
- TTY driver: architecture-specific sources, keymaps install
- Storage: at_wini with blockdriver + sys + timers, memory driver (last)
- Audio/Net/HID: architecture-conditional subdirectory trees

**Libraries (15+ CMakeLists.txt files)**
- Top-level `minix/lib/CMakeLists.txt` with architecture-conditional + .WAIT ordering
- **libsys** (full migration, 80+ sources + arch-specific + PCI + coverage)
  - i386 arch sources: 17 files (ser_putc, sys_in/out, vbox, etc.)
  - earm arch sources: 3 files (frclock_util, spin, tsc_util)
  - Conditional PCI: 18 sources (pci_attr_*, pci_find_*, etc.)
  - Conditional coverage: gcov.c, sef_gcov.c, llvm_gcov.c
- **libminc** (full migration, 60+ sources from 8 different source trees)
  - Own sources: atoi, fputs, _snprintf, strtol
  - Imports from libsa (errno, printf, strerror, subr_prf)
  - Imports from libsys (kputc, sys_diagctl)
  - Imports from common/lib/libc (bswap64, rb, sha2, string funcs, stdlib)
  - Imports from compiler-rt (15 division/mul routines)
  - Imports from libc (gen, stdlib, string, regex, time, misc)
  - Architecture-specific syscall wrappers (.S files)
  - Generated errlist.c from errno.h via awk
  - Per-file compile definitions for _STANDALONE, _LIBC, _LIBSYS, putchar=kputc
- **libtimers**, **libbdev**, **libmthread**: full migration (simple)
- **libexec, libblockdriver, libchardriver, libfsdriver, libvtreefs, libddekit, liblwip**: structural placeholders with includes and install paths

**Root CMakeLists.txt** updated — servers, drivers, lib subdirectories are now active.

**Total**: 36+ new CMakeLists.txt files across servers, drivers, and libraries.

**Phase 3: Userland Migration**
- [x] Migrate userland tools to CMake
- [x] Migrate tests to CMake
- [x] Update CI/CD pipelines
- [x] Update documentation

**Status**: COMPLETED

**Implementation Summary**:
Phase 3 migrated userland tools, filesystem servers, network services, tests, and CI/CD:

**Commands (60+ commands in 1 top-level CMakeLists.txt)**
- Top-level `minix/commands/CMakeLists.txt` with helper macros for 60+ commands
- All commands get default -lasyn -lterminfo (from Makefile.inc) and install to /usr/bin
- Simple commands: `_add_simple_command(name)` — one .c file, auto man page install
- Complex commands: `_add_command()` with explicit sources, libs, BINDIR
- Special cases: fetch with wolfSSL (conditional MKCRYPTO), mount in /bin
- Script-only commands: setup, lspci, MAKEDEV (installed as scripts)
- update_asr conditionally built with USE_ASR
- Test data files: ministat chameleon/iguana installed to /usr/share/ministat

**Filesystem Servers (4 CMakeLists.txt files)**
- Top-level `minix/fs/CMakeLists.txt` with MKIMAGEONLY gates
- MFS, PFS: always built
- ext2, isofs, procfs, ptyfs: conditional (MKIMAGEONLY)
- hgfs, vbfs: i386-only conditional
- All filesystem servers use add_minix_service() with proper -lminixfs -lfsdriver -lbdev -lsys linkage

**Network Services (3 CMakeLists.txt files)**
- lwIP TCP/IP service: full source list, INET6 conditional, lwip.conf install
- UDS (UNIX Domain Sockets): full sources, uds.conf + unix.8 install
- Top-level `minix/net/CMakeLists.txt` with MKIMAGEONLY gate

**User Binaries (1 CMakeLists.txt file)**
- grep: 6 sources, -lz, symbolic links for egrep/fgrep
- diff, mined, ministat (-lm), mtop (-lcurses -lterminfo), toproto, trace
- eepromread: earm-only conditional

**System Binaries (1 CMakeLists.txt file)**
- btrace, diskctl, fbdctl, mkproto: add_minix_executable with BINDIR /usr/sbin + man pages
- mkfs.mfs: subdirectory with v3 sub-build

**Tests (3 CMakeLists.txt files)**
- Top-level `minix/tests/CMakeLists.txt` with MKATF gate
- blocktest, ddekit, ds, fbdtest, rmibtest, safecopy: stub structures
- CTest integration: ATF/KYUA test discovery via add_test()
- Kernel test from Phase 1: compilation + size check
- wolfSSL test from Phase 1: compilation + header accessibility

**CI/CD**
- `cmake/ci-config.cmake`: CI pipeline documentation with GitHub Actions reference
- Build matrix: i386 + earm × Debug + Release + MinSizeRel × clang + gcc
- Standard stages: Configure → Build → Test → Package
- Custom validation targets: ci-validate-config, ci-build-all, ci-smoke-test
- Full GitHub Actions YAML workflow reference included

**Root CMakeLists.txt** updated: added fs, net, commands, usr.bin, usr.sbin, tests subdirectories. Removed duplicate minix/lib entry. Fixed CMAKE_MODULE_PATH for flat arch files.

**Total Phase 3**: 10+ new CMakeLists.txt files covering all userland, filesystems, network, tests, and CI/CD config.

**Phase 4: Cleanup**
- [x] Add DEPRECATED headers to key BSD Makefiles
- [x] Create releasetools/cmake-build.sh — CMake build wrapper script
- [x] Create CMakePresets.json — presets for common configurations
- [x] Create docs/dual-build-guide.md — transition guide
- [x] Fix cmake-build.sh (check_prereqs call, shift for args)
- [x] Fix CMakePresets.json version 6 → 3 (CMake 3.21+ compat)
- [x] Fix build.sh — add deprecation notice header
- [x] Fix docs/dual-build-guide.md — detailed milestone table

**Status**: COMPLETED

**Implementation Summary**:
Phase 4 established the dual-build transition infrastructure:

**Deprecation Notices**:
- `build.sh` (legacy entry point) — full deprecation header with CMake migration instructions
- `minix/Makefile` — deprecation notice pointing to CMake build
- `minix/{kernel,servers,drivers,lib}/Makefile` — brief deprecation notices
- BSD Makefiles preserved for backward compatibility

**CMake Build Infrastructure**:
- `releasetools/cmake-build.sh` — wrapper script with configure/build/test/clean/list commands
  - Auto-detects Ninja vs Make backend
  - Supports cross-compilation via arch parameter
  - Color-coded output, error handling
- `CMakePresets.json` (version 3) — 5 configure presets (default, i386-release, earm-debug, i386-minimal, i386-coverage)
  - Compatible with CMake 3.21+
  - GitHub: `cmake --preset default && cmake --build --preset default && ctest --preset default`
- `cmake_minimum_required` bumped to 3.21

**Documentation**:
- `docs/dual-build-guide.md` — when to use each system, file organization, migration timeline with specific milestones

**Original Makefiles**:
- NOT deleted — preserved for backward compatibility during transition
- All marked DEPRECATED with pointers to CMake equivalents

#### Dependencies
- Phases 1-3 (CMake build system must exist)

#### Risks
- Parallel build systems increase maintenance burden
- Developers may accidentally use wrong build system
- CMake prototype not yet at production parity for release images


---

### 2. Architecture Migration (i386 → x86_64 + ARM64)

#### Current State
- Primary: **x86_64** (64-bit) — sole x86 architecture
- ✅ i386: **Phase 4 — Complete Removal** (code removed from main branch, preserved in git tag `archive/i386-last`)
- Experimental ARM (earm) support
- No ARM64 support

#### Target State
- Primary: x86_64 and ARM64
- Deprecated: i386 ✅ (fully removed)
- Full 64-bit support

#### Migration Steps

**Phase 1: x86_64 Foundation**
- [x] Audit i386-specific code
- [x] Identify 32-bit assumptions
- [x] Create x86_64 architecture directory
- [x] Implement x86_64 boot process
- [x] Port kernel to x86_64 (phases 1–4: build, boot, memory, syscalls/signals)
- [x] Port servers to x86_64
- [x] Port drivers to x86_64

**Status**: COMPLETED ✅

**Implementation Summary**:
x86_64 migration completed across 6 phases:
1. **Build Infrastructure** — cross-toolchain (gcc 14.2.0, binutils 2.44), Makefile.inc build rules, addr2line support
2. **Kernel Bootstrap** — multiboot entry, long mode switch (GDT/IDT/paging), printf, exception handlers, APIC timers, segment register context switch
3. **Memory Management** — 4-level page tables, PAE/PSE, pmap for x86_64, VM inherit, kernel/user split
4. **System Calls + Signals** — sigcontext (16 GP + iretq), mcontext_t (26 gregs + FXSAVE), stackframe, ucontext, sigreturn
5. **Libraries + Toolchain** — libc (cpuid, IPC via SYSCALL, RDTSC, ucontext), libsys (ser_putc, I/O ports), libminc (setjmp/longjmp)
6. **Driver Adaptation** — PCI (0xCF8/0xCFC), UART/RS232, CMOS/RTC, PIC, archtypes (16-byte IDT, u64_t desctableptr)

**Phase 2: ARM64 Foundation**
- [ ] Audit ARM-specific code
- [x] Create ARM64 architecture directory ✅ (planning/08 Phase 1)
- [x] CMake build infrastructure for aarch64 ✅
- [x] ARM64 kernel source files (Phase 2: head.S, mpx.S, klib.S, exception.c, memory.c, protect.c, pg_utils.c, arch_system.c, arch_do_vmctl.c, arch_timer.c, arch_reset.c, hw_intr.c) ✅
- [x] FDT Device Tree parser (fdt.h/fdt.c) — валидация DTB, /memory, /cpus, /chosen, stdout-path UART lookup с alias resolution ✅
- [x] Limine AAC64 request structures (limine.h/limine.c) — .limine_requests, pre_init entry, self-contained PL011 ✅
- [ ] Implement ARM64 boot process (нужен sysroot — planning/17 T1–T2)
- [ ] Port kernel to ARM64 (в процессе, см. planning/08)
- [ ] Port servers to ARM64
- [ ] Port drivers to ARM64

**Подробнее**: `planning/08_arm64_migration_plan.md`, `planning/17_remaining_tasks.md` §T1–T11

**Phase 3: Testing and Validation**
- [ ] Set up x86_64 test infrastructure (QEMU, real hardware)
- [ ] Set up ARM64 test infrastructure
- [ ] Comprehensive testing on both architectures
- [ ] Performance benchmarking
- [ ] Security validation

**Phase 4: i386 Deprecation — COMPLETED ✅**
- [x] Phase 1: Announcement and Preparation (Q2 2026)
- [x] Phase 2: Soft Deprecation (Q2 2026) — x86_64 default, deprecation warnings, CI/CD priority shift
- [x] Phase 3: Hard Deprecation (Q2 2026) — `-DMKI386=ON` required, community-only support
- [x] Phase 4: Complete Removal (Q2 2026) — arch code deleted, build system cleaned, `__i386__` ifdefs cleaned
- [x] Documentation: announcement, FAQ, troubleshooting, codebase audit, support channels, hard deprecation notice, archive guide
- [x] Git tag: `archive/i386-last` preserves legacy code

For full details, see:
- `planning/05_i386_deprecation_timeline.md` — full timeline with checkboxes
- `docs/i386-deprecation-announcement.md` — announcement and status
- `docs/archive/` — archived i386 documentation

**Phase 5: i386 Removal — 🟡 Deferred (x86_64 cleanup → planning/17)**

**Что сделано**:
- [x] i386-only директории удалены (`sys/arch/i386/`, i386-драйверы, `cmake/arch_i386.cmake`)
- [x] Build system очищен (CMakeLists.txt, BSD Makefiles)
- [x] `__i386__` ifdefs очищены (ядро, серверы, библиотеки)
- [x] i386-only тесты удалены

**Что осталось (перенесено в `planning/17_remaining_tasks.md` §T3–T7)**:
- Оставшиеся shared arch/i386 директории (`minix/kernel/arch/i386/`, `minix/lib/libsys/arch/i386/`, `minix/include/arch/i386/`, `minix/servers/vm/arch/i386/`) содержат код, общий с x86_64 через `__x86_64__` ifdefs. Эти директории были восстановлены из git tag `archive/i386-last`. Создание чистых `arch/x86_64/` директорий — отдельная задача очистки.

#### Dependencies
- Build system migration (completed ✅)
- Rust integration (can be done in parallel)
- C language modernization (C17 for C code)

#### Risks
- Complex architecture-specific code
- x86_64 kernel still depends on code in `arch/i386/` — needs proper separation
- Need access to ARM64 hardware for testing
- Potential performance regressions


---

### 3. C Language Modernization (C89 → C17 + Rust)

#### Current State
- C89/C90 standard throughout
- No modern C features
- Manual memory management

#### Target State
- C17 for existing C code
- Rust for new components
- Gradual migration to Rust

#### Migration Steps

**Phase 1: Foundation** ✅
- [x] Enable C17 support in compiler (`-std=gnu17`)
- [x] Update coding standards for C17 + Rust
- [x] Set up Rust toolchain (rustc, cargo, cross-compilation)
- [x] Create Rust-C FFI interface standards
- [x] Build system integration for Rust (CMake add_rust_utility/add_rust_test)
- [x] Add `__STDC_VERSION__` / feature-test-macro strategy to config.h

**Status**: ✅ COMPLETED (see `planning/09_c_language_modernization.md` §Phase 1)

**Phase 2: C17 Migration — Incremental per Component** ✅
- [x] Audit code for C89/C99 assumptions (K&R style, implicit int, `register` keyword)
- [x] Enable C17 features incrementally per subsystem:
      • Kernel core — designated initializers, `_Alignas`, `_Static_assert`
      • Servers (PM, VFS, VM) — `_Noreturn`, `_Generic`, compound literals
      • Drivers — `__func__`, `inline`, anonymous structures
- [x] Update `share/mk/bsd.sys.mk`: `-std=gnu99` → `-std=gnu17` (or drop for default)
- [x] Update `cmake/`: set `CMAKE_C_STANDARD 17` with `CMAKE_C_EXTENSIONS ON`
- [x] Resolve C17 `inline` semantics differences (C99→C17 changed linkage rules)
- [x] Phase out `register` keyword (C17 deprecated, C23 removes)
- [x] Verify no breakage: compile entire tree with `-std=gnu17 -Werror`

**Status**: ✅ COMPLETED (see `planning/09_c_language_modernization.md` §Phase 2a-2e)
- ~200 `register` keywords removed across ~60 files
- 31 `__dead` → `_Noreturn` across 22 files
- `_Generic` MMIO macros for sound drivers (als4000, cmi8738)
- `_Static_assert` for kernel struct invariants

**Phase 3: Rust Integration** ✅
- [x] Create prototype Rust component (basename, dirname, echo, true, false, yes, sleep, seq)
- [x] Implement Rust-C FFI layer (syscall wrappers via `extern "C"`)
- [x] Migrate string handling utilities to Rust
- [x] Migrate parsing components (grep: Quick Search + regex) to Rust
- [x] Create Rust test infrastructure (cargo test, CI integration, CTest)
- [x] Set up Rust cross-compilation for x86_64 and earm

**Status**: ✅ COMPLETED (see `planning/09_c_language_modernization.md` §Phase 3)
- 10 utilities ported to Rust (basename, dirname, echo, true, false, yes, sleep, seq, grep, minix-rs)
- grep: full POSIX implementation with Quick Search + regex + gzip + mmap
- minix-rs: FFI bindings crate (Message struct, syscall, endpoint constants, 100+ call numbers)
- Build integration: CMake add_rust_utility() + add_rust_test() + BSD Make

**Phase 4: Critical Memory-Safe Components** ✅
- [x] Migrate buffer/network parsers to Rust
- [x] Create audio-buf: ring buffer management crate
- [x] Create procfs-path: PID/path parsing crate
- [x] Implement memory-safe IPC message handling in Rust (minix-rs validation layer)
- [x] Create net-parse: TCP/UDP/DNS protocol parsers in Rust
- [x] Add fuzz testing for FFI boundaries (cargo-fuzz, 6 targets)

**Status**: ✅ COMPLETED (see `planning/09_c_language_modernization.md` §Phase 4)
- 4 new crates: audio-buf (14 tests), procfs-path (16 tests), net-parse (23 tests), fuzz (6 targets)
- All no_std, zero unsafe code (except Mmap::map)
- minix-rs extended with IPC validation (is_pm_call, is_vfs_call, check_offset, etc.)

**Phase 5: Advanced Components (kernel-adjacent)** ✅
- [x] Create safe MMIO and port I/O wrappers (minix-driver)
- [x] Evaluate PM/VFS for Rust rewrite (documented in planning/13 — NOT recommended)
- [x] Create GlobalAlloc → C malloc/free bridge (minix-alloc)
- [x] Assess ASan/MSan/TSan infrastructure (exists in LLVM tree, deferred to Phase 6)

**Status**: ✅ COMPLETED (see `planning/09_c_language_modernization.md` §Phase 5 + `planning/13_pm_vfs_rust_evaluation.md`)
- minix-driver: VolatileCell, MmioRegion (bounds-checked), port I/O FFI
- minix-alloc: GlobalAlloc via malloc/free FFI, no_std
- PM/VFS: Full rewrite impractical — incremental Rust helpers recommended
- ASan/MSan/TSan: Infrastructure exists, integration deferred to Phase 6

**Phase 6: CI/CD & Sanitizer Integration** ✅
- [x] Set up QEMU-based test runner for MINIX (`scripts/run_qemu_test.sh`)
- [x] Integrate ASan/MSan/TSan into CMake (`get_rust_sanitizer_flags()`, `RUST_SANITIZE_*` options)
- [x] Create cargo-fuzz CI job for Rust-C FFI boundaries (3 fuzz targets, 5-10 min each)
- [x] Add performance benchmarking (`scripts/run_benchmarks.sh`, hyperfine, 20+ variants)
- [x] Add code coverage (`cargo llvm-cov` → Codecov, CI `rust-coverage` job)
- [x] Full CI pipeline: 8 jobs (rust-build, rust-sanitizers, rust-fuzz, rust-coverage,
      rust-benchmarks, build (legacy), qemu-test, static-analysis, security-scan)

**Documents**: `planning/14_phase6_cicd_sanitizers.md`

**Phase 7: Future Directions** 🔮 (отложено — GUI, Lua, rebranding не являются
критическими на данном этапе)
- [ ] Incremental PM helpers (signal mask, PID allocator)
- [ ] Incremental VFS helpers (path validation, permission checks)
- [ ] GUI infrastructure (framebuffer → Wayland — see planning/11)
- [ ] GergiOS rebranding (see planning/10 §5)
- [ ] Lua scripting integration (games, config, GUI)

#### Key Technical Details

**Why C17 over C11?**
- C17 (N2176) is a bug-fix release of C11 — no new language features, but clarifies
  undefined behavior, improves `_Atomic` semantics, and deprecates ancient features
- Compiler support: GCC 8+, Clang 7+ (both widely available in 2026)
- C17 is the last ISO C standard before C23; migrating to C17 now avoids two jumps

**C99 → C17 migration surface (actual code changes needed):**
- `register` keyword: remove throughout (deprecated in C17, removed in C23)
- `__STDC_VERSION__` strategy: add `#if __STDC_VERSION__ >= 201710L` for C17 guards
- `inline` semantics: C99 `extern inline` vs C17 `static inline` — audit header functions
- `_Noreturn` (C11): replace `__dead`/`__attribute__((noreturn))` where appropriate
- `_Alignas`/`_Alignof` (C11): replace compiler-`__attribute__((aligned))`
- `_Static_assert` (C11): replace runtime assert.h compile-time checks
- `__func__` (C99): standardize on `__func__` instead of custom `__FUNCTION__`
- `_Generic` (C11): type-generic macros instead of `#define` overloading

#### Success Metrics
- **C17**: Full tree compiles with `-std=gnu17 -Werror` without warnings
- **Rust**: 5+ Rust components in production (userland utilities)
- **Memory safety**: 50% reduction in buffer-overflow CVEs in migrated components
- **Coverage**: Full tree + Rust components in CI with ASan

#### Dependencies
- Build system migration (for Rust integration, C17 standard setting)
- Architecture migration (for testing on x86_64 + ARM64)

#### Risks
- Learning curve for Rust
- FFI complexity (panic safety, ownership across language boundary)
- C17 `inline` semantics changes may cause subtle linker errors
- Performance concerns for Rust in kernel-adjacent code

#### Dependencies
- Build system migration (for Rust integration)
- Architecture migration (for testing)

#### Risks
- Learning curve for Rust
- FFI complexity
- Performance concerns
- Developer resistance


---

### 4. Filesystem Migration (Minix FS → ext4)

#### Current State
- Minix filesystem (v1, v2, v3)
- Limited ext2 support
- No modern filesystem features

#### Target State
- ext4 as primary filesystem
- Minix FS as read-only legacy support
- FUSE for additional filesystems

#### Migration Steps

**Phase 1: Research and Design**
- [ ] Research ext4 implementation
- [ ] Design ext4 integration architecture
- [ ] Evaluate existing ext4 drivers
- [ ] Plan migration strategy

**Phase 2: ext4 Driver Development**
- [ ] Implement ext4 driver
- [ ] Implement ext4 server
- [ ] Add ext4 to VFS
- [ ] Implement ext4-specific features

**Phase 3: Testing and Validation**
- [ ] Test ext4 driver
- [ ] Performance testing
- [ ] Compatibility testing
- [ ] Migration tools testing

**Phase 4: Migration**
- [ ] Create migration tools
- [ ] Update installation process
- [ ] Update documentation
- [ ] Provide migration guide

**Phase 5: Legacy Support**
- [ ] Keep Minix FS as read-only
- [ ] Add FUSE support
- [ ] Deprecate Minix FS write support
- [ ] Update default filesystem

#### Dependencies
- Architecture migration (for testing)
- Driver model modernization

#### Risks
- Complex filesystem implementation
- Data loss during migration
- Performance issues
- Compatibility problems


---

### 5. Driver Model Modernization

#### Current State
- Legacy driver interfaces
- Monolithic driver structure
- Poor hot-plug support

#### Target State
- Modern driver framework
- Modular driver structure
- Hot-plug support
- Linux driver compatibility layer

#### Migration Steps

**Phase 1: Design**
- [ ] Design modern driver framework
- [ ] Define driver interfaces
- [ ] Plan hot-plug support
- [ ] Evaluate Linux driver compatibility

**Phase 2: Framework Implementation**
- [ ] Implement driver framework
- [ ] Implement driver registry
- [ ] Implement hot-plub support
- [ ] Create driver templates

**Phase 3: Driver Migration**
- [ ] Migrate block drivers
- [ ] Migrate character drivers
- [ ] Migrate network drivers
- [ ] Migrate other drivers

**Phase 4: Linux Compatibility**
- [ ] Implement Linux driver compatibility layer
- [ ] Test Linux drivers
- [ ] Document compatibility
- [ ] Create driver porting guide

**Phase 5: Testing**
- [ ] Comprehensive driver testing
- [ ] Hardware compatibility testing
- [ ] Performance testing
- [ ] Security testing

#### Dependencies
- Architecture migration
- C language modernization

#### Risks
- Complex driver interfaces
- Hardware availability for testing
- Linux compatibility complexity
- Performance overhead


---

### 6. Security Model Modernization

#### Current State
- Unix-style permissions
- No capability-based security
- No mandatory access control

#### Target State
- Capability-based security
- SELinux/AppArmor equivalent
- Enhanced memory safety

#### Migration Steps

**Phase 1: Design**
- [ ] Design capability-based security model
- [ ] Design MAC framework
- [ ] Plan migration strategy
- [ ] Evaluate existing frameworks

**Phase 2: Foundation**
- [ ] Implement capability system
- [ ] Implement MAC framework
- [ ] Update kernel for security
- [ ] Update servers for security

**Phase 3: Integration**
- [ ] Integrate with filesystem
- [ ] Integrate with IPC
- [ ] Integrate with drivers
- [ ] Update userland tools

**Phase 4: Testing**
- [ ] Security testing
- [ ] Performance testing
- [ ] Compatibility testing
- [ ] Documentation

#### Dependencies
- C language modernization (for memory safety)
- Architecture migration

#### Risks
- Complex security model
- Performance impact
- Compatibility issues
- Learning curve


---

### 7. Network Stack Modernization

#### Current State
- BSD-derived network stack
- Limited protocol support
- Poor IPv6 support

#### Target State
- Modern TCP/IP stack
- Full IPv6 support
- Modern TCP features

#### Migration Steps

**Phase 1: Evaluation**
- [ ] Evaluate lwIP
- [ ] Evaluate FreeBSD network stack
- [ ] Evaluate other options
- [ ] Choose best option

**Phase 2: Implementation**
- [ ] Integrate chosen stack
- [ ] Implement IPv6 support
- [ ] Implement modern TCP features
- [ ] Update network drivers

**Phase 3: Testing**
- [ ] Network performance testing
- [ ] Protocol compliance testing
- [ ] Security testing
- [ ] Compatibility testing

#### Dependencies
- Driver model modernization
- Architecture migration

#### Risks
- Complex network stack
- Performance regressions
- Compatibility issues
- Security vulnerabilities


---

### 8. Testing Framework Migration

#### Current State
- ATF testing framework
- Limited test coverage
- No integration tests

#### Target State
- Modern testing framework
- High test coverage
- Integration and fuzzing tests

#### Migration Steps

**Phase 1: Evaluation**
- [ ] Evaluate Google Test
- [ ] Evaluate Catch2
- [ ] Evaluate Rust testing
- [ ] Choose framework

**Phase 2: Implementation**
- [ ] Integrate chosen framework
- [ ] Migrate existing tests
- [ ] Set up CI integration
- [ ] Add coverage reporting

**Phase 3: Expansion**
- [ ] Add integration tests
- [ ] Add fuzzing tests
- [ ] Add performance tests
- [ ] Increase coverage

#### Dependencies
- Build system migration
- C language modernization

#### Risks
- Test migration complexity
- Maintaining test compatibility
- CI integration issues


---

### 9. Bootloader Modernization

#### Current State
- Legacy bootloader
- No UEFI support
- No secure boot

#### Target State
- Modern bootloader (GRUB2/systemd-boot)
- UEFI support
- Secure boot support

#### Migration Steps

**Phase 1: Evaluation**
- [ ] Evaluate GRUB2
- [ ] Evaluate systemd-boot
- [ ] Evaluate other options
- [ ] Choose bootloader

**Phase 2: Implementation**
- [ ] Integrate chosen bootloader
- [ ] Implement UEFI support
- [ ] Implement secure boot
- [ ] Update boot process

**Phase 3: Testing**
- [ ] Boot testing
- [ ] UEFI testing
- [ ] Secure boot testing
- [ ] Compatibility testing

#### Dependencies
- Architecture migration

#### Risks
- Boot complexity
- UEFI implementation
- Secure boot complexity
- Hardware compatibility


---

### 10. Crypto Libraries Modernization

#### Current State
- ✅ OpenSSL 0.9.8/1.0.1p — **полностью заменён**
- ✅ wolfSSL 5.9.1 — основной крипто-провайдер
- ✅ libhcrypto (heimdal) — для Kerberos (OpenSSL-совместимый API)
- Нет Rust crypto в production (отложено)

#### Target State
- ✅ wolfSSL как sole crypto provider
- ✅ libhcrypto для heimdal (OpenSSL ABI-совместимость)
- OpenSSL полностью удалён из дерева

#### Migration Steps

**Phase 1: Быстрые победы (libsaslc, libfetch, libevent, pkg_install)**
- [x] Заменить `<openssl/*.h>` на `<wolfssl/openssl/*.h>`
- [x] Заменить `-lcrypto`/`-lssl` на `-lwolfssl` в LDADD
- [x] Настроить wolfSSL с OPENSSL_COMPAT_DEFINES
- [x] 7 компонентов переведены на wolfSSL

**Status**: ✅ COMPLETED

**Phase 2: Средняя сложность (netpgp, tcpdump, dhcp)**
- [x] netpgp: BIGNUM, SHA, RSA, DSA, AES — wolfSSL compat слой покрывает
- [x] tcpdump: проверен, не требует OpenSSL напрямую
- [x] dhcp: переведён на wolfSSL
- [x] 3 компонента переведены на wolfSSL

**Status**: ✅ COMPLETED

**Phase 3: Высокая сложность (heimdal → libhcrypto)**
- [x] Собран libhcrypto (heimdal's own crypto library) из dist/lib/hcrypto/
- [x] LibTomMath интегрирован для BN операций
- [x] config.h: HAVE_OPENSSL → HAVE_HCRYPTO
- [x] crypto-headers.h переключен на `<hcrypto/*>` включает
- [x] 10+ Makefile'ов обновлены: `-lcrypto` → `-lhcrypto`
- [x] heimdal собирается без OpenSSL (проверено препроцессором)

**Status**: ✅ COMPLETED (подробно: `planning/15_crypto_migration.md`)

**Phase 4: Очистка — OpenSSL удалён**
- [x] `crypto/Makefile.openssl` — удалён
- [x] `crypto/external/bsd/openssl/` — удалён с диска
- [x] `crypto/external/bsd/Makefile` — openssl убран из SUBDIR
- [x] heimdal SSLBASE — убран из Makefile.inc и libhx509/Makefile
- [x] netpgp CLI (7 Makefile'ов): `-lcrypto` → `-lwolfssl`
- [x] dhcp/Makefile.inc: `-lcrypto` → `-lwolfssl`
- [x] tests: libcrypto subdir убран, ссылки обновлены
- [x] planning/15_crypto_migration.md — полная документация

**Status**: ✅ COMPLETED

#### Dependencies
- C language modernization (независимо)

#### Risks
- ✅ API compatibility — решён через wolfSSL compat слой + libhcrypto
- ✅ Security — wolfSSL 5.9.1 активно поддерживается
- ✅ Performance — wolfSSL benchmarks в `docs/wolfssl-performance-report.md`


---

## Migration Dependencies Graph

```
Build System (CMake)
    ├─> Architecture Migration (x86_64/ARM64)
    │       ├─> Filesystem Migration (ext4)
    │       ├─> Driver Model Modernization
    │       └─> Bootloader Modernization
    ├─> C Language Modernization (C17 + Rust)
    │       ├─> Security Model Modernization
    │       ├─> Crypto Libraries Modernization
    │       └─> Network Stack Modernization
    ├─> NetBSD Dependency Reduction (planning/10)
    │       ├─> pkgsrc migration (tools, libs, externals)
    │       ├─> musl libc migration
    │       └─> GergiOS rebranding
    └─> Testing Framework Migration
            └─> All other migrations (for testing)
```

## Risk Mitigation Strategies

### Technical Risks
- **Prototype First**: Always create prototypes before full migration
- **Parallel Development**: Maintain old and new systems during transition
- **Comprehensive Testing**: Extensive testing before deprecation
- **Rollback Plans**: Ability to rollback if migration fails

### Organizational Risks
- **Developer Training**: Provide training for new technologies
- **Documentation**: Comprehensive documentation for all changes
- **Communication**: Regular communication about migration progress
- **Community Involvement**: Engage community in migration process

### Timeline Risks
- **Buffer Time**: Add buffer time to estimates
- **Priority Adjustment**: Adjust priorities based on progress
- **Scope Management**: Be willing to adjust scope if needed
- **Milestone Reviews**: Regular milestone reviews

## Success Metrics

### Build System
- Build time reduced by 50%
- Parallel build support working
- IDE integration working

### Architecture
- x86_64 and ARM64 fully supported
- Performance improved by 30%
- i386 successfully deprecated

### C Language
- 50% of new code in Rust
- Memory safety incidents reduced by 80%
- C11 features used throughout

### Filesystem
- ext4 as default filesystem
- Migration tools working
- Performance improved by 40%

### Security
- Capability-based security implemented
- Security incidents reduced by 70%
- Compliance with modern security standards

## Conclusion

This migration roadmap provides a structured approach to modernizing Minix while minimizing risk and ensuring continuity. The phased approach allows for incremental progress with regular validation and adjustment points.
