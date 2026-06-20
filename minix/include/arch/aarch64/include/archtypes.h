/* ============================================================
 * archtypes.h — ARM64 Architecture Types
 *
 * ARM64-specific type definitions for the MINIX kernel:
 *   - stackframe_s: register save area for context switching
 *   - segframe_s: segment/memory context
 *   - Exception frame layout
 *
 * ARM64 register conventions:
 *   x0–x7:  Argument/result registers
 *   x8:     Indirect result register (or syscall number)
 *   x9–x15: Temporary registers
 *   x16:    IP0 (intra-procedure-call)
 *   x17:    IP1 (intra-procedure-call)
 *   x18:    Platform register (TLS)
 *   x19–x28: Callee-saved registers
 *   x29:    Frame pointer (FP)
 *   x30:    Link register (LR)
 *   SP_EL0: User stack pointer
 *   SP_EL1: Kernel stack pointer
 *
 * Exception state:
 *   ELR_EL1:  Exception return address (PC at exception)
 *   SPSR_EL1: Saved processor state
 *   FAR_EL1:  Fault address (for page faults, alignment, etc.)
 *   ESR_EL1:  Exception syndrome register
 *
 * See ARM DDI 0487 (ARMv8-A Architecture Reference Manual)
 * ============================================================ */

#ifndef _AARCH64_ARCHTYPES_H
#define _AARCH64_ARCHTYPES_H

#include <stdint.h>

/* Register type — used by kernel structures (priv.h, proc.h) */
typedef u64_t reg_t;

/* =========================================================================
 * Stack frame — register save area for context switching
 *
 * When an exception occurs (or a context switch is needed), the kernel
 * saves the CPU registers to this structure on the kernel stack.
 *
 * ARM64 has 31 general-purpose registers (x0–x30), plus SP_EL0 (user
 * stack pointer), ELR_EL1 (return PC), and SPSR_EL1 (saved PSTATE).
 *
 * Total: 31 + 3 = 34 × 8 = 272 bytes
 *
 * Stack layout (from high to low):
 *   [SP+272]  <- bottom of save area
 *   [SP+264]  SPSR_EL1
 *   [SP+256]  ELR_EL1
 *   [SP+248]  SP_EL0
 *   [SP+240]  x30 (LR)
 *   ...
 *   [SP+0]    x0
 * ========================================================================= */

struct stackframe_s {
    /* General-purpose registers (x0–x30) */
    uint64_t gpr[31];

    /* User stack pointer (SP_EL0) */
    uint64_t sp_el0;

    /* Exception return address and state */
    uint64_t elr_el1;          /* PC at exception */
    uint64_t spsr_el1;         /* Saved PSTATE */
};

/* Offset macros for assembly (used by mpx.S) */
#define GPR_OFFSET(n)       (n * 8)
#define SP_EL0_OFFSET       (31 * 8)   /* After gpr[31] */
#define ELR_EL1_OFFSET      (32 * 8)
#define SPSR_EL1_OFFSET     (33 * 8)
#define FRAME_SIZE          (34 * 8)

/* =========================================================================
 * Segment frame — memory context
 *
 * ARM64 uses TTBR0_EL1 for user page tables and TTBR1_EL1 for kernel
 * page tables. Each process has its own TTBR0_EL1 value.
 * ========================================================================= */

typedef struct segframe {
    /* Page table base register for user space */
    uint64_t ttbr0_el1;

    /* Address Space ID (for TLB efficiency) */
    uint64_t asid;             /* Optional, Phase 6+ */

    /* Translation Control Register (per-process ASID) */
    uint64_t tcr_el1;
} segframe_t;

/* =========================================================================
 * Exception frame — layout of values pushed by CPU on exception entry
 *
 * When an exception occurs (with CPL change from EL0 to EL1), the CPU
 * automatically pushes:
 *   [SP]     SS (if coming from AArch32, unused for AArch64→AArch64)
 *   [SP+0]   SP_EL0 (user stack pointer)
 *   [SP+8]   SPSR_EL1 (saved PSTATE)
 *   [SP+16]  ELR_EL1 (return PC)
 *
 * Note: ARM64 does NOT push SS/RSP for AArch64→AArch64 transitions
 * (unlike x86_64). Only SPSR_EL1 and ELR_EL1 are saved automatically.
 * User SP must be saved manually by the handler.
 *
 * For our context save:
 *   [SP]     x0..x30 (saved manually)
 *   [SP+248] SP_EL0 (saved manually)
 *   [SP+256] ELR_EL1 (from CPU or saved manually)
 *   [SP+264] SPSR_EL1 (from CPU)
 * ========================================================================= */

struct exception_frame {
    /* These are saved by the exception handler, not the CPU */
    struct stackframe_s stackframe;   /* Full register save area */
};

/* =========================================================================
 * Process structure ARM64-specific fields
 *
 * Added to struct proc in proc.h via #ifdef __aarch64__.
 * ========================================================================= */

/* Additional process fields for ARM64 (defined in proc.h) */
#define ARCH_PROC_NEW_FIELDS                                                \
    uint64_t p_ttbr0;              /* TTBR0_EL1 for process */             \
    uint64_t p_asid;               /* Address Space ID */                  \
    uint64_t p_sp_el0;             /* User stack pointer (for HW ctx) */   \
    uint32_t p_kern_trap_style;    /* How to return to user mode */

/* =========================================================================
 * Useful constants for assembly code
 * ========================================================================= */

/* Exception vector offsets (from VBAR_EL1 base) */
#define VECTOR_EL1t_SYNC      0x000
#define VECTOR_EL1t_IRQ       0x080
#define VECTOR_EL1t_FIQ       0x100
#define VECTOR_EL1t_SERROR    0x180
#define VECTOR_EL1h_SYNC      0x200
#define VECTOR_EL1h_IRQ       0x280
#define VECTOR_EL1h_FIQ       0x300
#define VECTOR_EL1h_SERROR    0x380
#define VECTOR_EL0_SYNC_64    0x400
#define VECTOR_EL0_IRQ_64     0x480
#define VECTOR_EL0_FIQ_64     0x500
#define VECTOR_EL0_SERROR_64  0x580
#define VECTOR_EL0_SYNC_32    0x600
#define VECTOR_EL0_IRQ_32     0x680
#define VECTOR_EL0_FIQ_32     0x700
#define VECTOR_EL0_SERROR_32  0x780

/* Exception class codes (from ESR_EL1.EC) */
#define EC_UNKNOWN            0x00
#define EC_WFI                0x01
#define EC_SVC64              0x15     /* SVC from AArch64 */
#define EC_HVC64              0x16     /* HVC from AArch64 */
#define EC_SMC64              0x17     /* SMC from AArch64 */
#define EC_SYS64              0x18     /* MSR/MRS from AArch64 */
#define EC_IABORT_EL0         0x20     /* Instruction abort EL0 */
#define EC_IABORT_EL1         0x21     /* Instruction abort EL1 */
#define EC_DABORT_EL0         0x24     /* Data abort EL0 */
#define EC_DABORT_EL1         0x25     /* Data abort EL1 */
#define EC_SP_ALIGN           0x26     /* Stack alignment */
#define EC_FP32               0x28     /* Floating point (AArch32) */
#define EC_FP64               0x2C     /* Floating point (AArch64) */
#define EC_SERROR             0x2F     /* SError */
#define EC_BREAKPOINT_EL0     0x30     /* Breakpoint EL0 */
#define EC_BREAKPOINT_EL1     0x31     /* Breakpoint EL1 */

#endif /* _AARCH64_ARCHTYPES_H */
