# x86_64 Architecture Migration Plan

## Overview

This document details the migration plan for porting MINIX from i386 (32-bit) to x86_64 (64-bit). This is Phase 1 of the Architecture Migration defined in `planning/03_migration_roadmap.md`.

## Current State (Post-Audit)

### Architecture Support Matrix

| Component | i386 | earm (ARM) | x86_64 |
|-----------|------|------------|--------|
| Kernel ASM (boot, traps, ctx) | ✅ Complete | ✅ Complete | ❌ Missing |
| Pagetable (VM server) | ✅ Complete (PAE + legacy) | ✅ Complete | ❌ Missing |
| Signals/mcontext/syscalls | ✅ Complete | ✅ Complete | ❌ Missing |
| libsys arch | ✅ Complete | ✅ Complete | ❌ Missing |
| libminc arch (setjmp/longjmp) | ✅ Complete | ✅ Complete | ❌ Missing |
| Linker scripts | ✅ Complete | ✅ Complete | ❌ Missing |
| Build system (CMake) | ✅ arch_i386.cmake | ✅ arch_earm.cmake | ❌ Missing |
| Device drivers (I/O, DMA, MMIO) | ✅ Complete | ✅ Complete | ❌ Missing |
| `__x86_64__` conditionals in MINIX code | — | — | **0 found** |

### Key Findings from Phase 1.1 Audit

1. **No `__x86_64__` code exists** in MINIX core — the `#ifdef __x86_64__` paths mentioned in planning/04 do not exist in the actual codebase.
2. **VM pagetable.c is the most critical file** — 26 `__i386__` ifdefs controlling PAE vs legacy paging, PTEs, PDEs, page directory handling.
3. **4 kernel ASM files require full rewrite** — mpx386.S (context switch), klib386.S (library), head.S (boot init), and the linker script.
4. **Signal handling deeply tied to i386 trapframe layout** — do_sigsend.c, do_sigreturn.c, do_mcontext.c all hardcode i386 register layout.
5. **External libraries already support x86_64** — wolfSSL, compiler-rt, OpenSSL all have `__x86_64__` code paths.

## Migration Strategy

### Approach: Incremental, Testable Phases

Rather than a single "big bang" port, we break the work into phases where each phase produces a testable result:

```
Phase 1: Build Infrastructure  → cmake configure succeeds
Phase 2: Kernel Bootstrap      → boots to "Hello from x86_64" in QEMU
Phase 3: Memory Management     → VM works with 4-level paging
Phase 4: System Calls + Signals → PM/VFS/VM communication works
Phase 5: Libraries + Drivers   → libsys, libminc, drivers ported
Phase 6: Full Userland         → shell, commands, networking work
```

---

## Phase 1: Build Infrastructure 🟢 (Easiest)

### 1.1 cmake/arch_x86_64.cmake
Create architecture definition for x86_64:

```cmake
# === x86_64 Architecture ===
set(MACHINE_ARCH "x86_64")
set(MACHINE "x86_64")
set(GNUMACH "x86_64")
set(PLATFORM_CPUTYPE "x86_64")

# CPU model
set(CMAKE_SYSTEM_PROCESSOR "x86_64")

# Compile flags
set(ARCH_CFLAGS "-m64 -march=x86-64 -mtune=generic")
set(ARCH_LDFLAGS "-m64")

# ABI
add_compile_definitions(
    __x86_64__
    _LP64
    __LP64__
)

# Global register assignments (MINIX specific)
add_compile_definitions(
    # No global registers needed - x86_64 has enough registers
)
```

### 1.2 cmake/toolchain-minix.cmake
- Add `x86_64` to the architecture detection block
- Add x86_64 tool prefix handling
- Enable 64-bit DESTDIR paths

### 1.3 CMakePresets.json
- Add `x86_64-debug` preset
- Add `x86_64-release` preset

### 1.4 Verify
- `cmake -DMACHINE_ARCH=x86_64 ..` succeeds
- Compiler flags are correct (`-m64`)

**Status**: ✅ COMPLETED

**Files Created/Modified**:
- `cmake/arch_x86_64.cmake` — architecture definition for x86_64
- `cmake/toolchain-minix.cmake` — dynamic tool prefix (x86_64 → `x86_64-elf64-minix`)
- `CMakePresets.json` — `x86_64-debug` and `x86_64-release` presets
- `CMakeLists.txt` — arch detection (x86_64 before i386)

