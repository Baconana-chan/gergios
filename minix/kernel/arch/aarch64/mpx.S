/* ============================================================
 * mpx.S — ARM64 Process Switching and Message Handling
 *
 * This is the lowest layer of the MINIX kernel. It does process
 * switching and message handling.
 *
 * The kernel is entered via exception vectors (vectors.S).
 * Once the handler is done, switch_to_user() is called to pick
 * a new process, restore its context, and return to userspace.
 *
 * ARM64 exception entry (from EL0 to EL1):
 *   - CPU auto-saves: ELR_EL1 (return PC), SPSR_EL1 (saved PSTATE)
 *   - CPU auto-switches to SP_EL1 (kernel stack)
 *   - CPU does NOT auto-save: SP_EL0 (user stack), GPRs (x0–x30)
 *   - All GPRs and SP_EL0 must be saved manually by the handler
 *
 * For return to userspace, we use ERET which restores:
 *   - PC from ELR_EL1
 *   - PSTATE from SPSR_EL1
 *
 * Reference: ARMv8-A Architecture Reference Manual (DDI 0487),
 *            Chapter D1 (Exception Handling)
 * ============================================================ */

#include <machine/asm.h>
#include <machine/vm.h>

#include "kernel/kernel.h"
#include "kernel/const.h"
#include "kernel/proc.h"
#include "sconst.h"

#include <minix/com.h>
#include <minix/config.h>
#include <minix/const.h>
#include <minix/ipcconst.h>

#include "arch_proto.h"      /* K_STACK_SIZE, get_k_stack_top */

/* =========================================================================
 * Constants
 * ========================================================================= */

/* ESR_EL1 fields for SVC dispatch */
#define ESR_EC_SHIFT          26
#define ESR_EC_SVC64          (0x15 << 26)   /* SVC from AArch64 */

/* SPSR_EL1 mode field */
#define SPSR_M_EL0            0x0000         /* EL0t */
#define SPSR_M_EL1t           0x0004         /* EL1t */
#define SPSR_M_EL1h           0x0005         /* EL1h */

/* =========================================================================
 * save_process_ctx — Save all CPU registers to proc->p_reg
 *
 * Saves the current CPU state to the process structure so that
 * the process can be resumed later via restore_user_context().
 *
 * Stack layout expected on entry (from SAVE_FULL_CONTEXT in vectors.S):
 *   [SP+0]   SPSR_EL1
 *   [SP+8]   ELR_EL1
 *   [SP+16]  SP_EL0
 *   [SP+24]  x30 (LR)
 *   [SP+32]  x29 (FP)
 *   [SP+40]  x28
 *   ... (descending)
 *   [SP+264] x0
 *   [SP+272] <- original SP (before save)
 *
 * After save, proc->p_reg contains:
 *   gpr[0..30] = x0..x30
 *   sp_el0     = SP_EL0
 *   elr_el1    = ELR_EL1
 *   spsr_el1   = SPSR_EL1
 *
 * Input:
 *   x0 = proc_ptr (pointer to struct proc)
 *
 * Clobbers: x1, x2, x3, x4, x5
 * ========================================================================= */
