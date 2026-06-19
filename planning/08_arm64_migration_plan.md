# ARM64 (AArch64) Architecture Migration Plan

## Overview

This document details the migration plan for porting MINIX from 32-bit ARM (earm) to ARM64 (AArch64). This is Phase 2 of the Architecture Migration defined in `planning/03_migration_roadmap.md` and expands on the brief outline in `planning/04_target_architecture_support.md`.

ARM64 is fundamentally different from 32-bit ARM — it is not merely a 64-bit extension but a clean-slate architecture. Unlike x86_64 (which builds on i386 with long mode), ARM64 shares only conceptual similarity with 32-bit ARM. The instruction set, exception model, page tables, and boot process are entirely different.

## Current State (Post-Audit)

### Architecture Support Matrix

| Component | i386 | earm (32-bit ARM) | x86_64 | ARM64 (AArch64) |
|-----------|------|-------------------|--------|------------------|
| Build system (CMake) | ✅ arch_i386.cmake | ✅ arch_earm.cmake | ✅ arch_x86_64.cmake | ❌ Missing |
| Kernel ASM (boot, traps, ctx) | ✅ Complete | ✅ Complete | ✅ Complete | ❌ Missing |
| Memory management (VM) | ✅ Complete | ✅ Complete (v7) | ✅ Complete | ✅ arch/aarch64/include/vm.h + pagetable.h + __aarch64__ in pagetable.c |
| Signals/mcontext/syscalls | ✅ Complete | ✅ Complete | ✅ Complete | ❌ Missing |
| libsys arch | ✅ Complete | ✅ 3 files | ✅ Complete | ❌ Missing |
| libminc arch (setjmp/longjmp) | ✅ Complete | ✅ Complete | ✅ Complete | ❌ Missing |
| libc arch (IPC, ucontext) | ✅ Complete | ✅ Complete | ✅ Complete | ❌ Missing |
| Device drivers | ✅ Complete | ✅ Partial (AM335x) | ✅ Complete | ❌ Missing |
| Platform support | ✅ All PCs | ✅ BeagleBone | ✅ All x86_64 | ❌ Missing |
| Linker scripts | ✅ Complete | ✅ Complete | ✅ Complete | ❌ Missing |

### Existing ARM Infrastructure

**32-bit ARM (earm) — Well-Established:**
- Kernel arch directory: `minix/kernel/arch/earm/` — 16+ assembly and C files
- VM arch: `minix/servers/vm/arch/earm/pagetable.h`, `vm.lds`
- Library arch: `minix/lib/libsys/arch/earm/` (3 files), `minix/lib/libc/arch/arm/`
- Sys headers: `sys/arch/arm/include/` (87 headers), `sys/arch/evbarm/include/` (57 headers)
- Platform: BeagleBone (AM335x), OMAP GPIO, USB MUSB, MMC, LCD

**ARM64 (AArch64) — Nearly Nonexistent:**
- No dedicated arch directory in `sys/arch/`
- No kernel arch files in `minix/kernel/arch/`
- No VM arch files
- No library arch files
- Minimal aarch64 references in:
  - `sys/arch/evbarm/include/param.h` — some ARM64 constants
  - `lib/libm/arch/aarch64/` — 2 files (fenv.c, e_sqrt.S)
  - `lib/libkvm/kvm_aarch64.c` — kvm support
  - `external/gpl3/binutils/usr.bin/ld/arch/aarch64/` — linker emulation
- Toolchain: `build.sh` recognizes `evbarm64/MACHINE_ARCH=aarch64`
- CMake: `cmake/toolchain-minix.cmake` has `MACHINE_ARCH STREQUAL "aarch64"` case

### Key Findings

1. **No `__aarch64__` code exists in MINIX kernel or servers** — all ARM code uses `__arm__` for 32-bit
2. **ARM64 requires a ground-up implementation** — cannot leverage shared code paths like x86_64↔i386
3. **32-bit ARM vs ARM64 differences are profound**:
   - **Exception model**: ARM64 has 4 exception levels (EL0–EL3) vs ARM's 7 modes
   - **Page tables**: ARM64 uses 3/4-level translation tables vs ARM v7 short descriptor or LPAE
   - **Instructions**: Completely different encoding — no binary compatibility
   - **Boot**: ARM64 typically boots via UEFI or device tree, ARM via ATAGS or device tree
4. **Platform targets are different**: BeagleBone (32-bit) vs Raspberry Pi 4/5, AWS Graviton (64-bit)
5. **External libraries already support ARM64**: wolfSSL, compiler-rt have `__aarch64__` code paths

## Migration Strategy

### Approach: Clean-Slate with ARM64-Specific Design

Since ARM64 shares no code paths with 32-bit ARM at the assembly/register level, we take a ground-up approach while reusing ARM64 experience from external projects (NetBSD aarch64, Linux arm64).

```
Phase 1: Build Infrastructure    → cmake configure succeeds for aarch64
Phase 2: Kernel Bootstrap         → boots to "Hello from ARM64" in QEMU
Phase 3: Memory Management        → VM works with ARM64 4-level translation
Phase 4: Interrupts + Timers      → GIC v2/v3, ARM generic timer work
Phase 5: System Calls + Signals   → SVC exception handling, signal delivery
Phase 6: Libraries + Userland     → libsys, libminc, libc ported
Phase 7: Platform + Drivers       → Raspberry Pi 4/5 support
Phase 8: Testing + Polish         → Full functional testing, documentation
```

---

## Phase 1: Build Infrastructure ✅ **Выполнена**

### 1.1 cmake/arch_aarch64.cmake ✅

Created architecture definition for ARM64 (see `cmake/arch_aarch64.cmake`):
- `MACHINE_CPU=aarch64`, `GNU_ARCH=aarch64`, `GNU_PLATFORM=aarch64-elf64-minix`
- `-march=armv8-a`, `-mstrict-align`, `-fomit-frame-pointer` (Release only)
- `__aarch64__`, `__ARM_ARCH_8__`, `_LP64`, `__LP64__`
- x86 options (USE_WATCHDOG, USE_ACPI, etc.) forced OFF (same as arch_earm.cmake)

