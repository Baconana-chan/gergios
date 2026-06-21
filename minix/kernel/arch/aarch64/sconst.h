/* ============================================================
 * sconst.h — ARM64 context save/restore assembly macros
 *
 * Stack layout for ARM64 exception handling:
 *
 * When an exception occurs from EL0 (user) to EL1 (kernel):
 *   CPU automatically saves:
 *     ELR_EL1 = return PC
 *     SPSR_EL1 = saved PSTATE
 *   CPU does NOT save SP_EL0 (user stack) — must be saved manually
 *   CPU does NOT save GPRs — must be saved manually
 *
 * Stack layout (after manual save by handler):
 *   [SP+0..247]   x0..x30 (31 × 8 bytes)
 *   [SP+248]      SP_EL0
 *   [SP+256]      ELR_EL1
 *   [SP+264]      SPSR_EL1
 *
 * Exception syndrome:
 *   ESR_EL1: exception class (EC), instruction specific syndrome
 *   FAR_EL1: fault address (page faults, alignment, etc.)
 * ============================================================ */

#ifndef _AARCH64_SCONST_H
#define _AARCH64_SCONST_H

#include "kernel/const.h"
#include "procoffsets.h"

/* =========================================================================
 * Stack frame offsets (for use in assembly)
 * ========================================================================= */

/* Offset from proc struct to the GP registers in stackframe_s */
#define GPR_REG_OFFSET(n)       (n * 8)

/* Named register offsets (matching archtypes.h + proc.h naming) */
#define AXREG(pptr)             (pptr + GPR_REG_OFFSET(0))   /* x0 */
#define BXREG(pptr)             (pptr + GPR_REG_OFFSET(1))   /* x1 */
#define CXREG(pptr)             (pptr + GPR_REG_OFFSET(2))   /* x2 */
#define DXREG(pptr)             (pptr + GPR_REG_OFFSET(3))   /* x3 */
#define SIREG(pptr)             (pptr + GPR_REG_OFFSET(4))   /* x4 */
#define DIREG(pptr)             (pptr + GPR_REG_OFFSET(5))   /* x5 */
#define R6REG(pptr)             (pptr + GPR_REG_OFFSET(6))   /* x6 */
#define R7REG(pptr)             (pptr + GPR_REG_OFFSET(7))   /* x7 */
#define R8REG(pptr)             (pptr + GPR_REG_OFFSET(8))   /* x8 */
#define R9REG(pptr)             (pptr + GPR_REG_OFFSET(9))   /* x9 */
#define R10REG(pptr)            (pptr + GPR_REG_OFFSET(10))  /* x10 */
#define R11REG(pptr)            (pptr + GPR_REG_OFFSET(11))  /* x11 */
#define R12REG(pptr)            (pptr + GPR_REG_OFFSET(12))  /* x12 */
#define R13REG(pptr)            (pptr + GPR_REG_OFFSET(13))  /* x13 */
#define R14REG(pptr)            (pptr + GPR_REG_OFFSET(14))  /* x14 */
#define R15REG(pptr)            (pptr + GPR_REG_OFFSET(15))  /* x15 */
#define R16REG(pptr)            (pptr + GPR_REG_OFFSET(16))  /* x16 */
#define R17REG(pptr)            (pptr + GPR_REG_OFFSET(17))  /* x17 */
#define R18REG(pptr)            (pptr + GPR_REG_OFFSET(18))  /* x18 */
#define R19REG(pptr)            (pptr + GPR_REG_OFFSET(19))  /* x19 */
#define R20REG(pptr)            (pptr + GPR_REG_OFFSET(20))  /* x20 */
#define R21REG(pptr)            (pptr + GPR_REG_OFFSET(21))  /* x21 */
#define R22REG(pptr)            (pptr + GPR_REG_OFFSET(22))  /* x22 */
#define R23REG(pptr)            (pptr + GPR_REG_OFFSET(23))  /* x23 */
#define R24REG(pptr)            (pptr + GPR_REG_OFFSET(24))  /* x24 */
#define R25REG(pptr)            (pptr + GPR_REG_OFFSET(25))  /* x25 */
#define R26REG(pptr)            (pptr + GPR_REG_OFFSET(26))  /* x26 */
#define R27REG(pptr)            (pptr + GPR_REG_OFFSET(27))  /* x27 */
#define R28REG(pptr)            (pptr + GPR_REG_OFFSET(28))  /* x28 */
#define FPREG(pptr)             (pptr + GPR_REG_OFFSET(29))  /* x29 = FP */
#define LRREG(pptr)             (pptr + GPR_REG_OFFSET(30))  /* x30 = LR */