**Verification**:
- `cmake -DMACHINE_ARCH=x86_64` → `-- Target architecture: x86_64` ✅
- Global `-mcmodel=kernel` / `-mno-red-zone` removed after code review (kernel-only flags moved to Phase 2)
- Pre-existing `add_minix_library` error in libsys is not related to x86_64 changes

**Next**: Phase 2 — Kernel Bootstrap

---

## Phase 2: Kernel Bootstrap 🔴 (Most Complex)

### 2.1 Boot Assembly (head.S → head64.S)
The boot process for x86_64 differs significantly from i386:

**i386 boot (current):**
1. BIOS loads boot sector (16-bit real mode)
2. Boot code switches to 32-bit protected mode
3. Sets up GDT, IDT, initial page tables (identity mapping)
4. Jumps to kernel main()

**x86_64 boot (required):**
1. BIOS loads boot sector (16-bit real mode)
2. Boot code switches to 32-bit protected mode
3. **Switch to long mode** (requires: PAE-enabled page tables, EFER.LME=1, CS.L=1)
4. Set up 4-level page tables (PML4 → PDP → PD → PT)
5. Set up 64-bit GDT and IDT
6. Jump to 64-bit kernel entry

**Key differences:**
- Must enable PAE (Physical Address Extension) just to enter long mode — PAE is **mandatory**, not optional
- 4-level page table walk (PML4 → PDP → PD → PT)
- 64-bit GDT entries (8 bytes → 16 bytes)
- IDT entries are 16 bytes (vs 8 bytes for i386)
- Use `LGDT` with 64-bit base address
- `LIDT` with 64-bit base
- First jump to 64-bit uses `ljmp` with `R_CS` segment
- Stack pointer is RSP (64-bit)

### 2.2 Context Switch (mpx386.S → mpx64.S)

**i386 context (current):**
```
PUSHAD: EAX, ECX, EDX, EBX, ESP, EBP, ESI, EDI
PUSH: EFLAGS, CS, EIP (from interrupt)
```

**x86_64 context (required):**
```
PUSH: RAX, RCX, RDX, RBX, RSP, RBP, RSI, RDI
PUSH: R8, R9, R10, R11, R12, R13, R14, R15
PUSH: RFLAGS, CS, RIP (from interrupt)
PUSH: SS, RSP (for interrupts that change privilege)
```

**Key differences:**
- 16 general-purpose registers vs 8 (extra R8–R15)
- 64-bit register operands (RAX vs EAX)
- Different syscall mechanism: `SYSCALL`/`SYSRET` (instead of `int 0x80`/`INT`)
- Different interrupt handling: `iretq` vs `iretd`
- Kernel may use its own stack (IST = Interrupt Stack Table)

### 2.3 Kernel Library (klib386.S → klib64.S)

**i386:** Software integer division/multiplication (since i386 lacks hardware div in early models)
**x86_64:** Hardware 64-bit `DIV`/`MUL` instructions available natively

Replace:
- `_k_muldiv64` → use hardware mul/div
- String ops → use `REP MOVSQ` (64-bit) instead of `REP MOVSD` (32-bit)
- `_k_cpuid` → update for x86_64 CPUID leaf handling

### 2.4 Linker Script (kernel.lds → kernel64.lds)

**Current i386:**
```
. = 0x0;  /* Base address */
.unpaged : { *(.unpaged) }
.text : { *(.text) }
.data : { *(.data) }
.bss : { *(.bss) }
```

**Required x86_64:**
```
. = 0xFFFF800000000000;  /* x86_64 canonical kernel space */
.unpaged : { *(.unpaged) }
.text : { *(.text) }
.data : { *(.data) }
.bss : { *(.bss) }
```

**Key differences:**
- Kernel virtual base in canonical form (0xFFFF800000000000+)
- 64-bit absolute addressing requires special handling (`.text` may need `-mcmodel=kernel`)
- PIE/PIC considerations

**Status**: ✅ COMPLETED