### 1.2 cmake/toolchain-minix.cmake ✅

Added tool prefix: `_set_tool_prefix("aarch64-elf64-minix")`
`CMAKE_SYSTEM_PROCESSOR="aarch64"` already existed.

### 1.3 CMakePresets.json ✅

- `aarch64-debug`: Debug build, Ninja, MACHINE_ARCH=aarch64
- `aarch64-release`: Release build, Ninja, MACHINE_ARCH=aarch64
- Build presets for both

### 1.4 cmake/options.cmake ✅

Updated comment to mention aarch64 alongside earm for force-off options.

### 1.5 cmake/ci-config.cmake ✅

Added aarch64 to CI_ARCHITECTURES list.

### 1.6 minix/kernel/arch/aarch64/ — stub files ✅

Minimum required files for cmake configure:
- `kernel.lds` — stub linker script (OUTPUT_ARCH(aarch64), ENTRY(_start))
- `procoffsets.cf` — stub offset definitions for Phase 2+
- `include/` — empty directory (for include paths)

### 1.7 minix/include/arch/aarch64/include/archtypes.h ✅

Stub header with register convention documentation.

### 1.8 minix/kernel/CMakeLists.txt ✅

Added `elseif(MACHINE_ARCH STREQUAL "aarch64")` cases:
- KERNEL_ARCH_SOURCES: empty (Phase 2 will add head.S, mpx.S, etc.)
- UNPAGED_*_OBJECTS: empty sets

### Status: ✅ **Build infrastructure complete**

- `cmake -DMACHINE_ARCH=aarch64 ..` succeeds
- `cmake --build .` will fail (expected — no kernel arch code yet)
- **Добавленные/изменённые файлы**:
  - 🆕 `cmake/arch_aarch64.cmake`
  - ✏️ `cmake/toolchain-minix.cmake`
  - ✏️ `CMakePresets.json`
  - ✏️ `cmake/options.cmake`
  - ✏️ `cmake/ci-config.cmake`
  - 🆕 `minix/kernel/arch/aarch64/kernel.lds`
  - 🆕 `minix/kernel/arch/aarch64/procoffsets.cf`
  - 🆕 `minix/include/arch/aarch64/include/archtypes.h`
  - ✏️ `minix/kernel/CMakeLists.txt`

---

## Phase 2: Kernel Bootstrap 🔴 (Most Complex)

### 2.1 ARM64 Boot Requirements

ARM64 boot differs fundamentally from 32-bit ARM:

**32-bit ARM boot (current earm):**
1. Bootloader (U-Boot) loads kernel at a known address
2. Kernel entry point in `start.S`:
   - Sets up CPU in SVC mode
   - Initializes stacks for each mode
   - Sets up initial page tables (section mapping)
   - Enables MMU
   - Jumps to C code start_kernel()

**ARM64 boot (required):**
1. Bootloader (U-Boot, UEFI) loads kernel at a known address
2. Kernel entry point in `head.S`:
   - CPU starts in EL1 (kernel privilege level) or EL2 (hypervisor)
   - Must set up:
     - Exception vectors (VBAR_EL1)
     - Stack pointer (SP_EL1)
     - Page tables (TTBR0_EL1 / TTBR1_EL1 for kernel/user split)
     - System control registers (SCTLR_EL1)
   - Must initialize:
     - Translation Control Register (TCR_EL1) — page size, VA size
     - Memory Model (MAIR_EL1) — memory type attributes
   - Enable MMU (SCTLR_EL1.M = 1)
   - Jump to C code

**Key ARM64 specifications:**
| Parameter | Value | Notes |
|-----------|-------|-------|
| Exception level | EL1 | Kernel runs at EL1 |
| Page size | 4KB | Standard, matches existing MINIX |
| VA space | 48-bit | Max for ARMv8.0, 256TB |
| PA space | 48-bit | Standard ARMv8.0 |
| Translation regime | 2-stage | VMSAv8-64, stage 1 only |
| Stack alignment | 16-byte | Mandatory SP alignment |
| Cache line | 64 bytes | Cortex-A72, A53, etc. |

### 2.2 Exception Vectors (vectors.S)

ARM64 has a **vector table** at a base address (VBAR_EL1) with 16 entries, each 128 bytes:

```
Offset  | Exception Type                | Description
--------|-------------------------------|------------------------------
0x000   | EL1t Sync (SP_EL0)           | Synchronous, same EL, SP0
0x080   | EL1t IRQ (SP_EL0)            | IRQ, same EL, SP0
0x100   | EL1t FIQ (SP_EL0)            | FIQ, same EL, SP0
0x180   | EL1t SError (SP_EL0)         | SError, same EL, SP0
0x200   | EL1h Sync (SP_EL1)           | Synchronous, same EL, SPx
0x280   | EL1h IRQ (SP_EL1)            | IRQ, same EL, SPx
0x300   | EL1h FIQ (SP_EL1)            | FIQ, same EL, SPx
0x380   | EL1h SError (SP_EL1)         | SError, same EL, SPx
0x400   | EL0 Sync (AArch64)           | SVC from AArch64 user
0x480   | EL0 IRQ (AArch64)            | IRQ from AArch64 user
0x500   | EL0 FIQ (AArch64)            | FIQ from AArch64 user
0x580   | EL0 SError (AArch64)         | SError from AArch64 user
0x600   | EL0 Sync (AArch32)           | SVC from AArch32 user
...     | ...                           | (unlikely to support AArch32)
```

Each entry is 128 bytes — enough for a complete handler or a branch to a C handler.