.macro save_process_ctx, proc_ptr_reg
    /* Save GPRs to proc->p_reg.gpr[].
     * Stack layout (from high to low, SAVE_FULL_CONTEXT format):
     *   [SP+248] x0    ← first saved (at stack after SAVE_GPRS)
     *   [SP+240] x1
     *   ...
     *   [SP+0]   x30  ← last saved
     *   [SP-8]   SPSR_EL1
     *   [SP-16]  ELR_EL1
     *   [SP-24]  SP_EL0
     *
     * Wait — the actual SAVE_GPRS pushes pairs like this:
     *   stp x0, x1, [sp, #-16]!   → [SP+0]=x0, [SP+8]=x1, SP-=16
     *   stp x2, x3, [sp, #-16]!   → [SP+0]=x2, [SP+8]=x3, SP-=16
     * So after all 16 pairs + x30:
     *   [SP+240] x0   (pushed first, at highest offset)
     *   [SP+232] x1
     *   [SP+224] x2
     *   ...
     *   [SP+0]   x30  (pushed last, at current SP)
     *
     * Then SAVE_EXTRA_STATE pushes:
     *   [SP-8]   SPSR_EL1
     *   [SP-24]  SP_EL0, ELR_EL1 (stp)
     *
     * So final layout (SP points to after all saves):
     *   [SP+0]   SPSR_EL1   ← last pushed by SAVE_EXTRA_STATE (str x2, [sp, #-8]!)
     *   [SP+8]   ELR_EL1
     *   [SP+16]  SP_EL0
     *   [SP+24]  x30
     *   [SP+32]  x29
     *   ...
     *   [SP+264] x0
     *
     * We copy from stack to proc struct.
     */
    .set _offset, 0
    .rept 31
        ldr     x1, [sp, #(264 - _offset)]
        str     x1, [\proc_ptr_reg, #_offset]
        .set _offset, _offset + 8
    .endr
    .set _offset, 0

    /* Save SP_EL0, ELR_EL1, SPSR_EL1 from stack.
     * After SAVE_FULL_CONTEXT (SAVE_GPRS → SAVE_EXTRA_STATE):
     *   [SP+0]  = SPSR_EL1  (from str x2, [sp, #-8]! — last push)
     *   [SP+8]  = SP_EL0    (from stp x0=SP_EL0, x1=ELR_EL1 — lower addr)
     *   [SP+16] = ELR_EL1   (from stp x0, x1 — upper addr)
     *   [SP+24] = x30..     (start of SAVE_GPRS data)
     */
    ldr     x1, [sp, #8]            /* SP_EL0 */
    str     x1, [\proc_ptr_reg, #248]
    ldr     x1, [sp, #16]           /* ELR_EL1 */
    str     x1, [\proc_ptr_reg, #256]
    ldr     x1, [sp, #0]            /* SPSR_EL1 */
    str     x1, [\proc_ptr_reg, #264]
.endm

/* =========================================================================
 * SAVE_FULL_CONTEXT_TO_PROC — Combination of stack save + proc save
 *
 * This macro saves all registers to the kernel stack (using the existing
 * SAVE_FULL_CONTEXT macro from sconst.h), then copies them to the proc
 * struct. This is the standard entry sequence for exceptions from EL0.
 *
 * After this macro:
 *   - All registers are saved to proc->p_reg
 *   - proc_ptr_reg holds the proc pointer for the interrupted process
 *   - SP points to the kernel stack frame (for C handler use)
 *
 * Input:
 *   proc_ptr_reg = register to use for proc pointer (will be set)
 *
 * Clobbers: proc_ptr_reg, plus all registers used by save_process_ctx
 * ========================================================================= */
.macro SAVE_FULL_CONTEXT_TO_PROC, proc_ptr_reg
    /* Step 1: Save all registers to kernel stack */
    SAVE_FULL_CONTEXT

    /* Step 2: Get current process pointer.
     * For now, use a simple approach: load from per-CPU data.
     * In Phase 4+, this will use tpidr_el1 or a per-CPU stack slot.
     */
    mrs     \proc_ptr_reg, tpidr_el1
    ldr     \proc_ptr_reg, [\proc_ptr_reg]

    /* Step 3: Copy from stack to proc struct */
    save_process_ctx \proc_ptr_reg
.endm

/* =========================================================================
 * SECTION: .unpaged.text
 *
 * All context switching code must be in .unpaged.text (identity-mapped
 * section) because it deals with kernel stacks and process switching
 * before the high kernel mapping is fully active for all processes.
 * ========================================================================= */

.section .unpaged.text, "ax"

/* =========================================================================
 * restore_user_context — Restore process context and return to user mode
 *
 * Called from the C function switch_to_user() in proc.c:
 *   void switch_to_user(void);
 * which picks the next process and calls:
 *   restore_user_context(struct proc *rp);
 *
 * Note: In the MINIX convention, switch_to_user() is called like a
 * function but never returns (it ERETs to userspace). The C compiler
 * must not emit epilogue code. The function is declared as:
 *   void switch_to_user(void) __attribute__((noreturn));
 *
 * Input:
 *   x0 = proc pointer (struct proc *rp)
 *
 * Clobbers: all GPRs (restored from proc struct)
 * Does not return (ERET to userspace)
 * ========================================================================= */
ENTRY(restore_user_context)
    /* Save proc pointer in x1 — we need it to load all registers */
    mov     x1, x0

    /* ----------------------------------------------------------
     * Step 1: Restore exception return registers
     * ---------------------------------------------------------- */

    /* Restore SPSR_EL1 (saved PSTATE for return) */
    ldr     x2, [x1, #PSWREG(0)]
    msr     SPSR_EL1, x2
    isb

    /* Restore ELR_EL1 (return PC) */
    ldr     x2, [x1, #PCREG(0)]
    msr     ELR_EL1, x2
    isb

    /* Restore SP_EL0 (user stack pointer) */
    ldr     x2, [x1, #SPREG(0)]
    msr     SP_EL0, x2

    /* ----------------------------------------------------------
     * Step 2: Push saved x0 and x1 onto kernel stack
     *
     * We need to restore x0 and x1 from the proc struct, but x0
     * is our proc pointer and x1 is our saved base pointer. We
     * push them onto the kernel stack so we can pop them after
     * restoring all other GPRs.
     * ---------------------------------------------------------- */
    ldp     x2, x3, [x1]            /* x2 = saved_x0, x3 = saved_x1 */
    stp     x2, x3, [sp, #-16]!     /* Push saved_x0, saved_x1 */

    /* ----------------------------------------------------------
     * Step 3: Restore GPRs x2–x30 from proc struct
     *
     * We restore in pairs using LDP for efficiency.
     * x1 still holds the proc pointer as base address.
     * ---------------------------------------------------------- */
    ldp     x2, x3,   [x1, #16]
    ldp     x4, x5,   [x1, #32]
    ldp     x6, x7,   [x1, #48]
    ldp     x8, x9,   [x1, #64]
    ldp     x10, x11, [x1, #80]
    ldp     x12, x13, [x1, #96]
    ldp     x14, x15, [x1, #112]
    ldp     x16, x17, [x1, #128]
    ldp     x18, x19, [x1, #144]
    ldp     x20, x21, [x1, #160]
    ldp     x22, x23, [x1, #176]
    ldp     x24, x25, [x1, #192]
    ldp     x26, x27, [x1, #208]
    ldp     x28, x29, [x1, #224]
    ldr     x30,      [x1, #240]

    /* ----------------------------------------------------------
     * Step 4: Restore x0 and x1 from kernel stack
     * ---------------------------------------------------------- */
    ldp     x0, x1, [sp], #16       /* x0 = saved_x0, x1 = saved_x1 */

    /* ----------------------------------------------------------
     * Step 5: Return to userspace via ERET
     *
     * ERET restores:
     *   - PC from ELR_EL1 (set in Step 1)
     *   - PSTATE from SPSR_EL1 (set in Step 1)
     *   - SP stays on SP_EL0 (set in Step 1)
     * ---------------------------------------------------------- */
    eret

END(restore_user_context)

/* =========================================================================
 * arm64_svc_handler — SVC (system call) entry point
 *
 * Called from vectors.S for EL0 Sync exceptions with EC == SVC64.
 * SVC is the ARM64 system call instruction used by user processes
 * to enter the kernel.
 *
 * SVC calling convention (MINIX for ARM64):
 *   x0   = message pointer / arg0
 *   x1   = arg1
 *   x2   = arg2
 *   x3   = arg3
 *   x8   = syscall number (ARM64 convention)
 *         IPCVEC_INTR     — IPC call (send/receive)
 *         KERVEC_INTR     — Kernel call (syscall)
 *         otherwise       — Invalid, return -1
 *
 * On entry (from vectors.S sync_el0_handler):
 *   - SP = SP_EL1 (kernel stack)
 *   - x0..x30 = user values (not yet saved!)
 *   - ELR_EL1 = original SVC return address
 *   - SPSR_EL1 = user PSTATE
 *   - ESR_EL1 = SVC exception syndrome
 *
 * This handler:
 *   1. Saves full context to proc struct
 *   2. Checks x8 for syscall type
 *   3. Calls the appropriate C handler
 *   4. Falls through to switch_to_user for scheduling
 * ========================================================================= */
ENTRY(arm64_svc_handler)
    /* ----------------------------------------------------------
     * Step 1: Save all registers to proc struct
     * ---------------------------------------------------------- */
    SAVE_FULL_CONTEXT_TO_PROC x5    /* x5 = proc_ptr */

    /* x8 holds the syscall type (set by user before SVC).
     * The user's x8 is now saved in proc->p_reg.gpr[8].
     * We reload it from the saved context.
     */
    ldr     x8, [x5, #64]           /* x8 = saved x8 = syscall type */

    /* ----------------------------------------------------------
     * Step 2: Dispatch based on syscall type
     * ---------------------------------------------------------- */
    cmp     x8, #KERVEC_INTR
    beq     arm64_kernel_call

    cmp     x8, #IPCVEC_INTR
    beq     arm64_ipc_entry

    /* ----------------------------------------------------------
     * Step 3: Invalid syscall — return -ENOSYS
     * ---------------------------------------------------------- */
    mov     x0, #-1                 /* Return -1 to userspace */
    str     x0, [x5, #0]           /* x0 in proc->p_reg.gpr[0] */
    b       arm64_switch_to_user

END(arm64_svc_handler)

/* =========================================================================
 * arm64_kernel_call — Dispatch a kernel call
 *
 * Called when x8 == KERVEC_INTR.
 *
 * Arguments from userspace (preserved in proc->p_reg):
 *   x0 = message pointer
 *   x1 = caller process number (src)
 *
 * Calls: void kernel_call(message *msg, struct proc *caller)
 * ========================================================================= */
arm64_kernel_call:
    /* Save proc pointer and message pointer before calling C code.
     * The user's x0 (message ptr) and x5 (proc ptr) are used as args.
     * x5 = proc_ptr, we need to pass x5 as the 2nd arg.
     */
    stp     x5, x30, [sp, #-16]!   /* Save proc ptr and LR */

    /* Reload the user's original x0 (message pointer) from saved context */
    ldr     x0, [x5, #0]           /* x0 = saved x0 = message ptr */

    /* Step 1: Stop user process cycles (call context_stop) */
    /* x0 = x5 (proc_ptr) already — no dead load needed */
    bl      _C_LABEL(context_stop)

    /* Step 2: Call kernel_call(message_ptr, proc_ptr) */
    ldr     x0, [x5, #0]           /* x0 = saved message ptr */
    mov     x1, x5                  /* x1 = caller proc ptr */
    bl      _C_LABEL(kernel_call)

    /* Step 3: Restore proc ptr and jump to switch_to_user */
    ldp     x5, x30, [sp], #16
    b       arm64_switch_to_user

/* =========================================================================
 * arm64_ipc_entry — Dispatch an IPC call
 *
 * Called when x8 == IPCVEC_INTR.
 *
 * Arguments from userspace (preserved in proc->p_reg):
 *   x0 = destination endpoint
 *   x1 = message pointer
 *   x2 = status/flags
 *   x8 = IPCVEC_INTR
 *
 * Calls: int do_ipc(endpoint_t dest, message *m_ptr, int flags)
 *   Returns: result code (stored in proc->p_reg.gpr[0])
 * ========================================================================= */
arm64_ipc_entry:
    /* Save proc pointer */
    stp     x5, x30, [sp, #-16]!   /* Save proc ptr and LR */
    stp     x6, x7, [sp, #-16]!    /* Save temp regs for args */

    /* Load IPC arguments from saved user context */
    ldr     x0, [x5, #0]           /* x0 = saved x0 = dest endpoint */
    ldr     x1, [x5, #8]           /* x1 = saved x1 = message ptr */
    ldr     x2, [x5, #16]          /* x2 = saved x2 = flags/status */

    /* Step 1: Stop user process cycles */
    mov     x6, x0                  /* Save dest endpoint */
    mov     x7, x1                  /* Save message ptr */
    mov     x0, x5                  /* 1st arg: proc_ptr */
    bl      _C_LABEL(context_stop)

    /* Step 2: Call do_ipc(dest, msg_ptr, flags) */
    mov     x0, x6                  /* dest endpoint */
    mov     x1, x7                  /* message pointer */
    mov     x2, x2                  /* flags (preserved) */
    bl      _C_LABEL(do_ipc)

    /* Step 3: Save return value in proc->p_reg.gpr[0] */
    ldr     x5, [sp, #16]          /* Reload proc ptr (was saved earlier) */
    str     x0, [x5, #0]           /* Return value in proc->p_reg.gpr[0] */

    /* Step 4: Clean up and jump to scheduling */
    ldp     x6, x7, [sp], #16
    ldp     x5, x30, [sp], #16
    b       arm64_switch_to_user

/* =========================================================================
 * arm64_irq_entry_from_user — IRQ from EL0 (user mode)
 *
 * Called from vectors.S EL0 IRQ vector when an interrupt occurs
 * while the CPU was running user code.
 *
 * On entry:
 *   - SP = SP_EL1 (kernel stack, from exception entry)
 *   - ELR_EL1 = user PC at point of interrupt
 *   - SPSR_EL1 = user PSTATE
 *
 * This handler:
 *   1. Saves full user context to proc struct
 *   2. Calls bsp_irq_handle() to dispatch the interrupt
 *   3. Falls through to switch_to_user for scheduling
 * ========================================================================= */
ENTRY(arm64_irq_entry_from_user)
    /* Step 1: Save all registers to proc struct */
    SAVE_FULL_CONTEXT_TO_PROC x5    /* x5 = proc_ptr */

    /* Step 2: Stop user process cycles */
    stp     x5, x30, [sp, #-16]!   /* Save proc ptr and LR */
    mov     x0, x5                  /* 1st arg: caller proc ptr */
    bl      _C_LABEL(context_stop)

    /* Step 3: Call the interrupt handler */
    bl      _C_LABEL(bsp_irq_handle)   /* bsp_irq_handle(void) */

    /* Data synchronization barrier (for MMIO consistency) */
    dsb     sy

    /* Step 4: Restore and jump to scheduling */
    ldp     x5, x30, [sp], #16
    b       arm64_switch_to_user

END(arm64_irq_entry_from_user)

/* =========================================================================
 * arm64_irq_entry_from_kernel — IRQ from EL1 (kernel mode)
 *
 * Called from vectors.S EL1 IRQ vector (0x280) when an interrupt
 * occurs while the kernel is executing.
 *
 * Kernel context was pre-saved by vectors.S EL1h handler before
 * dispatching here. This handler just calls the interrupt handler
 * and returns.
 *
 * On entry:
 *   - SP = SP_EL1 (kernel stack, unchanged)
 *   - ELR_EL1 = kernel PC at point of interrupt
 *   - SPSR_EL1 = kernel PSTATE
 *   - Registers x0–x30 = values at interrupt (NOT saved yet for kernel)
 * ========================================================================= */
ENTRY(arm64_irq_entry_from_kernel)
    /* Save all GPRs to stack (kernel context may be in use) */
    SAVE_GPRS

    /* Step 1: Call interrupt handler */
    bl      _C_LABEL(bsp_irq_handle)

    /* Data synchronization barrier */
    dsb     sy

    /* Step 2: Restore GPRs and return to kernel EL1 */
    RESTORE_GPRS
    eret

END(arm64_irq_entry_from_kernel)

/* =========================================================================
 * arm64_exception_entry_from_user — Exception from EL0 (user mode)
 *
 * Called from vectors.S EL0 Sync vector (0x400) when the exception
 * is NOT an SVC (i.e., a fault like page fault, alignment, etc.).
 * SVC exceptions are handled by arm64_svc_handler instead.
 *
 * On entry:
 *   - SP = SP_EL1 (kernel stack)
 *   - ESR_EL1 = exception syndrome
 *   - FAR_EL1 = fault address (if applicable)
 *   - ELR_EL1 = PC at fault
 *   - SPSR_EL1 = user PSTATE
 *
 * Phase 2: Dispatch to exception_handler(is_nested, frame_ptr, vector).
 * Phase 4+: Full page fault handling, signal delivery.
 *
 * Note: The ESR_EL1 exception class is passed as the vector number.
 * ========================================================================= */
ENTRY(arm64_exception_entry_from_user)
    /* Step 1: Save full context to proc struct */
    SAVE_FULL_CONTEXT_TO_PROC x5    /* x5 = proc_ptr */

    /* Step 2: Read ESR_EL1 to determine exception class (vector number) */
    mrs     x4, ESR_EL1            /* x4 = ESR_EL1 */

    /* Step 3: Stop user process cycles */
    stp     x5, x30, [sp, #-16]!   /* Save proc ptr */
    mov     x0, x5                  /* 1st arg: caller proc ptr */
    bl      _C_LABEL(context_stop)

    /* Step 4: Call exception_handler(nested, frame_ptr, vector)
     *   x0 = 0 (not nested — from user, not kernel)
     *   x1 = pointer to saved frame (SP after SAVE_FULL_CONTEXT_TO_PROC)
     *   x2 = exception vector number (from ESR_EL1 EC field)
     */
    mov     x0, #0                  /* Not a nested exception */
    mov     x1, sp                  /* Frame pointer (current SP) */
    mov     x2, x4                  /* Vector = ESR_EL1 (EC field) */
    bl      _C_LABEL(exception_handler)

    /* Step 5: Restore and jump to scheduling */
    ldp     x5, x30, [sp], #16
    b       arm64_switch_to_user

END(arm64_exception_entry_from_user)

/* =========================================================================
 * arm64_exception_entry_from_kernel — Exception from EL1 (kernel mode)
 *
 * Called from vectors.S EL1 Sync vector (0x200) when a fault occurs
 * while executing kernel code (e.g., NULL pointer dereference in C).
 *
 * On entry (from vectors.S sync_el1h_handler):
 *   - SP = SP_EL1 (unchanged, at the point of fault)
 *   - ESR_EL1 = exception syndrome
 *   - FAR_EL1 = fault address
 *   - ELR_EL1 = kernel PC at fault
 *   - x19..x22 = saved by sync_el1h_handler
 *
 * Phase 2: Print diagnostic and halt (kernel panic).
 * Phase 4+: Proper kernel exception handling.
 * ========================================================================= */
ENTRY(arm64_exception_entry_from_kernel)
    /* Save callee-saved registers used by our diagnostic output */
    stp     x19, x20, [sp, #-16]!
    stp     x21, x22, [sp, #-16]!

    /* Read exception syndrome registers */
    mrs     x19, ESR_EL1
    mrs     x20, FAR_EL1
    mrs     x21, ELR_EL1
    mrs     x22, SPSR_EL1

    /* Print diagnostic via PL011 UART */
    adr     x0, str_kernel_panic
    bl      pl011_puts_asm
    bl      pl011_newline_asm

    adr     x0, str_esr
    bl      pl011_puts_asm
    mov     x0, x19
    bl      pl011_puthex_asm
    bl      pl011_newline_asm

    adr     x0, str_far
    bl      pl011_puts_asm
    mov     x0, x20
    bl      pl011_puthex_asm
    bl      pl011_newline_asm

    adr     x0, str_elr
    bl      pl011_puts_asm
    mov     x0, x21
    bl      pl011_puthex_asm
    bl      pl011_newline_asm

    /* Phase 2: Halt on kernel exception */
    adr     x0, str_halt
    bl      pl011_puts_asm
    bl      pl011_newline_asm

    ldp     x21, x22, [sp], #16
    ldp     x19, x20, [sp], #16

    /* Disable all exceptions and halt */
    msr     DAIFSet, #0xF
1:  wfi
    b       1b

END(arm64_exception_entry_from_kernel)

/* =========================================================================
 * arm64_switch_to_user — Schedule next process and return to user
 *
 * This is the common exit point for all exception handlers.
 * It calls the C function switch_to_user() which:
 *   1. Picks the next runnable process
 *   2. Sets up page tables (switches TTBR0_EL1 if needed)
 *   3. Calls restore_user_context(proc_ptr)
 *
 * switch_to_user() never returns (it ERETs to userspace).
 * ========================================================================= */
arm64_switch_to_user:
    bl      _C_LABEL(switch_to_user)
    /* NOTREACHED — switch_to_user does not return */
1:  b       1b

/* =========================================================================
 * PL011 UART Assembly Diagnostics
 *
 * Simple assembly functions for debug output via PL011 UART. These are
 * called from exception handlers when we cannot rely on C printf().
 *
 * All functions are in .unpaged.text (identity-mapped).
 * ========================================================================= */

#define PL011_BASE              0x09000000
#define PL011_DR                (PL011_BASE + 0x000)
#define PL011_FR                (PL011_BASE + 0x018)
#define PL011_FR_TXFF           (1 << 5)
#define PL011_FR_BUSY           (1 << 3)

/*
 * pl011_putchar_asm — Write a single character to PL011 UART (assembly)
 *   x0 = character to write
 *   Clobbers: x0, x1
 *   Returns: void
 */
ENTRY(pl011_putchar_asm)
    /* Save character */
    mov     x1, x0

    /* Wait for TX FIFO to have space */
    ldr     x0, =PL011_FR
1:  ldr     x0, [x0]
    tst     x0, #PL011_FR_TXFF
    b.ne    1b

    /* Write character to Data Register */
    ldr     x0, =PL011_DR
    str     x1, [x0]

    /* If newline, also send carriage return */
    cmp     x1, #'\n'
    b.ne    2f
    mov     x1, #'\r'
    b       1b
2:  ret
END(pl011_putchar_asm)

/*
 * pl011_puts_asm — Write a null-terminated string to PL011 UART
 *   x0 = pointer to string
 *   Clobbers: x0, x1, x2
 *   Returns: void
 */
ENTRY(pl011_puts_asm)
    mov     x2, x0                 /* x2 = string pointer */
1:  ldrb    w1, [x2], #1          /* Load next char, advance */
    cbz     w1, 2f                /* End of string? */
    mov     x0, x1
    bl      pl011_putchar_asm     /* Print character */
    b       1b
2:  ret
END(pl011_puts_asm)

/*
 * pl011_puthex_asm — Write a 64-bit value as hex to PL011 UART
 *   x0 = 64-bit value
 *   Clobbers: x0, x1, x2, x3
 *   Returns: void
 */
ENTRY(pl011_puthex_asm)
    mov     x3, x0                 /* x3 = value to print */

    /* Print '0x' prefix */
    mov     x0, #'0'
    bl      pl011_putchar_asm
    mov     x0, #'x'
    bl      pl011_putchar_asm

    /* Print all 16 hex digits */
    mov     x2, #16                /* x2 = nibble counter */
1:  lsl     x1, x3, #60           /* Get MS nibble */
    lsr     x1, x1, #60
    cmp     x1, #9
    b.le    2f
    add     x1, x1, #('A' - 10)
    b       3f
2:  add     x1, x1, #'0'
3:  mov     x0, x1
    bl      pl011_putchar_asm
    lsl     x3, x3, #4            /* Next nibble */
    subs    x2, x2, #1
    b.gt    1b
    ret
END(pl011_puthex_asm)

/*
 * pl011_newline_asm — Write CR+LF to PL011 UART
 *   Clobbers: x0, x1
 */
ENTRY(pl011_newline_asm)
    mov     x0, #'\n'
    bl      pl011_putchar_asm     /* pl011_putchar_asm handles \n→\r\n */
    ret
END(pl011_newline_asm)

/* =========================================================================
 * Diagnostic strings (in .unpaged.rodata)
 * ========================================================================= */

.section .unpaged.rodata, "a", %progbits

str_kernel_panic:
    .asciz  "\r\n=== KERNEL PANIC: Exception in EL1 ===\r\n"
str_esr:
    .asciz  "ESR_EL1: "
str_far:
    .asciz  "FAR_EL1: "
str_elr:
    .asciz  "ELR_EL1: "
str_halt:
    .asciz  "SYSTEM HALTED"

/* =========================================================================
 * Kernel stacks
 *
 * Each CPU has a dedicated kernel stack (SP_EL1) used for exception
 * handling and process scheduling. The stacks are allocated in the
 * unpaged data section (identity-mapped) since they're used during
 * context switching before the high kernel mapping is available.
 *
 * Stack layout per CPU:
 *   k_stacks_start + 2 * K_STACK_SIZE * cpu  → stack area
 *   Each stack: K_STACK_SIZE for data, K_STACK_SIZE as guard page
 *
 * The current process pointer is stored at the top of the kernel stack
 * (at a negative offset from the stack pointer).
 * ========================================================================= */

.section .unpaged.data, "aw", %progbits

/* Magic number — must be first data entry */
.short  0x526F

.balign 4
k_initial_stack:
.space  K_STACK_SIZE
LABEL(__k_unpaged_k_initial_stktop)

/*
 * Boot stack — used during early kernel initialization before
 * per-CPU stacks are set up.
 */
k_boot_stack:
.space  K_STACK_SIZE
LABEL(k_boot_stktop)

/* Per-CPU kernel stacks */
.balign K_STACK_SIZE
LABEL(k_stacks_start)

.space  2 * (K_STACK_SIZE * CONFIG_MAX_CPUS)
LABEL(k_stacks_end)

/* =========================================================================
 * End of mpx.S
 * ========================================================================= */