/* Additional saved state (after GPRs in stackframe_s) */
#define SPREG(pptr)             (pptr + (31 * 8))            /* SP_EL0 */
#define PCREG(pptr)             (pptr + (32 * 8))            /* ELR_EL1 */
#define PSWREG(pptr)            (pptr + (33 * 8))            /* SPSR_EL1 */
#define P_KERN_TRAP_STYLE(pptr) (pptr + (34 * 8))            /* Trap style */

/* =========================================================================
 * Kernel stack layout
 *
 * The kernel stack per-CPU area contains:
 *   [stack_top - 8]  = current process pointer
 *   [stack_top - 16] = current CPU id
 * ========================================================================= */

#define CURR_PROC_PTR           (-8)
#define CURR_CPU_ID             (-16)

/* =========================================================================
 * Exception frame offsets
 *
 * ARM64 exception stack (after handler saves context):
 *   [SP]     x0 (saved manually)
 *   ...
 *   [SP+248] SP_EL0 (saved manually)
 *   [SP+256] ELR_EL1 (from CPU)
 *   [SP+264] SPSR_EL1 (from CPU)
 * ========================================================================= */

#define STACKFRAME_SIZE         (34 * 8)      /* 272 bytes */

/* =========================================================================
 * Context save/restore macros
 * ========================================================================= */

/*
 * SAVE_GPRS: Save all general-purpose registers (x0–x30) to stack
 *
 * ARM64: Use STP (Store Pair) for efficiency.
 * Saves to the top of the kernel stack.
 */
