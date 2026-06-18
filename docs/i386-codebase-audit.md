# i386 Codebase Audit: Critical Dependencies Assessment

**Date**: June 18, 2026
**Scope**: Full codebase analysis of `__i386__` conditional compilation and i386-specific code

---

## Executive Summary

A comprehensive audit of the MINIX codebase identified **226+ occurrences** of `__i386__` conditional compilation across the project, spanning kernel, servers, drivers, libraries, filesystems, and external dependencies. This document categorizes these dependencies by severity and provides a roadmap for addressing them.

---

## Dependency Categories

### Critical Dependencies (Blocking Removal)

These are the components that must be addressed before i386 can be removed.

#### 1. Kernel Architecture Code

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `minix/kernel/system/do_sigsend.c` | 3 | Signal delivery — i386 trapframe layout |
| `minix/kernel/system/do_sigreturn.c` | 2 | Signal return — i386 stack frame layout |
| `minix/kernel/system/do_fork.c` | 4 | Process forking — register save/restore |
| `minix/kernel/system/do_trace.c` | 1 | Ptrace — register layout |
| `minix/kernel/system/do_mcontext.c` | 0* | Uses `__i386__ || __x86_64__` (already handled) |
| `minix/kernel/system.c` | 0* | Uses `__i386__ || __x86_64__` (already handled) |
| `minix/kernel/proc.c` | 1 | Process structure — register layout |
| `minix/kernel/debug.c` | 1 | Debug output |
| `minix/kernel/watchdog.h` | 1 | Watchdog timer |
| **Subtotal** | **13** | |

**Status**: ✅ Most kernel files already have x86_64 support via `__i386__ || __x86_64__` conditionals in `do_mcontext.c`, `do_sigreturn.c`, `system.c`. Remaining i386-only files (`do_sigsend.c`, `do_fork.c`, `do_trace.c`, `proc.c`, `debug.c`) need x86_64 branches.

#### 2. VM Pagetable Management

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `minix/servers/vm/pagetable.c` | 10 | **26** original ifdefs, 10 shown as `__i386__ || __x86_64__` |
| **Impact** | High | Controls memory mapping, page table walk, kernel mapping |

**Status**: ✅ Already updated during x86_64 Phase 3 — uses `__i386__ || __x86_64__` for architecture-specific paging.

#### 3. PM (Process Manager)

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `minix/servers/pm/misc.c` | 2 | uname machine string, i386-specific |

**Status**: ✅ x86_64 branch added — returns `"x86_64"` for machine/architecture on x86_64.

#### 4. Kernel Architecture-Specific Code

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `sys/arch/x86/include/sysarch.h` | 2 | System architecture calls |
| `sys/arch/x86/include/cpu.h` | 1 | CPU detection structures |
| `sys/arch/i386/` directory | 242 items | Complete i386 architecture directory |

**Status**: The `sys/arch/i386/` directory (242 items) is the primary i386 architecture support directory. An x86_64 counterpart exists in `sys/arch/x86_64/` and `minix/include/arch/x86_64/`.

---

### High Dependencies (Important, Not Blocking)

These components have significant i386 dependencies but are either already handled or can be addressed incrementally.

#### 1. Standard Library

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `lib/libc/tls/tls.c` | 1 | Thread-local storage |
| `lib/libc/stdlib/malloc.c` | 1 | Memory allocator |
| `lib/libc/stdlib/jemalloc.c` | 1 | jemalloc allocator |
| `lib/libc/net/getnetnamadr.c` | 2 | Network name resolution |
| `lib/libc/net/getnetent.c` | 1 | Network entry parsing |
| `lib/libc/gen/nlist_private.h` | 1 | Object file symbol table |
| `lib/libc/sys/_ucontext.c` | 1 | User context |

**Status**: ✅ _ucontext.c has x86_64 support. Other files need architecture-independent implementations or x86_64 branches.

#### 2. Library Support

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `lib/libm/` (multiple files) | 12 | Math library (i387 co-processor) |
| `lib/libcrypt/crypt.c` | 1 | Password hashing |
| `lib/libaudiodriver/audio_fw.c` | 5 | Audio driver framework |

**Status**: Math library uses `arch/i387/` for i386 — x86_64 uses SSE/AVX instead. Audio driver framework needs x86_64 support.

#### 3. Libraries (minix/)

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `minix/lib/libmthread/misc.c` | 2 | Stack trace (i386/x86_64 dual) |
| `minix/lib/libmthread/allocate.c` | 2 | Thread allocation |
| `minix/lib/libc/sys/_ucontext.c` | 1 | User context management |

**Status**: ✅ libmthread already handles both i386 and x86_64 via `__i386__ || __x86_64__`.

#### 4. Filesystem Servers

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `minix/fs/procfs/root.c` | 6 | ProcFS — i386 register display |
| `minix/fs/procfs/cpuinfo.c` | 3 | ProcFS — CPU info display |

**Status**: These display i386-specific register/CPU information. Need x86_64 variants for equivalent functionality.