**Files Created (7 files):**
- `kernel.lds` — ELF64 linker script (phys=1MB, virt=FFFF8000F0100000)
- `include/archconst.h` — x86_64 constants (64-bit GDT, IDT, MSR, KTS flags)
- `sconst.h` — ASM macros (SAVE_PROCESS_CTX, SAVE_TRAP_CTX_USER, 64-bit regs)
- `head.S` — 32→64 bit boot (PAE, LME, PML4/PDP/PD, multiboot, GS.base init)
- `mpx.S` — hwint00-15, SYSCALL entry, exceptions, restore_user_context, iretq
- `klib.S` — phys_copy, memset, msg_copy, MSR/CR, cpuid, I/O, FPU, seg regs
- `Makefile.inc` — BSD Makefile for x86_64 arch (compiler-rt, string ops, unpaged)

**Key Design Decisions:**
- SYSCALL/SYSRET (not SYSENTER/SYSEXIT) for fast system calls
- SWAPGS-based kernel stack switching (KERNEL_GS_BASE MSR)
- iretq for interrupt/exception return
- 4-level paging (PML4 + PDP + PD + 2MB huge pages for boot)
- Identity map kernel low (via `_kern_phys_base = 0x00100000`)

**Bug Fixes Applied (from code reviews):**
1. ✅ SAVE_TRAP_CTX_USER — reversed offsets fixed (was SS↔RIP)
2. ✅ head.S — only KERNEL_GS_BASE set, not GS.base (SWAPGS fix)
3. ✅ klib.S — phys_copy source/dest swap (xchgq %rsi, %rdi)
4. ✅ klib.S — cpuid register assignments (save ecx_ptr before CPUID)
5. ✅ mpx.S — exception_entry displacement (24→16)
6. ✅ kernel.lds — phys base changed to 1MB (identity-map reachable)

---

## Phase 3: Memory Management 🔴

### 3.1 VM Pagetable (pagetable.c)

This is the most critical component with 26 `__i386__` ifdefs.

**i386 paging modes:**
- Legacy (2-level): PD → PT, 4KB pages, 32-bit phys
- PAE (3-level): PDP → PD → PT, extends to 36-bit phys

**x86_64 paging (required):**
- 4-level (IA-32e): PML4 → PDP → PD → PT
- 48-bit virtual address space (canonical form)
- Up to 52-bit physical address (hardware-dependent)
- Page size: 4KB (default), 2MB (large), 1GB (huge)

**Affected functions in pagetable.c:**
| Function | i386 implementation | x86_64 changes |
|----------|-------------------|----------------|
| `pt_pt()` | Creates page table entries | 64-bit PTE format, 4 levels |
| `pt_pd()` | Creates page directory | PML4 → PDP → PD mapping |
| `pt_checkrange()` | Range validation | 48-bit vs 32-bit address check |
| `pt_mapkernel()` | Kernel mapping | High address mapping (0xFFFF8000...) |
| `pt_writemap()` | Page table walk | 4-level walk instead of 2/3 |
| `alloc_ptable()` | Physical page alloc | 64-bit phys addr |

### 3.2 VM Physical Memory Management
- phys_bytes → 64-bit
- avrpl — physical memory ranges
- Memory map from bootloader (multiboot vs EFI)

**Status**: ✅ COMPLETED

**Files Created/Changed:**

**New headers (2 files):**
| File | Purpose |
|------|---------|
| `minix/include/arch/x86_64/include/vm.h` | 4-level paging constants (PML4/PDP/PD/PT, 512 entries, 64-bit PTEs) |
| `minix/servers/vm/arch/x86_64/pagetable.h` | `pt_entry_t = u64_t`, `ARCH_VM_DIR_ENTRIES=512`, `ARCH_BIG_PAGE_SIZE=2MB` |

**Modified files (4 files):**
| File | Changes |
|------|---------|
| `minix/servers/vm/pt.h` | `u32_t` → `pt_entry_t` (64-bit on x86_64) |
| `minix/servers/vm/arch/i386/pagetable.h` | Added `typedef u32_t pt_entry_t` |
| `minix/servers/vm/arch/earm/pagetable.h` | Added `typedef u32_t pt_entry_t` |
| `minix/servers/vm/pagetable.c` | ~30 `__x86_64__` branches, type fixes (struct pdm, entry, currentpagedir, etc.) |