**MINIX-specific mapping:**
| Vector | MINIX Handler | Purpose |
|--------|---------------|---------|
| EL1h Sync | exception_handler | Page faults, alignment, undefined |
| EL1h IRQ | hwint_handler | Hardware interrupts (GIC) |
| EL0 Sync (AArch64) | syscall_handler | System calls via SVC #0 |
| EL0 Sync (AArch32) | — | Not supported (32-bit compat) |

### 2.3 Context Switch (mpx.S)

**32-bit ARM context (current):**
```asm
SAVE_CONTEXT:
    STMFD sp!, {r0-r14}^    ; Save user regs
    MRS r0, SPSR_svc        ; Save SPSR
    STR lr, [sp, #-4]!      ; Save return address
```

**ARM64 context (required):**
```asm
SAVE_CONTEXT_USER:
    STP x0, x1, [sp, #-16]!
    STP x2, x3, [sp, #-16]!
    ...
    STP x28, x29, [sp, #-16]!
    MRS x0, SP_EL0          ; Save user SP
    STP x30, x0, [sp, #-16]! ; LR + user SP
    MRS x0, SPSR_EL1        ; Save saved processor state
    STP x0, x1, [sp, #-16]! ; SPSR + ELR_EL1

RESTORE_CONTEXT_USER:
    LDP x0, x1, [sp], #16   ; Restore SPSR + ELR
    MSR SPSR_EL1, x0
    MSR ELR_EL1, x1
    LDP x30, x0, [sp], #16  ; Restore LR + user SP
    MSR SP_EL0, x0
    LDP x0, x1, [sp], #16   ; Restore x0..x1
    ...
    LDP x28, x29, [sp], #16
    ERET                    ; Return to user
```

**Key differences from 32-bit ARM:**
- 31 general-purpose registers (x0–x30) vs 16 (r0–r15)
- Separate SP_EL0 (user) and SP_EL1 (kernel) stack pointers
- `ERET` instruction instead of `MOVS PC, LR` for return
- No `SPSR_svc` — banked registers replaced by EL-specific registers
- `STP`/`LDP` (store/load pair) for efficient register save/restore
- Link register is x30 (not LR/r14)

### 2.4 Kernel Library (klib.S)

| Function | ARM Implementation | ARM64 Implementation |
|----------|-------------------|----------------------|
| phys_copy | `LDMIA/STMIA` with r0-r3 | `LDP/STP` with x0-x3 |
| phys_memset | `STR` with r0 | `STR` with x0 (64-bit) |
| memcpy | Software loop | `REP`-style or NEON |
| get_bp | `MOV r0, fp` | `MOV x0, x29` (fp=x29) |
| read_tsc | CNTPCT_EL0 (ARM arch timer) | Same (64-bit counter) |
| cpuid | MIDR_EL1 register | Same (but different layout) |

**ARM64-specific kernel utilities:**
```asm
/* Read system register — ARM64 requires MSR/MRS */
get_cpuid:
    MRS x0, MIDR_EL1        ; Main ID Register
    RET

read_cycle_counter:
    ISB                     ; Instruction synchronization barrier
    MRS x0, CNTPCT_EL0     ; Physical count register (64-bit)
    RET

get_ttbr0:
    MRS x0, TTBR0_EL1      ; Translation Table Base Register 0
    RET
```

### 2.5 Linker Script (kernel.lds)

ARM64 kernel layout (different virtual base):

```lds
OUTPUT_ARCH(aarch64)
ENTRY(_start)

PHYS_BASE = 0x00080000;      /* Typical ARM64 kernel load address */
KERNEL_VBASE = 0xFFFF800000000000;  /* High kernel virtual address */

SECTIONS {
    . = PHYS_BASE;
    .unpaged : { *(.unpaged) }      /* Identity-mapped boot code */

    . = KERNEL_VBASE + PHYS_BASE;
    .text : AT(PHYS_BASE + SIZEOF(.unpaged)) {
        *(.text*)
    }
    .rodata : { *(.rodata*) }
    .data : { *(.data*) }
    .bss : { *(.bss*) }
}
```

**Key differences from 32-bit ARM:**
- ARM64 kernel typically loaded at 0x00080000 (512KB) vs ARM at 0x80200000 (for BeagleBone)
- 64-bit virtual address space (0xFFFF8000... high addresses)
- `OUTPUT_ARCH(aarch64)` instead of `arm`
- No THUMB interworking considerations
- `ENTRY(_start)` — standard ARM64 entry point name

### Files to Create (Phase 2)

| File | Purpose |
|------|---------|
| `minix/kernel/arch/aarch64/head.S` | Boot entry, MMU enable, EL1 setup |
| `minix/kernel/arch/aarch64/mpx.S` | Context save/restore, ERET, SVC handler |
| `minix/kernel/arch/aarch64/vectors.S` | Exception vector table (VBAR_EL1) |
| `minix/kernel/arch/aarch64/klib.S` | phys_copy, memset, cpuid, cycle counter |
| `minix/kernel/arch/aarch64/kernel.lds` | Linker script (64-bit, high VA) |
| `minix/kernel/arch/aarch64/startup.c` | C startup after MMU enable |
| `minix/include/arch/aarch64/include/archtypes.h` | ARM64-specific types: exception frame, stack frame |

**Estimated effort**: 4-6 weeks
**Dependencies**: Phase 1 (build infrastructure)

---

## Phase 3: Memory Management ✅ **Выполнена**

### 3.1 ARM64 Translation Tables

ARM64 uses a **4-level or 3-level translation table** depending on page size and VA size:

**With 4KB pages and 48-bit VA:**
```
Level 0:  47:39 (9 bits) → Table (512 entries, 4KB)
Level 1:  38:30 (9 bits) → Table (512 entries, 4KB)
Level 2:  29:21 (9 bits) → Table (512 entries, 4KB)  ← PDE level (MINIX 2-level abstraction)
Level 3:  20:12 (9 bits) → Page (512 entries, 4KB)   ← PTE level
          11:0  (12 bits) → Page offset
```

