# NetBSD Dependency Audit and Migration Plan

> **Part of**: Overall modernization roadmap (`planning/03_migration_roadmap.md`)
> **Related**: `planning/02_legacy_dependencies.md`, `planning/09_c_language_modernization.md`
> **Status**: Audit complete — migration not started

---

## 1. Introduction

### 1.1 Background

MINIX 3 (this project) was originally forked from **NetBSD** around 2005-2006. The MINIX team took the NetBSD userland, libraries, build system, and kernel infrastructure, and built a new **microkernel** on top of it. The result is a hybrid:

- **MINIX-native**: `minix/kernel/`, `minix/servers/`, `minix/drivers/`, `minix/fs/`, `minix/net/`
- **NetBSD-imported**: Everything else — libraries (`lib/`), utilities (`bin/`, `sbin/`, `usr.bin/`, `usr.sbin/`), build system (`share/mk/`), kernel headers (`sys/sys/`, `sys/ufs/`), external packages (`external/`), shared code (`common/`)

The project is now being rebranded as **GergiOS** (see Section 5). MINIX remains the microkernel heritage/base, but the system as a whole moves forward under a new identity.

### 1.2 Why Migrate Away from NetBSD?

| Motivation | Explanation |
|-----------|-------------|
| **Aging codebase** | NetBSD userland from ~2015 (NetBSD 7.x era) — outdated tools, libraries, security |
| **Maintenance burden** | Syncing with NetBSD upstream is effort-intensive; MINIX diverges significantly |
| **pkgsrc availability** | Most NetBSD tools can be installed via pkgsrc on modern systems |
| **Modern alternatives** | musl, FreeBSD userland, LLVM/Clang, Rust provide better foundations |
| **Rebranding opportunity** | "GergiOS" needs its own identity — shedding NetBSD baggage enables this |

---

## 2. NetBSD Dependency Map

### 2.1 Layer Diagram

```
                    ┌─────────────────────────────────┐
                    │      GergiOS Identity Layer      │
                    │  (boot, motd, uname, branding)   │
                    ├─────────────────────────────────┤
                    │    MINIX Microkernel (kernel,    │
                    │     servers, drivers, fs, net)   │
                    ├──────────┬──────────┬────────────┤
                    │  NetBSD  │  NetBSD  │  NetBSD    │
                    │  libc    │  Userland│  Build Sys │
                    │ (lib/)   │(bin,usr) │(share/mk/) │
                    ├──────────┴──────────┴────────────┤
                    │   NetBSD Kernel Headers (sys/)   │
                    │   NetBSD Common Code (common/)   │
                    └─────────────────────────────────┘
```

### 2.2 Full Dependency Inventory

| # | Component | Location | NetBSD Origin | Size (est.) | Critical? | Replaceable? |
|---|-----------|----------|---------------|-------------|-----------|-------------|
| 1 | **libc** | `lib/libc/` + `common/lib/libc/` | ✅ 100% | ~500 files | 🔴 **Critical** | 🟡 Complex |
| 2 | **libm** | `lib/libm/` | ✅ 100% | ~150 files | 🔴 **Critical** | 🟡 Complex |
| 3 | **sys headers** | `sys/sys/*.h` | ✅ 100% | ~350 files | 🔴 **Critical** | 🔴 Very Hard |
| 4 | **BSD Make** | `share/mk/*.mk` | ✅ 100% | 37 files | 🟡 Important | 🟢 **Easy** ✅ |
| 5 | **Userland utils** | `bin/`, `sbin/`, `usr.bin/`, `usr.sbin/` | ✅ 90% | ~250 tools | 🟡 Important | 🟢 **Easy** |
| 6 | **Libraries** | `lib/{edit,curses,form,menu,pci,prop,puffs,...}` | ✅ 100% | ~40 libs | 🟢 Low | 🟢 **Easy** |
| 7 | **Kernel FS** | `sys/ufs/`, `sys/fs/` | ✅ 100% | ~80 files | 🟡 Important | 🟡 Complex |
| 8 | **UVM/VMM** | `sys/uvm/` | ✅ 100% | ~40 files | 🟡 Important | 🟡 Complex |
| 9 | **boot lib** | `sys/lib/libsa/` | ✅ 100% | ~80 files | 🔴 **Critical** | 🟡 Complex |
| 10 | **External pkg** | `external/{bsd,gpl2,gpl3,mit,public-domain}/` | ✅ 90% | ~50 packages | 🟢 Low | 🟢 **Easy** |
| 11 | **Crypto** | `crypto/external/{bsd,gpl2}/` | ✅ 100% | ~5 packages | 🟡 Important | 🟢 **Easy** ✅ |
| 12 | **Common code** | `common/lib/libc/` (atomic, md, string, stdlib, sys) | ✅ 100% | ~100 files | 🔴 **Critical** | 🟡 Complex |
| 13 | **games** | `games/` | ✅ 100% | ~30 games | 🟢 Low | 🟢 **Easy** |
| 14 | **Documentation** | `share/man/`, `share/doc/` | ✅ 100% | ~50 files | 🟢 Low | 🟢 **Easy** |
| 15 | **i18n/locale** | `share/i18n/`, `share/locale/` | ✅ 100% | ~500 files | 🟢 Low | 🟢 **Easy** |
| 16 | **termcap/terminfo** | `share/terminfo/` | ✅ 100% | ~1 file (db) | 🟢 Low | 🟢 **Easy** |