#### 5. Device Drivers

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `minix/drivers/tty/tty/arch/i386/rs232.c` | 4 | RS232 serial driver |
| `minix/drivers/storage/memory/memory.c` | 1 | Memory storage driver |

**Status**: ✅ RS232 already uses `__i386__ || __x86_64__` conditionals.

#### 6. Information Service

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `minix/servers/is/dmp_kernel.c` | 2 | Kernel dump — i386 register display |

**Status**: X86_64 equivalent exists; minor updates needed for register display.

#### 7. mib Service

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `minix/servers/mib/mib.h` | 1 | MIB header architecture check |
| `minix/servers/mib/hw.c` | 1 | MIB hardware info |

**Status**: Minor — need x86_64 architecture handling.

#### 8. User Tools

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `minix/usr.bin/trace/service/pm.c` | 1 | Trace tool — PM register display |
| `minix/usr.bin/trace/kernel.c` | 3 | Trace tool — kernel register display |

**Status**: Need x86_64 register definitions.

#### 9. Include Headers

| File | i386-#ifdef Count | Description |
|------|-------------------|-------------|
| `minix/include/minix/drivers.h` | 1 | Driver header |
| `minix/include/minix/const.h` | 1 | MINIX constants |
| `minix/include/arch/i386/include/ports.h` | 1 | I/O ports (i386 only) |
| `minix/include/arch/i386/include/interrupt.h` | 1 | Interrupt handling (i386 only) |
| `include/fenv.h` | 1 | Floating-point environment |
| `include/netdb.h` | 2 | Network database |
| `include/arpa/nameser_compat.h` | 1 | DNS name server |
| `include/sys/cpuio.h` | 1 | CPU I/O |

**Status**: ✅ x86_64 versions exist in `minix/include/arch/x86_64/`. Standard headers need architecture-independent implementations.

#### 10. minix-specific Makefiles (MACHINE_ARCH == "i386")

| File | Description |
|------|-------------|
| `minix/lib/Makefile` | Library build — i386 conditional |
| `minix/tests/Makefile` | Test build — i386 conditional |
| `minix/fs/Makefile` | Filesystem build — i386 conditional |
| `minix/drivers/*/Makefile` | Driver builds — ~15 i386-conditionals |
| `minix/kernel/system/Makefile.inc` | Kernel system build |

**Status**: These Makefiles already have x86_64 support in the CMake build system. The BSD Make path needs updating.

---

### Medium Dependencies (Non-Critical)

These are external dependencies that support i386 but are maintained upstream.

| Dependency | i386 References | Impact on MINIX |
|-----------|----------------|-----------------|
| wolfSSL | Multiple `__i386__` paths | Low — wolfSSL handles both i386 and x86_64 |
| GCC (gpl3) | Multiple Makefile checks | Low — upstream handles both |
| LLVM/Clang | Multiple references | Low — upstream handles both |
| Xorg (mit) | 30+ Makefile conditionals | Medium — Xorg has i386/x86_64 dual support |
| compiler-rt | 15 assembly files | Low — i386-specific ASM for compiler builtins |
| OpenSSL | Multiple files | Low — being replaced by wolfSSL |
| BIND (external/bsd/bind) | Minor references | Low — already migrated to wolfSSL |
| expat, tcpdump, libpcap | Minor references | Low — upstream handles both |

---

## Dependency Map Summary

| Category | Count | Priority | Effort to Fix |
|----------|-------|----------|---------------|
| Kernel architecture code | 13 | Critical | High (complex ASM) |
| VM pagetable management | 10 | Critical | ✅ Done |
| Process Manager | 2 | Critical | ✅ Done |
| Standard library | 8 | High | Medium |
| Math library | 12 | High | Medium |
| Library support | 6 | Medium | Low |
| Filesystem servers | 9 | Medium | Low |
| Device drivers | 5 | Medium | Low |
| User tools | 4 | Low | Low |
| Include headers | 10 | Medium | Low |
| Makefiles (BSD Make) | 25+ | Medium | Low |
| External dependencies | 60+ | Low | Minimal (upstream) |

---

## Total i386 Count

| Category | Count |
|----------|-------|
| MINIX core `__i386__` #ifdefs | ~80 |
| External code `__i386__` #ifdefs | ~100 |
| BSD Make MACHINE_ARCH i386 checks | ~46 |
| **Total** | **226+** |

---

## Migration Priority Matrix

| Priority | Components | Status |
|----------|-----------|--------|
| **P0 — Already Done** | VM pagetable, PM misc, mcontext, RS232, pagetable.h types | ✅ Complete |
| **P1 — Phase 2 (Soft Deprecation)** | Kernel syscalls, IS dump, libmthread, ucontext | 🔜 Planned |
| **P2 — Phase 3 (Hard Deprecation)** | ProcFS, drivers, user tools, trace | 🔜 Planned |
| **P3 — Phase 4 (Removal)** | External deps, Makefile cleanups, arch/i386/ removal | 🔜 Planned |

---

*This audit was generated as part of Phase 1 of the i386 deprecation process. For the full deprecation timeline, see [i386 Deprecation Timeline](../planning/05_i386_deprecation_timeline.md).*