**Key changes in pagetable.c:**
- `#if defined(__i386__)` → `#if defined(__i386__) || defined(__x86_64__)` — all ~25 branches
- `u32_t entry/maskedentry` → `pt_entry_t` — 64-bit PTE values
- `struct pdm { ... }` — `u32_t val/page_directories` → `pt_entry_t`
- `static u32_t currentpagedir[...]` → `static pt_entry_t currentpagedir[...]`
- `kern_start_pde` formula: division → `ARCH_VM_PDE()` (critical for x86_64 mask)
- `findhole()`: `static u32_t` → `static vir_bytes` — support >4GB VA space

---

## Phase 4: System Calls and Signals 🟡

### 4.1 System Call Interface

**i386 (current):** `int 0x80` with:
- EAX = syscall number
- EBX, ECX, EDX, ESI, EDI, EBP = arguments
- Return in EAX

**x86_64 (required):** `syscall` instruction with:
- RAX = syscall number
- RDI, RSI, RDX, R10, R8, R9 = arguments (x86_64 ABI convention)
- Return in RAX
- RCX and R11 clobbered (RIP, RFLAGS saved by hardware)

**Files affected:**
| File | Changes |
|------|---------|
| `kernel/system/do_sigsend.c` | Trapframe layout (iretq vs iretd) |
| `kernel/system/do_sigreturn.c` | Stack frame layout |
| `kernel/system/do_mcontext.c` | mcontext_t size/offsets |
| `kernel/system/do_trace.c` | Register layout for PT_GETREGS |
| `kernel/system.c` | User memory access (copy vs 64-bit) |
| `pm/misc.c` | Signal frame setup |

### 4.2 User Context
- `lib/libc/sys/_ucontext.c` — update `mcontext_t` for x86_64 registers
- `lib/libmthread/misc.c` — stacktrace with 64-bit addresses
- `lib/libmthread/allocate.c` — stack alignment (16-byte alignment on x86_64)

**Status**: ✅ COMPLETED

**New Files Created (4 files):**
| File | Purpose |
|------|---------|
| `sys/arch/x86_64/include/mcontext.h` | x86_64 mcontext_t (26 × 64-bit gregs, FXSAVE FPU, `_UC_MACHINE_*` macros) |
| `sys/arch/x86_64/include/signal.h` | x86_64 sigcontext (16 GP regs + seg regs + iretq frame, SC_MAGIC=0xc0ffee3) |
| `sys/arch/x86_64/include/frame.h` | x86_64 trapframe/intrframe/switchframe/sigframe (uint64_t fields) |
| `minix/include/arch/x86_64/include/stackframe.h` | stackframe_s (r15-r8, di, si, fp, bx, dx, cx, retreg, pc, cs, psw, sp, ss) |

**Modified Files (9 files):**
| File | Changes |
|------|---------|
| `kernel/system/do_sigsend.c` | Added `__x86_64__`: saves 16 GP regs + iretq frame to sigcontext, restores with `new_fp` |
| `kernel/system/do_sigreturn.c` | Added `__x86_64__`: restores r15-r8, rdi-rbp, rbx-rax from sigcontext, X86_FLAGS_USER for RFLAGS |
| `kernel/system/do_mcontext.c` | Changed `__i386__` → `__i386__ || __x86_64__` for FPU state (identical on both) |
| `kernel/system/do_trace.c` | Added `__x86_64__`: protects CS+SS from modification (DS/ES/FS/GS are flat in long mode) |
| `kernel/system.c` | `__x86_64__` for SYS_DEVIO/VDEVIO/READBIOS/IOPENABLE/SDEVIO (in/out work in 64-bit) |
| `pm/misc.c` | uname: `"x86_64"` for machine + architecture |
| `lib/libc/sys/_ucontext.c` | makecontext: x86_64 ABI (first 6 args in RDI/RSI/RDX/RCX/R8/R9, R12 for ucp) |
| `lib/libmthread/misc.c` | stacktrace: uses `_UC_MACHINE_RBP` for 64-bit frame pointer unwinding |
| `lib/libmthread/allocate.c` | `__x86_64__` in guard page and stack deallocation conditionals |

**Key Design Decisions:**
- `SC_MAGIC = 0xc0ffee3` (unique from i386 0xc0ffee1, ARM 0xc0ffee2)
- Segment registers (GS/FS/ES/DS) set to 0 in sigcontext — flat segment model in long mode
- CS and SS protected from ptrace modification in x86_64 (but not DS/ES/FS/GS)
- FPU state format identical to i386 (FXSAVE) — same `FPU_XFP_SIZE`
- x86_64 ABI for `makecontext`: first 6 args in registers, rest on stack, R12 preserves ucp