.macro SAVE_GPRS
    stp     x0, x1, [sp, #-16]!
    stp     x2, x3, [sp, #-16]!
    stp     x4, x5, [sp, #-16]!
    stp     x6, x7, [sp, #-16]!
    stp     x8, x9, [sp, #-16]!
    stp     x10, x11, [sp, #-16]!
    stp     x12, x13, [sp, #-16]!
    stp     x14, x15, [sp, #-16]!
    stp     x16, x17, [sp, #-16]!
    stp     x18, x19, [sp, #-16]!
    stp     x20, x21, [sp, #-16]!
    stp     x22, x23, [sp, #-16]!
    stp     x24, x25, [sp, #-16]!
    stp     x26, x27, [sp, #-16]!
    stp     x28, x29, [sp, #-16]!
    str     x30, [sp, #-16]!
.endm

/*
 * RESTORE_GPRS: Restore all general-purpose registers from stack
 */
.macro RESTORE_GPRS
    ldr     x30, [sp], #16
    ldp     x28, x29, [sp], #16
    ldp     x26, x27, [sp], #16
    ldp     x24, x25, [sp], #16
    ldp     x22, x23, [sp], #16
    ldp     x20, x21, [sp], #16
    ldp     x18, x19, [sp], #16
    ldp     x16, x17, [sp], #16
    ldp     x14, x15, [sp], #16
    ldp     x12, x13, [sp], #16
    ldp     x10, x11, [sp], #16
    ldp     x8, x9, [sp], #16
    ldp     x6, x7, [sp], #16
    ldp     x4, x5, [sp], #16
    ldp     x2, x3, [sp], #16
    ldp     x0, x1, [sp], #16
.endm

/*
 * SAVE_EXTRA_STATE: Save SP_EL0, ELR_EL1, SPSR_EL1
 *
 * These are the additional registers beyond GPRs that need saving.
 */
.macro SAVE_EXTRA_STATE
    mrs     x0, SP_EL0
    mrs     x1, ELR_EL1
    mrs     x2, SPSR_EL1
    stp     x0, x1, [sp, #-16]!
    str     x2, [sp, #-8]!
.endm

/*
 * RESTORE_EXTRA_STATE: Restore SP_EL0, ELR_EL1, SPSR_EL1
 */
.macro RESTORE_EXTRA_STATE
    ldr     x2, [sp], #8
    ldp     x0, x1, [sp], #16
    msr     SP_EL0, x0
    msr     ELR_EL1, x1
    msr     SPSR_EL1, x2
.endm

/*
 * SAVE_FULL_CONTEXT: Save all registers on exception entry
 *
 * Stack layout after SAVE_FULL_CONTEXT:
 *   [SP+0]   SPSR_EL1
 *   [SP+8]   ELR_EL1
 *   [SP+16]  SP_EL0
 *   [SP+24]  x30..x0 (saved by SAVE_GPRS)
 *   [SP+272] <- original SP (no more data below this)
 */
.macro SAVE_FULL_CONTEXT
    SAVE_GPRS
    SAVE_EXTRA_STATE
.endm

/*
 * RESTORE_FULL_CONTEXT: Restore all registers and return to user
 *
 * Uses ERET to return (restores PC from ELR_EL1, PSTATE from SPSR_EL1).
 */
.macro RESTORE_FULL_CONTEXT
    RESTORE_EXTRA_STATE
    RESTORE_GPRS
    eret
.endm

/*
 * TEST_INT_IN_KERNEL: Check if exception came from kernel mode
 *
 * On ARM64, we check SPSR_EL1 to determine if we came from EL1 or EL0.
 * SPSR_EL1[3:0] contains the exception level and stack pointer selection
 * for the interrupted context.
 *
 *   SPSR_EL1.M[3:0] = 0b0000  -> EL0t
 *   SPSR_EL1.M[3:0] = 0b0100  -> EL1t
 *   SPSR_EL1.M[3:0] = 0b0101  -> EL1h
 *
 * Offset from SP: SPSR is at the top of the saved context.
 * displ: displacement from SP to SPSR_EL1 in current frame.
 */
.macro TEST_INT_IN_KERNEL, displ, label
    ldr     x0, [sp, #\displ]
    and     x0, x0, #0xF        /* Extract M[3:0] */
    cmp     x0, #0x4            /* EL1t or EL1h? */
    b.ge    \label
.endm

/*
 * SAVE_PROCESS_CTX: Save full process context on exception from user
 *
 * Used when an exception/interrupt occurs while running user code.
 * Saves all registers, records the trap style.
 *
 * displ: displacement from stack top to saved context
 * trapcode: trap style indicator (KTS_INT_HARD, KTS_SYSCALL, etc.)
 */
.macro SAVE_PROCESS_CTX, displ, trapcode
    SAVE_FULL_CONTEXT
    /* Get current process pointer from per-CPU area */
    mrs     x0, tpidr_el1       /* Per-CPU data pointer */
    ldr     x1, [x0, #CURR_PROC_PTR]  /* Current proc */
    /* Store trap style */
    mov     x2, #\trapcode
    str     x2, [x1, #P_KERN_TRAP_STYLE]
.endm

/*
 * RESTORE_KERNEL_SEGS: Set up kernel mode segments
 * ARM64 doesn't use segmentation like x86, but we need to ensure
 * we're in the right context.
 */
.macro RESTORE_KERNEL_SEGS
    /* ARM64: nothing to do — no segment registers */
.endm

#endif /* _AARCH64_SCONST_H */