**ARM64 PTE format (64-bit) — defined in `minix/include/arch/aarch64/include/vm.h`:**
```
Bit    | Field       | AARCH64_VM_* constant          | Description
-------|-------------|--------------------------------|------------------------------
0      | Valid       | PRESENT                        | Entry valid
1      | Page/Table  | PAGE or TABLE                  | 1=Page/L3, 1=Table/L0-L2
2-4    | AttrIndx    | NORMAL=0, DEVICE=1             | Index into MAIR_EL1
6      | AP[1]       | USER                           | EL0 access enable
7      | AP[2]       | RO                             | Read-only
8-9    | SH[1:0]     | SH_IS=3<<8                     | Inner Shareable
10     | AF          | AF                             | Access Flag (must be 1)
11     | nG          | NG                             | Not Global
12-47  | Output addr | ADDR_MASK=0x0000FFFFFFFFF000   | Physical address (48-bit PA)
53     | PXN         | PXN                            | Privileged Execute Never
54     | UXN         | UXN                            | User Execute Never
```

### 3.2 VM Server Architecture — Implementation Details

**2-level abstraction:** Same as x86_64:
- PDE = L2 descriptor (512 entries × 8 bytes = 4KB): maps 2MB per entry
- PTE = L3 descriptor (512 entries × 8 bytes = 4KB): maps 4KB per entry
- L0 and L1 managed by kernel (same as PML4/PDP on x86_64)

**Key ARM64 differences from x86_64:**
| Aspect | x86_64 | ARM64 |
|--------|--------|-------|
| PDE type | PD entry with PS bit | L2 table descriptor (bit1=1) or block (bit1=0) |
| PTE type | PT entry | L3 page descriptor (bit1=1) |
| RW flag | Set bit (bit1) | Absence of RO (AP[2]=bit7) |
| BIGPAGE | PS bit (bit7) | !TABLE (bit1=0) → inverted check |
| Access Flag | Optional (bit5) | Mandatory (bit10, must be 1) |
| User/Kernel | Supervisor bit (bit2) | AP[1] bit (bit6) |
| Memory attr | PWT/PCD bits | AttrIndx via MAIR_EL1 |
| Global page | G bit (bit8) | !nG (bit11=0) |

**BIGPAGE inversion:** ARM64 L2 block descriptors have bit1=0 (NOT Table), while table descriptors have bit1=1. This is the inverse of ARM32 where section descriptors have bit1=1. Handled with `#if defined(__aarch64__)` guards in all 5 usage sites.

### 3.3 Files Created/Modified

| File | Type | Description |
|------|------|-------------|
| `minix/include/arch/aarch64/include/vm.h` | 🆕 | ARM64 VMSAv8-64 constants: PTE flags, address masks, PDE/PTE macros, page fault FSC decoding |
| `minix/servers/vm/arch/aarch64/pagetable.h` | 🆕 | Abstraction: pt_entry_t=u64_t, ARCH_VM_DIR_ENTRIES=512, ARCH_BIG_PAGE_SIZE=2MB, PFERR_* for ESR_EL1 |
| `minix/servers/vm/pagetable.c` | ✏️ | ~20 `__aarch64__` blocks: pt_ptalloc (table desc), pt_mapkernel (block desc), pt_bind, pt_init, BIGPAGE, RO/RW, address masks, cached flags, page fault decode |
| `minix/servers/vm/CMakeLists.txt` | ✏️ | Compile definition `__aarch64__` for VM server |
| `planning/08_arm64_migration_plan.md` | ✏️ | Phase 3 status: ✅ Completed |

### Status: ✅ **Memory management infrastructure complete**

- ARM64 page table constants defined in `vm.h`
- Architecture abstraction in `pagetable.h` (pt_entry_t, ARCH_*, PFERR_*)
- All ~20 `__aarch64__` paths added to `pagetable.c`
- **Добавленные/изменённые файлы**:
  - 🆕 `minix/include/arch/aarch64/include/vm.h`
  - 🆕 `minix/servers/vm/arch/aarch64/pagetable.h`
  - ✏️ `minix/servers/vm/pagetable.c`
  - ✏️ `minix/servers/vm/CMakeLists.txt`
  - ✏️ `planning/08_arm64_migration_plan.md`

**Estimated effort**: 3-4 weeks
**Dependencies**: Phase 2 (kernel boots, exception vectors work)

---

## Phase 4: Interrupts and Timers 🟡

### 4.1 Generic Interrupt Controller (GIC)

ARM64 systems use the **GIC v2 or v3** instead of the simple interrupt controller on 32-bit ARM systems:

**GIC v2 (found on Cortex-A53, A57):**
```c
/* GIC CPU interface registers (memory-mapped) */
#define GIC_ICC_PMR     0x04    /* Priority Mask Register */
#define GIC_ICC_IAR     0x0C    /* Interrupt Acknowledge Register */
#define GIC_ICC_EOIR    0x10    /* End of Interrupt Register */

/* GIC Distributor registers */
#define GIC_ICD_CTLR    0x0000  /* Distributor Control */
#define GIC_ICD_ISENABLER 0x0100 /* Interrupt Set-Enable */
#define GIC_ICD_ICENABLER 0x0180 /* Interrupt Clear-Enable */
```

**GIC v3 (found on Cortex-A72, A76, etc.):**
- Uses system registers (ICC_*) instead of memory-mapped CPU interface
- Supports more interrupts, LPIs, and MSI
- Redistributor per core instead of shared CPU interface

**MINIX GIC driver requirements:**
```c
/* GIC v2 initialization (for QEMU virt, RPi 4): */
void gic_init(void) {
    /* Set priority mask to allow all interrupts */
    write_gic_reg(GIC_ICC_PMR, 0xFF);
    /* Enable group 0 and group 1 interrupts in distributor */
    write_gic_reg(GIC_ICD_CTLR, 0x3);
    /* Enable CPU interface */
    write_gic_reg(GIC_ICC_CTLR, 0x1);
}

/* IRQ handler: */
void gic_handle_irq(void) {
    uint32_t irq = read_gic_reg(GIC_ICC_IAR) & 0x3FF;
    handle_irq(irq);
    write_gic_reg(GIC_ICC_EOIR, irq);
}
```