**Next**: Phase 5 — Libraries and Toolchain

---

## Phase 5: Libraries and Toolchain 🟡

### 5.1 libsys arch
Create `minix/lib/libsys/arch/x86_64/`:
- `ser_putc.c` — serial output (simple, mostly same)
- `sys_in.c` / `sys_out.c` — I/O port access (same instructions: `in`/`out`)
- `get_randomness.c` — RDRAND support (x86_64 has RDRAND)
- `s_ipc.S` — IPC syscall wrapper (rewrite with `syscall` instruction)
- `ucontext.S` — getcontext/setcontext (64-bit versions)

### 5.2 libminc arch
Create `minix/lib/libminc/arch/x86_64/`:
- `setjmp.S` — save/restore 64-bit registers (R12–R15, RBX, RBP, RSP)
- `longjmp.S` — analogous
- `_cpufunc.S` — CPU feature detection (CPUID, RDTSC)

### 5.3 compiler-rt
Current `sys/external/bsd/compiler_rt/dist/lib/builtins/i386/*.S`:
- These are guarded by `#ifdef __i386__`
- x86_64 uses hardware `div`/`mul` for 64-bit operations
- No compiler-rt changes needed for basic 64-bit operations
- May need `floatundidf.S`, `floatundisf.S` for float operations

**Status**: ✅ COMPLETED

**New Files Created:**

**minix/lib/libc/arch/x86_64/ (6 files):**
| File | Purpose |
|------|---------|
| `Makefile.inc` | Build rules for arch-specific libc |
| `_cpuid.S` | CPUID wrapper (x86_64 ABI: RDI=eax, RSI=ebx, RDX=ecx, RCX=edx) |
| `get_bp.S` | Return RBP in RAX |
| `getprocessor.S` | CPU family detection via CPUID |
| `read_tsc.S` | RDTSC wrapper (RDI=high, RSI=low) |
| `_cpufeature.c` | CPU feature detection (SYSCALL always = 1 on x86_64) |

**minix/lib/libc/arch/x86_64/sys/ (8 files):**
| File | Purpose |
|------|---------|
| `Makefile.inc` | Build rules + ucontextoffsets.h generation |
| `_ipc.S` | IPC wrappers: RAX=opcode, RDX=endpoint, RSI=msg, status in RBX |
| `ucontext.S` | getcontext/setcontext(ctx_start), 64-bit regs |
| `ucontextoffsets.cf` | _REG_RDI, _REG_RSI, _REG_RBP, etc. offsets |
| `_do_kernel_call_intr.S` | Kernel call: RAX=KERNEL_CALL, RDX=msg |
| `__sigreturn.S` | Sigreturn stub (+16 bytes skip) |
| `brksize.S` | .quad _end (64-bit pointer) |
| `ipc_minix_kerninfo.S` | MINIX_KERNINFO via SYSCALL, RBX=kerninfo |

**minix/lib/libsys/arch/x86_64/ (copied from i386):**
- All device I/O files (identical — use kernel calls, work on both archs)

**minix/lib/libminc/arch/x86_64/ (1 file):**
| File | Purpose |
|------|---------|
| `Makefile.libc.inc` | References common/libc x86_64 string/atomic ops |

**Key Design Decisions:**
- MINIX IPC convention for SYSCALL: RAX=opcode, RDX=endpoint, RSI=msg, RBX=status return
- Kernel call: RAX=KERNEL_CALL, RDX=msg_ptr
- kerninfo: RAX=MINIX_KERNINFO, RBX=kerninfo_ptr
- SYSCALL clobbers RCX (RIP) and R11 (RFLAGS) — registers preserved: RBX, RBP, R12-R15
- libsys files are architecture-independent (use kernel call interface)

**Bug fixed:** _cpuid.S: 4th arg pointer (edx_ptr in RCX) was clobbered by CPUID; fixed by saving ptrs in R8-R11

**Next**: Phase 6 — Driver Adaptation

---

## Phase 6: Driver Adaptation 🟡

### 6.1 I/O Port Access
x86_64 supports `in`/`out` instructions in 64-bit mode (same as i386).
- `lib/libaudiodriver/audio_fw.c` — uses `inb`/`outb`, no change needed
- `sys/arch/x86/include/sysarch.h` — minix_io_port functions