### 2.3 Dependency Graph (Critical Path)

```
MINIX Microkernel
    ├── Needs: libc (system calls) ← 🔴 Critical (complex replacement)
    │       └── Needs: sys/sys/ headers (types, structures) ← 🔴 Critical (very hard)
    ├── Needs: common/lib/libc/ (shared kernel/userland code) ← 🔴 Critical
    ├── Needs: boot library (sys/lib/libsa/) ← 🔴 Critical
    └── Needs: libm (math) ← 🟡 Important
    │
    └── CAN BE REPLACED:
            ├── Userland utils → pkgsrc 🟢
            ├── BSD Make → CMake 🟢 (in progress)
            ├── External packages → pkgsrc 🟢
            ├── Libraries (curses, edit, etc.) → pkgsrc 🟢
            ├── games → pkgsrc 🟢
            ├── crypto/openssl → wolfSSL 🟢 (in progress)
            ├── locale/i18n → pkgsrc 🟢
            └── terminfo → pkgsrc 🟢
```

---

## 3. Migration Phases

### 3.1 Phase 0: Quick Wins (Already In Progress)

| Task | Status | Effort |
|------|--------|--------|
| **BSD Make → CMake** | ✅ Phase 1-3 complete | 3 months |
| **OpenSSL 0.9.8 → wolfSSL** | ✅ Prototype complete | 2 months |
| **GergiOS branding** (boot, uname, motd) | ❌ Not started | 1 week |

### 3.2 Phase 1: Easy Replacements (pkgsrc)

**Goal**: Remove ~60% of NetBSD code by delegating to pkgsrc.

**Strategy**: Instead of maintaining NetBSD userland in-tree, install packages via pkgsrc at build time. The build system produces a minimal GergiOS core + pkgsrc overlay.

#### Components to remove:

| Component | Replacement | Why easy |
|-----------|-------------|----------|
| `bin/` (cat, cp, ls, mv, rm, sh, test...) | pkgsrc/coreutils, pkgsrc/bash | Standard POSIX tools |
| `usr.bin/` (find, grep, sed, awk, diff...) | pkgsrc/findutils, pkgsrc/gnugrep, pkgsrc/gawk | Available in pkgsrc |
| `sbin/` (mount, fsck, newfs, ifconfig...) | pkgsrc + GergiOS-native wrappers | Some need MINIX-specific logic |
| `usr.sbin/` (syslogd, inetd, sysctl...) | pkgsrc or GergiOS-native | Can be replaced incrementally |
| `lib/{curses,edit,form,menu,pci,prop,puffs,refuse,terminfo}` | pkgsrc | Not used by kernel |
| `external/` (LLVM, GCC, GDB, tmux, less, nvi...) | pkgsrc | Independent projects |
| `games/` | pkgsrc | Not system-critical |
| `share/{man,locale,i18n,terminfo,misc}` | pkgsrc | Data files |
| `lib/libwrap/` (tcp_wrappers) | pkgsrc | Deprecated technology |
| `lib/libtelnet/` | Remove entirely | Telnet is deprecated |
| `lib/libkvm/` | pkgsrc or remove | MINIX doesn't use kvm |

#### Migration process:
1. Add pkgsrc bootstrap to build system (qemu + pkgin or similar)
2. For each component: remove from-tree, add to pkgsrc manifest
3. Verify with test suite