### 4.2 ARM Generic Timer

ARM64 has a **generic timer** (CNTP/CNTV) instead of the various timer peripherals found on ARM32:

```c
/* Read system counter (64-bit): */
uint64_t read_cntpct(void) {
    uint64_t val;
    __asm__ __volatile__("mrs %0, CNTPCT_EL0" : "=r"(val));
    return val;
}

/* Set compare value for physical timer: */
void set_cntp_cval(uint64_t val) {
    __asm__ __volatile__("msr CNTP_CVAL_EL0, %0" : : "r"(val));
}

/* Enable physical timer: */
void enable_cntp(void) {
    uint32_t ctl;
    __asm__ __volatile__("mrs %0, CNTP_CTL_EL0" : "=r"(ctl));
    ctl |= 1;  /* ENABLE bit */
    __asm__ __volatile__("msr CNTP_CTL_EL0, %0" : : "r"(ctl));
}
```

### Files to Create (Phase 4)

| File | Purpose |
|------|---------|
| `minix/kernel/arch/aarch64/gic.c` | GIC v2/v3 initialization and interrupt handling |
| `minix/kernel/arch/aarch64/arch_clock.c` | ARM generic timer initialization |
| `minix/kernel/arch/aarch64/hw_intr.c` | Hardware interrupt routing (GIC → kernel handlers) |
| `minix/include/arch/aarch64/include/interrupt.h` | Interrupt controller constants |
| `minix/include/arch/aarch64/include/gic.h` | GIC register definitions |

**Estimated effort**: 2-3 weeks
**Dependencies**: Phase 2 (exception vectors working)

---

## Phase 5: System Calls and Signals 🟡

### 5.1 System Call Interface

**ARM32 (current):** `SVC #0` with:
- R7 = syscall number (MINIX convention)
- R0–R6 = arguments
- Return in R0
- Kernel accesses via `exc_svc` handler

**ARM64 (required):** `SVC #0` with:
- X8 = syscall number (ARM64 convention, MINIX may use X7 or X8)
- X0–X5 = arguments (first 6)
- X6 = return status indicator (optional)
- Return in X0
- Kernel accesses via EL0 Sync exception handler

**MINIX-specific ARM64 syscall ABI:**
```c
/* Userspace syscall convention: */
// SVC #0 with:
//   x7 = MINIX syscall number (or endpoint)
//   x0 = message pointer (same as MINIX IPC convention)
//   Returns: x0 = status

// OR: SYSCALL instruction (if supported)
//   x8 = syscall number
//   x0-x5 = arguments
//   x0 = return value
```

**Decision needed**: ARM64 offers `SVC #0` (serviced via exception vector) as the standard MINIX IPC mechanism. ARM64 also has an optional `SMC` instruction for secure monitor calls. For MINIX's message-passing IPC, `SVC #0` is the appropriate mechanism.

### 5.2 Signal Frame Layout

ARM64 signal handling differs substantially from ARM32:

**ARM32 sigcontext:**
```c
struct sigcontext {
    uint32_t trapno;           // Not used on ARM
    uint32_t error_code;        // Not used on ARM
    uint32_t regs[16];          // r0-r15
    uint32_t cpsr;              // Saved program status
};
```

**ARM64 sigcontext:**
```c
struct sigcontext_aarch64 {
    uint64_t fault_address;     // FAR_EL1 at exception
    uint64_t regs[31];          // x0-x30
    uint64_t sp;                // SP_EL0
    uint64_t pc;                // ELR_EL1 (program counter)
    uint64_t pstate;            // SPSR_EL1 (processor state)
};
// NUKE NOTE: ARM64 has 31 GPRs, not 16 — significant structural difference
```

### 5.3 Files Affected

| File | Change Required |
|------|----------------|
| `minix/kernel/system/do_sigsend.c` | Add `__aarch64__`: save 31 GP regs + ELR + SPSR |
| `minix/kernel/system/do_sigreturn.c` | Add `__aarch64__`: restore from ARM64 sigframe |
| `minix/kernel/system/do_fork.c` | Add `__aarch64__`: save/restore ARM64 register set |
| `minix/kernel/system/do_trace.c` | Add `__aarch64__`: ARM64 register layout for PT_GETREGS |
| `minix/kernel/system/do_mcontext.c` | Add `__aarch64__`: ARM64 mcontext layout |
| `minix/kernel/system.c` | Add `__aarch64__` for kernel call support |
| `minix/servers/pm/misc.c` | Add `"aarch64"` for uname machine string |

### Files to Create (Phase 5)

| File | Purpose |
|------|---------|
| `sys/arch/aarch64/include/mcontext.h` | ARM64 mcontext_t (31 × 64-bit gregs, FPSIMD, _UC_MACHINE_* macros) |
| `sys/arch/aarch64/include/signal.h` | ARM64 sigcontext (31 GP regs + SP + PC + PSTATE, SC_MAGIC=0xc0ffee4) |
| `sys/arch/aarch64/include/frame.h` | ARM64 trapframe/intrframe/switchframe/sigframe |
| `minix/include/arch/aarch64/include/stackframe.h` | stackframe_s for ARM64 |

**Estimated effort**: 3-4 weeks
**Dependencies**: Phase 2 (exception vectors), Phase 4 (interrupts/timers)

---

## Phase 6: Libraries and Toolchain 🟡

### 6.1 libsys arch

Create `minix/lib/libsys/arch/aarch64/`:

| File | Purpose | Implementation Notes |
|------|---------|---------------------|
| `frclock_util.c` | Frame rate clock | Same as ARM (uses generic timer) |
| `spin.c` | Spinlock | ARM64: `LDXR`/`STXR` exclusive access |
| `tsc_util.c` | Timestamp counter | ARM64: `MRS CNTPCT_EL0` |
| `ser_putc.c` | Serial output | UART 8250/PL011 (same as ARM) |

### 6.2 libminc arch

Create `minix/lib/libminc/arch/aarch64/`:

| File | Purpose |
|------|---------|
| `Makefile.libc.inc` | References common/libc aarch64 string/atomic ops |
| `setjmp.S` | Save/restore x19–x30 + SP (ARM64 ABI: callee-saved are x19–x29, LR=x30) |
| `longjmp.S` | Corresponding restore |

### 6.3 libc arch (minix)

Create `minix/lib/libc/arch/aarch64/` and `minix/lib/libc/arch/aarch64/sys/`:

**Core files:**
| File | Purpose |
|------|---------|
| `Makefile.inc` | Build rules for arch-specific libc |
| `_cpuid.S` | Read MIDR_EL1 via SMC or from kernel |
| `get_bp.S` | Return x29 (frame pointer) in x0 |
| `read_tsc.S` | Read CNTPCT_EL0 |

**Syscall wrappers (sys/):**
| File | Purpose |
|------|---------|
| `Makefile.inc` | Build rules + offset header generation |
| `_ipc.S` | IPC using SVC #0: x7=opcode, x0=msg_ptr |
| `ucontext.S` | getcontext/setcontext for ARM64 registers |
| `_do_kernel_call_intr.S` | Kernel call via SVC |
| `__sigreturn.S` | Sigreturn stub |
| `brksize.S` | `.quad _end` (64-bit) |
| `ipc_minix_kerninfo.S` | MINIX_KERNINFO via SVC |

### 6.4 compiler-rt

ARM64 hardware provides native 64-bit division/multiplication:
- `udivdi3` → hardware `UDIV` instruction
- `divdi3` → hardware `SDIV` instruction
- Compiler-rt still needed for: `floatundidf`, `floatundisf` (float conversions)
- ARM64-specific: `__aeabi_*` functions not needed (not ARM EABI)

### 6.5 libm (Math Library)

`lib/libm/arch/aarch64/` already exists with:
- `fenv.c` — Floating-point environment
- `e_sqrt.S` — Square root

Need to add:
- `s_ceil.S`, `s_floor.S`, `s_rint.S` — ARM64 NEON/FPSIMD versions
- `s_copysign.S` — ARM64 FABS/FSIGN operations

### 6.6 compiler-rt ints

ARM64 needs compiler-rt builtins for division:
```c
// ARM64 has hardware UDIV/SDIV — compiler-rt division is only needed
// for older ARMv8.0 cores that might not have division (rare).
// Standard ARMv8-A has hardware integer division.
```

### 6.7 MINIX CMakeLists Updates

| File | Change |
|------|--------|
| `minix/kernel/CMakeLists.txt` | Add `MACHINE_ARCH STREQUAL "aarch64"` with arch sources, unpaged objects |
| `minix/servers/vm/CMakeLists.txt` | Add aarch64 arch sources section |
| `minix/lib/libsys/CMakeLists.txt` | Add aarch64 arch sources section |
| `minix/lib/libminc/CMakeLists.txt` | Add aarch64 arch sources |
| `minix/drivers/CMakeLists.txt` | Add aarch64 arch-conditional driver sections |
| `CMakeLists.txt` | Add aarch64 architecture detection |

**Estimated effort**: 3-4 weeks
**Dependencies**: Phases 2-5 (kernel, memory, interrupts, syscalls)

---

## Phase 7: Platform + Drivers 🟡

### 7.1 Platform Selection

**Primary target: QEMU virt (for development and testing)**
- GIC v2 emulation
- UART 8250/PL011 serial
- ARM Generic Timer
- No specialized hardware needed

**Secondary target: Raspberry Pi 4**
- CPU: Broadcom BCM2711 (Cortex-A72, ARMv8.0-A)
- GIC: v2 (no v3)
- UART: PL011 (mini UART)
- Timer: ARM Generic Timer
- Interrupts: GIC-400
- Memory map: 1GB base (RPi 4 has 1-8GB RAM)

**Tertiary target: AWS Graviton / Generic ARMv8**
- UEFI boot
- ACPI or Device Tree
- GIC v3
- PL011 or SBSA UART

### 7.2 Platform-Specific Code

**QEMU virt (minimal):**
```c
/* QEMU virt memory map: */
#define UART_BASE       0x09000000   // PL011 UART
#define GIC_DIST_BASE   0x08000000   // GIC v2 distributor
#define GIC_CPU_BASE    0x08010000   // GIC v2 CPU interface
#define RTC_BASE        0x09010000   // PL031 RTC
```

**Raspberry Pi 4:**
```c
/* RPi 4 memory map (BCM2711): */
#define UART_BASE       0xFE215000   // PL011 UART
#define GIC_DIST_BASE   0xFF840000   // GIC-400 distributor
#define GIC_CPU_BASE    0xFF842000   // GIC-400 CPU interface
#define ARM_LOCAL_BASE  0xFF800000   // ARM peripheral (mailbox, etc.)
#define MAILBOX_BASE    0xFE00B880   // VC mailbox
```

### 7.3 Device Drivers

| Driver | ARM32 Status | ARM64 Required Changes |
|--------|-------------|----------------------|
| UART (PL011) | ✅ Not used (AM335x UART) | ✅ New: needed for all ARM64 platforms |
| UART (8250) | ✅ Used on x86 | ✅ Compatible (memory-mapped) |
| GPIO | ✅ OMAP-specific | ❌ New: RPi GPIO, or generic GPIO via DTS |
| MMC/SD | ✅ MMC on AM335x | ❌ New: RPi eMMC/SD or generic SDHCI |
| USB | ✅ MUSB (AM335x) | ❌ New: RPi USB (DWC2) or generic XHCI |
| Clock | ✅ AM335x-specific | ❌ New: ARM Generic Timer + platform counter |
| Interrupt | ✅ OMAP INTC | ❌ New: GIC v2/v3 |
| PCIe | ❌ N/A | ❌ New: RPi 4 PCIe, server ARM64 PCIe |
| SATA | ❌ N/A | ❌ New: If needed for server platforms |
| Framebuffer | ✅ OMAP DSS | ❌ New: RPi VC4/DRM or simple framebuffer |