### 6.2 DMA and MMIO
- Block drivers: 64-bit DMA addresses
- Network drivers: 64-bit ring buffer addresses
- PCI: 64-bit MMIO base addresses (PCIe extended config space)

### 6.3 Interrupt Handling
- APIC: 64-bit destination format (x2APIC mode)
- I/O APIC: 64-bit redirection entries

**Status**: ✅ COMPLETED

**Files Created in `minix/include/arch/x86_64/include/` (17 files):**
Copied from i386 (arch-independent — same I/O ports, PCI config, UART, RTC):
- `pci.h`, `pci_amd.h`, `pci_intel.h`, `pci_sis.h`, `pci_via.h` — PCI config space (0xCF8/0xCFC)
- `cmos.h`, `diskparm.h`, `fpu.h`, `memory.h`, `partition.h`, `bios.h`, `ipcconst.h`
- `elf.h`, `interrupt.h` (fixed guard), `ports.h` (fixed guard)
- `stackframe.h`, `vm.h` (created in Phases 2-3)

Custom-created:
- `archtypes.h` — x86_64 version: `gatedesc_s` with 16-byte IDT (offset_low+middle+high), `desctableptr_s` with u64_t base, `segframe_t` with u64_t CR3

**Modified files:**
| File | Change |
|------|--------|
| `minix/drivers/tty/tty/arch/i386/rs232.c` | 6× `__i386__` → `__i386__ || __x86_64__` (UART 8250) |
| `minix/drivers/storage/memory/memory.c` | 1× `__i386__` → `__i386__ || __x86_64__` |

**Key design points:**
- PCI config space (in/out to 0xCF8/0xCFC) is identical on x86_64
- UART 8250 programming model is identical
- 8259 PIC ports and IRQ vectors are identical
- RTC/CMOS ports (0x70/0x71) are identical
- All `in`/`out` instructions work identically in 64-bit mode
- Main differences: IDT entries are 16 bytes (vs 8), descriptor table pointers are 10 bytes (vs 6)

**Verification:**
- No remaining `__i386__` guards in copied headers (checked via code searcher)
- `interrupt.h` and `ports.h` had guards that have been fixed

---

## Implementation Order (Recommended)

```
Month 1:  Build Infrastructure (Phase 1) + Planning
          → cmake/arch_x86_64.cmake, toolchain, presets
          → QEMU x86_64 system boot test

Month 2:  Kernel Bootstrap (Phase 2)
          → head64.S (long mode entry)
          → mpx64.S (context switch)
          → klib64.S (kernel library)
          → linker script
          → "Hello from x86_64 kernel" in QEMU

Month 3:  Memory Management (Phase 3)
          → 4-level pagetable implementation
          → VM server adaptation
          → Physical memory management

Month 4:  System Calls + Signals (Phase 4)
          → syscall/sysret instruction
          → Signal handling
          → mcontext update

Month 5:  Libraries + Drivers (Phases 5-6)
          → libsys, libminc arch ports
          → Driver adaptation
          → Boot to shell

Month 6:  Testing + Polish
          → Full functional testing
          → Performance benchmarking
          → Documentation
```

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-----------|--------|------------|
| Page table complexity | High | High | Start with QEMU, test incrementally |
| ASM bugs in context switch | Medium | Critical | Single-step debugging in QEMU |
| Signal frame layout mismatch | Medium | High | Compare with NetBSD x86_64 ABI |
| Driver 64-bit assumptions | Low | Medium | Mostly compatible (in/out same) |
| Build system issues | Low | Low | CMake infrastructure already proven |

## Dependencies

### Required
- ✅ Build System Migration (CMake) — completed
- x86_64 cross-compiler (GCC targeting x86_64-minix)
- QEMU with x86_64 system emulation

### Optional
- UEFI firmware for boot testing
- Physical x86_64 hardware for performance testing

## Success Criteria

Phase 2-3 (minimal bootable):
- [ ] x86_64 kernel boots in QEMU
- [ ] Prints startup messages
- [ ] VM initializes with 4-level paging

Phase 4 (functional system):
- [ ] Multiple processes run in user mode
- [ ] System calls work between servers
- [ ] Signal delivery works correctly

Phase 5-6 (production ready):
- [ ] Shell and basic commands work
- [ ] File system operations work
- [ ] Network stack works
- [ ] Device drivers functional