**Effort**: 4-8 weeks
**Risk**: Low (pkgsrc is well-maintained, MINIX already supports pkgsrc)

### 3.3 Phase 2: Crypto Consolidation

| Component | Status | Action |
|-----------|--------|--------|
| `crypto/external/bsd/openssl/` (OpenSSL 0.9.8) | ❌ EOL 2015 | Replace with wolfSSL ✅ in progress |
| `crypto/external/bsd/heimdal/` (Kerberos) | ❌ Unmaintained | Remove (MK KERBEROS=no already default) |
| `crypto/external/bsd/libsaslc/` (SASL) | ❌ Unused | Remove |
| `crypto/external/bsd/netpgp/` (PGP) | ❌ Unused | Remove |
| `crypto/external/gpl2/wolfssl/` | ✅ Modern | **Keep** as primary crypto provider |

**Effort**: 2-4 weeks
**Risk**: Low (wolfSSL prototype already exists)

### 3.4 Phase 3: BSD Make Retirement

| Task | Status |
|------|--------|
| CMake build for kernel | ✅ Complete |
| CMake build for servers | ✅ Complete |
| CMake build for drivers | ✅ Complete |
| CMake build for libraries | ✅ Complete |
| CMake build for userland | ✅ Complete |
| CMake build for tests | ✅ Complete |
| CMakePresets.json | ✅ Complete |
| cmake-build.sh | ✅ Complete |
| **Make `build.sh` point to CMake** | ❌ Remaining |
| **Deprecate BSD Make entirely** | ❌ Future |

**Effort**: 2-4 weeks remaining
**Risk**: Low (CMake covers all components)

### 3.5 Phase 4: libc Migration (Complex)

**Strategy**: Replace NetBSD libc with **musl libc** (or **FreeBSD libc**).