### 7.4 Device Tree Support

ARM64 platforms universally use Device Tree (FDT) or ACPI:

```c
/* Minimal FDT parser for boot info: */
int fdt_init(void *fdt_ptr) {
    // Walk the FDT to find:
    // - Memory size and layout
    // - CPU cores
    // - GIC base addresses
    // - UART base address
    // - Timer frequency
}

void fdt_get_memory(void *fdt_ptr, uint64_t *base, uint64_t *size) {
    // Parse /memory node
}

void fdt_get_chosen(void *fdt_ptr, char *cmdline, int max_len) {
    // Parse /chosen node for bootargs
}
```

### Files to Create (Phase 7)

| File | Purpose |
|------|---------|
| `minix/kernel/arch/aarch64/platform_qemu.c` | QEMU virt-specific initialization |
| `minix/kernel/arch/aarch64/platform_rpi4.c` | RPi 4-specific initialization |
| `minix/kernel/arch/aarch64/fdt.c` | Flattened Device Tree parser |
| `minix/kernel/arch/aarch64/arch_reset.c` | System reset (PSCI or watchdog) |
| `minix/drivers/tty/tty/arch/aarch64/pl011.c` | PL011 UART driver |
| `minix/drivers/tty/tty/arch/aarch64/arch_tty.c` | ARM64 TTY architecture setup |
| `minix/include/arch/aarch64/include/bcm2711.h` | RPi 4 memory map definitions |
| `minix/include/arch/aarch64/include/platform.h` | Platform detection and constants |

**Estimated effort**: 4-6 weeks
**Dependencies**: Phases 4-6 (interrupts, syscalls, libraries)

---

## Phase 8: Testing and Polish 🟢

### 8.1 QEMU Test Environment

```bash
# QEMU ARM64 system emulation
qemu-system-aarch64 \
    -M virt \
    -cpu cortex-a72 \
    -m 1024 \
    -kernel minix/kernel \
    -nographic \
    -append "root=/dev/vda"
```

### 8.2 Test Suite

| Test | Description | Phase Validated |
|------|-------------|-----------------|
| Boot test | Kernel boots to prompt | Phase 2 |
| Memory test | VM page allocation works | Phase 3 |
| Interrupt test | Timer interrupts fire | Phase 4 |
| Syscall test | IPC between servers works | Phase 5 |
| Fork test | Process creation works | Phase 5 |
| Signal test | Signal delivery works | Phase 5 |
| Driver test | UART output works | Phase 7 |
| Filesystem test | MFS operations work | Phase 7 |
| Network test | lwIP works | Phase 7+ |

### 8.3 Performance Benchmarks

| Benchmark | ARM32 Baseline | ARM64 Target |
|-----------|---------------|--------------|
| Boot time | ~5s (BeagleBone) | <2s (QEMU virt) |
| Context switch | ~1µs | <0.5µs (64-bit regs) |
| IPC latency | ~2µs | <1µs |
| Memcpy (1MB) | ~5ms | <1ms (NEON) |
| SHA-256 (1MB) | ~20ms | <5ms (ARMv8 crypto ext) |

### 8.4 Documentation

| Document | Description |
|----------|-------------|
| `docs/arm64-build-guide.md` | How to build MINIX for ARM64 |
| `docs/arm64-platform-guide.md` | Supported ARM64 platforms |
| `docs/arm64-porting-guide.md` | How to port to new ARM64 platforms |

**Estimated effort**: 2-3 weeks
**Dependencies**: All previous phases

---

## Implementation Order (Recommended)

```
Month 1-2:  Build Infrastructure (Phase 1) + Planning
            → cmake/arch_aarch64.cmake, toolchain, presets
            → QEMU aarch64 system boot test
            → Study NetBSD aarch64 code for reference

Month 3-4:  Kernel Bootstrap (Phase 2)
            → head.S (EL1 entry, MMU enable)
            → vectors.S (exception vector table)
            → mpx.S (context save/restore)
            → klib.S (kernel library)
            → linker script
            → "Hello from ARM64 kernel" in QEMU

Month 5:    Memory Management (Phase 3)
            → ARM64 4-level translation tables
            → VM server adaptation
            → TLB maintenance operations

Month 6:    Interrupts + Timers (Phase 4)
            → GIC v2 initialization (QEMU virt)
            → ARM generic timer
            → Interrupt routing

Month 7:    System Calls + Signals (Phase 5)
            → SVC exception handler
            → IPC via message passing
            → Signal delivery framework

Month 8:    Libraries (Phase 6)
            → libsys arch (spin, timer, serial)
            → libminc arch (setjmp/longjmp)
            → libc arch (IPC, ucontext)
            → CMakeLists updates

Month 9-10: Platform + Drivers (Phase 7)
            → Platform initialization (QEMU)
            → PL011 UART driver
            → RPi 4 support
            → Device Tree parsing
            → Boot to shell

Month 11-12: Testing + Polish (Phase 8)
            → Full functional testing
            → Performance benchmarking
            → Documentation
            → CI integration
```

## Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|-----------|--------|------------|
| ARM64 ASM bugs in exception vectors | High | Critical | Start with QEMU, single-step, compare with Linux/NetBSD |
| Page table complexity (4-level) | High | High | Start with 3-level (64KB pages) for simplicity, move to 4KB |
| GIC v2 vs v3 compatibility issues | Medium | High | Support both via probe, test QEMU (v2) and RPi 4 (v2) |
| Device Tree parsing complexity | Medium | Medium | Use minimal FDT parser, defer full libfdt integration |
| No hardware access for testing | High | Medium | QEMU virt is sufficient for all kernel/syscall development |
| RPi 4 hardware-specific issues | Medium | High | Start QEMU-only, add RPi 4 when HW is available |
| Toolchain missing aarch64 support | Low | High | Use GCC 10+ or LLVM/Clang 12+ (both widely available) |
| MINIX-specific BSP assumptions | Medium | Medium | Review BSP code for arch-specific assumptions early |

## Dependencies

### Required
- ✅ Build System Migration (CMake) — completed
- aarch64 cross-compiler (GCC 10+, Clang 12+, or LLVM)
- QEMU with aarch64 system emulation (`qemu-system-aarch64`)
- Reference: NetBSD aarch64 port (`sys/arch/evbarm64/`)

### Optional
- Raspberry Pi 4 hardware for physical testing
- Raspberry Pi 4 UART (PL011) for serial debug
- JTAG debugger for low-level debugging

## Key ARM64 References

| Resource | URL/Path | Use |
|----------|----------|-----|
| ARM Architecture Reference Manual (ARMv8) | ARM DDI 0487 | Definitive ARM64 reference |
| ARM64 System Register XML | https://developer.arm.com/architectures/system-architectures | System register descriptions |
| QEMU virt platform | `qemu-system-aarch64 -M virt` | Development/testing platform |
| NetBSD evbarm64 port | `sys/arch/evbarm64/` in NetBSD | Reference MINIX-like ARM64 implementation |
| Linux arm64 boot protocol | Documentation/arm64/booting.rst | ARM64 kernel boot requirements |
| Raspberry Pi 4 documentation | https://www.raspberrypi.com/documentation/ | RPi 4 hardware details |
| ARM GIC v2/v3 specification | ARM IHI 0048/0069 | Generic Interrupt Controller |
| ARM Generic Timer specification | ARM DDI 0487, chapter D10 | Timer and counter |

## Success Criteria

**Phase 2 (minimal bootable):**
- [ ] ARM64 kernel boots in QEMU virt
- [ ] Prints startup messages via PL011 UART
- [ ] MMU enabled with correct page tables

**Phase 3-4 (functional kernel):**
- [ ] VM initializes with ARM64 translation tables
- [ ] GIC handles interrupts correctly
- [ ] Timer interrupts fire and schedule processes

**Phase 5-6 (server communication):**
- [ ] Multiple processes run in user mode
- [ ] System calls work between servers
- [ ] Signal delivery works correctly
- [ ] IPC message passing works

**Phase 7 (platform support):**
- [ ] QEMU virt platform fully functional
- [ ] RPi 4 boots and runs basic commands
- [ ] PL011 UART for serial console
- [ ] Basic filesystem operations work

**Phase 8 (production ready):**
- [ ] Shell and basic commands work on ARM64
- [ ] File system operations work on multiple platforms
- [ ] Network stack functional
- [ ] All tests passing on QEMU and RPi 4

---

## Appendix: ARM64 vs ARM32 System Register Comparison

| Register | ARM32 | ARM64 | Purpose |
|----------|-------|-------|---------|
| Program Counter | R15 | PC (not a GPR) | Dedicated PC in ARM64 |
| Stack Pointer | R13 (banked) | SP_EL0/SP_EL1 | Separate user/kernel SP |
| Link Register | R14 (banked) | X30 | Return address |
| Saved State | SPSR_* (banked) | SPSR_EL1 | Saved processor state |
| Exception Return | MOVS PC, LR | ERET | Return from exception |
| Page Table Base | TTBR0/TTBR1 | TTBR0_EL1/TTBR1_EL1 | Translation tables |
| Vector Base | VBAR (non-standard) | VBAR_EL1 | Exception vector table |
| Fault Address | DFSR/FAR | FAR_EL1 | Data fault address |
| Instruction Fault | IFSR | ESR_EL1, FAR_EL1 | Instruction fault info |
| Cache control | CP15 (c7) | SCTLR_EL1, CSSELR_EL1 | System control |
| TLB maintenance | CP15 (c8) | TLBI * | TLB invalidation |
| Cycle counter | PMU counters | CNTPCT_EL0 | Generic system counter |
| CPU ID | CP15 (c0) | MIDR_EL1, REVIDR_EL1 | CPU identification |
| Memory type | CP15 (c2) | MAIR_EL1 | Memory attribute indirection |

## Appendix: GPR Mapping

| ARM32 | ARM64 | Callee Saved | Purpose |
|-------|-------|-------------|---------|
| r0 | x0 | No | Argument/return value |
| r1 | x1 | No | Argument |
| r2 | x2 | No | Argument |
| r3 | x3 | No | Argument |
| r4 | x4 | No | Argument |
| r5 | x5 | No | Argument |
| r6 | x6 | No | (or 7th arg on stack) |
| r7 | x7 | No | Syscall number (MINIX convention) |
| r8 | x8 | No | Indirect result / syscall number |
| — | x9–x15 | No | Temporary registers |
| r8 | x16 | No | IP0 (intra-procedure call) |
| r9 | x17 | No | IP1 (intra-procedure call) |
| r10 | x18 | No | Platform register (TLS, etc.) |
| r11 | x19 | **Yes** | Frame pointer alternative |
| r12 | x20 | **Yes** | Callee-saved |
| — | x21–x28 | **Yes** | Callee-saved |
| r11/fp | x29 | **Yes** | Frame pointer |
| r14/lr | x30 | No | Link register |
| sp/r13 | SP_EL0 | — | User stack pointer |

*Note: ARM64 has 31 GPRs (x0–x30) × 64-bit vs ARM32's 16 GPRs (r0–r15) × 32-bit. The callee-saved registers are x19–x29 (11 registers vs ARM32's r4–r11, which are 8).*