**Why musl?**
- Clean, modern C99/C11 codebase
- MIT license (vs NetBSD's BSD-with-advertising)
- Designed for embedded systems
- Good POSIX compliance
- Active community
- Smaller footprint

**Challenges:**

| Challenge | Explanation | Mitigation |
|-----------|-------------|------------|
| **Syscall layer** | MINIX syscalls (PM, VFS, VM) need libc wrappers | Port `minix/lib/libc/` syscall stubs to musl |
| **Thread-local storage** | musl uses TLS differently | Audit and adapt |
| **Signal handling** | MINIX signals have custom semantics | Keep MINIX signal wrappers |
| **common/lib/libc/** | Shared kernel/userland code — musl doesn't have `rb.c`, `sha2.c`, etc. | Keep `common/lib/libc/` as-is |
| **errno** | musl uses `__errno_location()` vs NetBSD's `__errno()` | Macro adaptation |
| **__minix** ifdefs | NetBSD libc has `#ifdef __minix` blocks | Remove or port to musl |

**Migration approach:**
1. Add musl as optional libc alongside NetBSD libc
2. Port syscall wrappers from `minix/lib/libc/` to musl ABI
3. Build a minimal GergiOS userland with musl
4. Test compatibility (start with static binaries)
5. Switch default to musl once validated

**Effort**: 8-16 weeks
**Risk**: Medium-High (syscall ABI, TLS, signals)

### 3.6 Phase 5: Math Library Migration

**Strategy**: Replace `lib/libm/` (NetBSD libm, from FreeBSD 5.x era) with musl's libm.

**Alternative**: Use OpenLibm (clean, portable, used by Julia, FreeBSD, etc.)

**Effort**: 2-4 weeks (done alongside libc migration)
**Risk**: Low (math functions are well-standardized)

### 3.7 Phase 6: Boot Library Cleanup

**Strategy**: The boot library (`sys/lib/libsa/`) currently supports multiple filesystem and network protocols. MINIX only needs:
- `minixfs3.c` (MINIX FS v3)
- `loadfile_elf32.c` / `loadfile_elf64.c` (ELF loading)
- `printf.c`, `alloc.c` (minimal runtime)
- `tftp.c` (network boot)
- `dev.c`, `net.c` (device/network abstraction)

**Action**: Remove unused filesystems (ffsv1, ffsv2, lfsv1, lfsv2, ext2fs, cd9660, ustarfs, nfs, bootp, rarp, dosfs, etc.)

**Effort**: 1-2 weeks
**Risk**: Low (MINIX only boots from MFS/ext2/minixfs3)

### 3.8 Phase 7: VFS/Filesystem Audit (Long-term)

The NetBSD VFS layer (`sys/ufs/`, `sys/fs/`) is used by MINIX filesystem servers. These are not directly replaceable without rewriting the MINIX FS servers.

| Component | Usage in MINIX | Action |
|-----------|---------------|--------|
| `sys/ufs/ffs/` | Not used (MINIX has MFS) | ❌ Keep for reference |
| `sys/ufs/lfs/` | Not used | ❌ Can remove |
| `sys/fs/chfs/` | Not used | ❌ Can remove |
| `sys/fs/ext2fs/` | MINIX `minix/fs/ext2/` uses ext2fs headers | 🔴 Needed |
| `sys/fs/v7fs/` | Not used | ❌ Can remove |
| `sys/fs/unicode.h` | Possibly used by other FS | 🤷 Check |

**Effort**: 4-8 weeks (for cleanup, not full replacement)
**Risk**: Low if only cleanup; High if full VFS replacement

### 3.9 Summary Timeline

```
Q3 2026: Phase 0 (branding) + Phase 1 (pkgsrc) + Phase 2 (crypto)
Q4 2026: Phase 3 (BSD Make done) + Phase 4 start (libc)
Q1 2027: Phase 4 (libc) + Phase 5 (libm)
Q2 2027: Phase 6 (boot library) + Phase 7 (VFS audit)
```

---

## 4. Detailed Component Analysis

### 4.1 libc — The Hardest Dependency

**Why it's critical**: Every process links against libc. MINIX syscalls go through libc wrappers.

**What MINIX actually needs from libc:**

```
libc needed by MINIX servers (PM, VFS, VM, RS, DS, etc.):
  stdio:    printf, fprintf, sprintf, snprintf, vprintf
  stdlib:   malloc, free, realloc, calloc, atoi, strtol, exit, getenv
  string:   memcpy, memmove, memset, strlen, strcpy, strcmp, strcat,
            strncpy, strncmp, strchr, strrchr, strstr, strsep, strlcpy
  signal:   sigaction, sigprocmask, sigemptyset, sigfillset
  time:     time, clock_gettime, nanosleep, gettimeofday
  errno:    errno, __errno, strerror
  syscall:  _syscall, __syscall (MINIX custom)
  pthread:  mutex_lock, mutex_unlock, thread_create (via libc pthread stubs)
  math:     (often not needed by servers, only by userland)
```

**Migration plan to musl:**
1. Create `minix/lib/libc-musl/` — syscall wrappers for musl ABI
2. Map MINIX-specific syscall numbering to musl `__syscall()` interface
3. Handle `__errno_location()` → `errno` translation
4. Port `common/lib/libc/` algorithms (rb.c, sha2.c, atomics) as standalone library
5. Build a minimal busybox-style userland with musl
6. Replace `lib/libc/` symlink with musl

### 4.2 `common/lib/libc/` — Shared Kernel/Userland Code

**Why it's critical**: This code runs both in kernel context (via `libminc`) and in userland (via `libc`). MINIX libminc is a standalone library without a full libc.

| File | Used by | Notes |
|------|---------|-------|
| `atomic/*.c` | kernel, servers | C11 atomics via CAS |
| `gen/rb.c` | kernel (VM), servers | Red-black tree — heavily used |
| `gen/radixtree.c` | kernel | Radix tree |
| `gen/ptree.c` | kernel | Priority tree |
| `gen/rpst.c` | kernel | Range-partitioning tree |
| `inet/*.c` | network | htonl, htons |
| `md/*.c` | kernel, crypto | MD4, MD5 |
| `string/*.c` | kernel, libc | memcpy, memset, strlen |
| `stdlib/*.c` | kernel, libc | strtol, random, heapsort |
| `quad/*.c` | kernel | 64-bit ops on 32-bit |

**Action**: Keep `common/lib/libc/` as a GergiOS-native utility library. It has no NetBSD-specific dependencies — it's generic C code portable to any libc.

### 4.3 `sys/lib/libsa/` — Boot Library

MINIX's bootloader uses this. It has ~40 files but MINIX only needs ~15:

**Keep (MINIX needs):**
- `alloc.c`, `printf.c`, `snprintf.c`, `strerror.c`, `errno.c`
- `dev.c`, `dev_net.c`, `files.c`, `fstat.c`, `getfile.c`, `open.c`, `read.c`, `close.c`, `lseek.c`, `stat.c`
- `loadfile.c`, `loadfile_elf32.c`, `loadfile_elf64.c`
- `minixfs3.c`, `minixfs3.h`
- `net.c`, `netif.c`, `ether.c`, `arp.c`, `ip.c`, `udp.c`, `tftp.c`
- `bootcfg.c`
- `exit.c`, `panic.c`
- `byteorder.c`
- `globals.c`, `twiddle.c`

**Remove (unused):**
- `cd9660.c`, `dosfs.c`, `ext2fs.c`, `ffsv1.c`, `ffsv2.c`, `lfsv1.c`, `lfsv2.c`, `nfs.c`, `ufs.c`, `nullfs.c`, `ustarfs.c`
- `bootp.c`, `rarp.c`, `rpc.c`
- `loadfile_aout.c`, `loadfile_ecoff.c`
- `lookup_elf32.c`, `lookup_elf64.c` (if not used)
- `ls.c` (debug utility)
- `fnmatch.c` (may be unused)

---

## 5. GergiOS Rebranding Concept

### 5.1 Philosophy

> **"MINIX"** — The microkernel heritage (internal, technical). Like "Linux" in "Android" — the kernel base.
> **"GergiOS"** — The product name (external, user-facing). The operating system the user interacts with.

This mirrors:
- **Android** (product) built on **Linux** (kernel)
- **macOS** (product) built on **XNU/Darwin** (kernel)
- **GergiOS** (product) built on **MINIX** (microkernel)

### 5.2 User-Facing Touchpoints

| Location | Current | Target | Priority |
|----------|---------|--------|----------|
| **Boot menu** (`etc/boot.cfg.default`) | `Start MINIX 3` | `Start GergiOS` | 🔴 High |
| **Kernel announce** (`minix/kernel/main.c`) | `MINIX 3.4.0` | `GergiOS 1.0 (MINIX 3.4.0)` | 🔴 High |
| **OS_NAME** (`minix/include/minix/config.h`) | `"Minix"` | `"GergiOS"` | 🔴 High |
| **OS_VERSION** | `"Minix 3.4.0 (GENERIC)"` | `"GergiOS 1.0 (GENERIC, MINIX 3.4.0)"` | 🔴 High |
| **motd** (`etc/motd`) | `MINIX 3 wiki...` | `GergiOS docs...` | 🟡 Medium |
| **uname -o** (via MIB) | `Minix` | `GergiOS` | 🟡 Medium |
| **uname -r** (via OS_RELEASE) | `3.4.0` | `1.0` (GergiOS version) | 🟡 Medium |
| **libc identification** | minix3 | gergios | 🟢 Low |
| **sysctl kern.ostype** | `Minix` | `GergiOS` | 🟡 Medium |
| **Website/Community** | `minix3.org` | `gergios.dev` (future) | 🟢 Low |
| **Man pages** (`minix/man/`) | `MINIX` references | `GergiOS` references | 🟢 Low |
| **Source file headers** | `Minix` in comments | `GergiOS` in comments | 🟢 Low |
| **Version file** (`etc/version`) | — | Add GergiOS version info | 🟢 Low |
| **makewhatis** database | MINIX | GergiOS | 🟢 Low |
| **Boot splash** (future) | MINIX logo | GergiOS logo | 🟢 Low |

### 5.3 Implementation Approach

**Internal reference** (keep "MINIX"):
- `minix/` directory name — stays
- `minix/include/minix/` headers — stay
- `__minix` preprocessor defines — stay
- Internal comments referencing MINIX heritage — keep

**User-facing** (change to "GergiOS"):
- `OS_NAME` in `minix/include/minix/config.h`
- Kernel `announce()` message
- Bootloader menu
- motd, issue, rc prompt
- uname output
- Package metadata
- Documentation and man pages

### 5.4 Versioning Scheme

```
GergiOS 1.0.0 "Aurora" (MINIX 3.4.0)
├── GergiOS major.minor.patch
│   ├── Major: architectural changes (new kernel, new libc)
│   ├── Minor: feature releases
│   └── Patch: bug fixes
├── Codename: marketing name per release
└── MINIX X.Y.Z: base microkernel version (internal reference)
```

### 5.5 Quick Branding Change (Phase 0)

The minimal change to establish GergiOS identity:

```c
// minix/include/minix/config.h
#define OS_NAME "GergiOS"
#define OS_RELEASE "1.0.0"     // GergiOS version
#define OS_VERSION OS_NAME " " OS_RELEASE " (MINIX 3.4.0, GENERIC)"
#define OS_CONFIG "GENERIC"
```

```c
// minix/kernel/main.c — announce() function
printf("\nGergiOS %s "
    "(MINIX microkernel 3.4.0)\n"
    "Copyright 2026, GergiOS Project\n",
    OS_RELEASE);
```

```makefile
# etc/boot.cfg.default
menu=Start GergiOS:load_mods /boot/default/mod*;multiboot /boot/default/kernel rootdevname=$rootdevname $args
menu=Start GergiOS (safe mode):load_mods /boot/default/mod*;multiboot /boot/default/kernel rootdevname=$rootdevname bootopts=-s $args
```

---

## 6. Risk Assessment

### 6.1 Migration Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| pkgsrc compatibility issues | Medium | Low | Test on QEMU before removing in-tree tools |
| musl libc ABI differences | High | Medium | Gradual migration: musl alongside NetBSD libc |
| Rebranding breaks scripts | Low | Low | `uname -s` still returns something consistent |
| Boot library cleanup breaks boot | Critical | Low | Keep all files until validated |
| VFS replacement breaks FS | Critical | Low | Keep existing VFS, only remove unused filesystems |

### 6.2 Rebranding Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| MINIX name recognition loss | Low | High | Keep "MINIX" in technical references |
| Config migration confusion | Low | Low | Version file documents the change |
| Package compatibility | Low | Low | OS_NAME change propagates to pkgin |

### 6.3 Dependencies Between Phases

```
Phase 0 (Branding) ──→ Phase 1 (pkgsrc) ──→ Phase 3 (BSD Make done)
                                                     │
                                                     ↓
Phase 6 (boot lib) ←─── Phase 7 (VFS) ←─── Phase 4 (libc) ←─── Phase 5 (libm)
```

Phases 0-3 can proceed in parallel. Phases 4-7 depend on earlier phases being stable.

---

## 7. Success Criteria

1. **GergiOS boots** with new branding (boot menu, kernel announce, uname)
2. **0 in-tree userland tools** — all provided via pkgsrc (optional; can keep minimal set)
3. **BSD Make not required** — CMake is the sole build system
4. **wolfSSL** is the sole crypto provider
5. **musl libc** (or similar) replaces NetBSD libc for userland
6. **Boot library** trimmed to <20 files
7. **100% of existing tests pass** after each phase
8. **Documentation** updated for GergiOS identity

---

## 8. Effort Summary

| Phase | Description | Effort | Risk | Priority |
|-------|-------------|--------|------|----------|
| **0** | GergiOS branding | 1 week | 🟢 Low | 🔴 High |
| **1** | pkgsrc migration (tools, libs, games, externals) | 4-8 weeks | 🟢 Low | 🔴 High |
| **2** | Crypto consolidation | 2-4 weeks | 🟢 Low | 🟡 Medium |
| **3** | BSD Make → CMake finalization | 2-4 weeks | 🟢 Low | 🔴 High |
| **4** | libc → musl | 8-16 weeks | 🟡 Medium | 🟡 Medium |
| **5** | libm → musl/OpenLibm | 2-4 weeks | 🟢 Low | 🟢 Low |
| **6** | Boot library cleanup | 1-2 weeks | 🟡 Medium | 🟢 Low |
| **7** | VFS/filesystem audit | 4-8 weeks | 🟡 Medium | 🟢 Low |

**Total estimated effort**: 24-48 weeks (spans Q3 2026 — Q2 2027)
**Code reduction**: ~70% of NetBSD code removed (saving ~15,000+ files)

---

## 9. Related Documents

- `planning/03_migration_roadmap.md` — overall roadmap (see Section 2: Architecture Migration)
- `planning/02_legacy_dependencies.md` — legacy dependency analysis
- `planning/05_i386_deprecation_timeline.md` — architecture deprecation
- `planning/06_openssl_to_wolfssl_migration.md` — crypto migration
- `planning/09_c_language_modernization.md` — C standard modernization
- `minix/include/minix/config.h` — OS_NAME, OS_RELEASE, OS_VERSION definitions
- `minix/kernel/main.c` — kernel announce() function
- `minix/servers/pm/misc.c` — uname service
- `minix/servers/mib/kern.c` — sysctl service
