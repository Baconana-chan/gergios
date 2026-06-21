/* ============================================================
 * vectors.S — ARM64 Exception Vector Table (VBAR_EL1)
 *
 * Defines the ARM64 exception vector table for the GergiOS kernel.
 * The vector table must be 2KB-aligned and contains 16 entries,
 * each 128 bytes wide.
 *
 * Vector layout (VBAR_EL1 + offset):
 *   Offset | Exception Type       | Description
 *   -------|----------------------|--------------------------------
 *   0x000  | EL1t Sync            | Sync, same EL, using SP_EL0
 *   0x080  | EL1t IRQ             | IRQ, same EL, using SP_EL0
 *   0x100  | EL1t FIQ             | FIQ, same EL, using SP_EL0
 *   0x180  | EL1t SError          | SError, same EL, using SP_EL0
 *   0x200  | EL1h Sync            | Sync, same EL, using SP_EL1  ← Main exception handler
 *   0x280  | EL1h IRQ             | IRQ, same EL, using SP_EL1   ← IRQ handler
 *   0x300  | EL1h FIQ             | FIQ, same EL, using SP_EL1
 *   0x380  | EL1h SError          | SError, same EL, using SP_EL1
 *   0x400  | EL0 Sync (AArch64)   | SVC from AArch64 user         ← Syscall entry
 *   0x480  | EL0 IRQ (AArch64)    | IRQ from AArch64 user
 *   0x500  | EL0 FIQ (AArch64)    | FIQ from AArch64 user
 *   0x580  | EL0 SError (AArch64) | SError from AArch64 user
 *   0x600  | EL0 Sync (AArch32)   | SVC from AArch32 user (unsupported)
 *   0x680  | EL0 IRQ (AArch32)    | IRQ from AArch32 user (unsupported)
 *   0x700  | EL0 FIQ (AArch32)    | FIQ from AArch32 user (unsupported)
 *   0x780  | EL0 SError (AArch32) | SError from AArch32 user (unsupported)
 *
 * Phase 2 status:
 *   - All vectors are installed (VBAR_EL1)
 *   - sync_el1h: Prints ESR/FAR info and halts
 *   - irq_el1h: Prints IRQ received and halts
 *   - sync_el0: Prints syscall info and halts
 *   - All other vectors: Print vector info and halt
 *   - Phase 4+ will add real GIC/timer/syscall dispatching
 *
 * Reference: ARM Architecture Reference Manual ARMv8-A (DDI 0487),
 *            Chapter D1.10 "Exception vectors"
 * ============================================================ */

#include <machine/asm.h>
#include <machine/vm.h>

/* Exception vectors and handlers must be identity-mapped (unpaged).
 * Override _TEXT_SECTION so ENTRY() uses .unpaged.text instead of .text. */
#undef _TEXT_SECTION
#define _TEXT_SECTION .section .unpaged.text, "ax"

/* =========================================================================
 * Constants
 * ========================================================================= */

/* ESR_EL1 fields */
#define ESR_EC_SHIFT           26
#define ESR_EC_MASK            (0x3F << 26)   /* Bits [31:26] */
#define ESR_IL                 (1 << 25)      /* Instruction Length */
#define ESR_ISS_MASK           0x00FFFFFF     /* Instruction Specific Syndrome */

/* Exception Class (EC) values */
#define EC_SVC64               (0x15 << 26)   /* SVC from AArch64 */
#define EC_SVC32               (0x11 << 26)   /* SVC from AArch32 */
#define EC_IABORT_EL0          (0x20 << 26)   /* Instruction Abort from EL0 */
#define EC_IABORT_EL1          (0x21 << 26)   /* Instruction Abort from EL1 */
#define EC_DABORT_EL0          (0x24 << 26)   /* Data Abort from EL0 */
#define EC_DABORT_EL1          (0x25 << 26)   /* Data Abort from EL1 */
#define EC_SP_ALIGN            (0x26 << 26)   /* SP Alignment Fault */
#define EC_FP_EXCEPTION        (0x2C << 26)   /* Floating-point Exception */
#define EC_SERROR              (0x2F << 26)   /* SError */
#define EC_BREAKPOINT_EL0      (0x30 << 26)   /* Breakpoint from EL0 */
#define EC_BREAKPOINT_EL1      (0x31 << 26)   /* Breakpoint from EL1 */

/* Data Abort ISS fields */
#define DA_ISS_WnR             (1 << 6)       /* Write not Read */
#define DA_ISS_FSC_SHIFT       0
#define DA_ISS_FSC_MASK        0x3F           /* Fault Status Code */

/* SPSR_EL1 fields */
#define SPSR_M_MASK            0x0F           /* Mode field [3:0] */
#define SPSR_M_EL0t            0x00
#define SPSR_M_EL1t            0x04
#define SPSR_M_EL1h            0x05

/* =========================================================================
 * Vector Table
 *
 * The VBAR_EL1 register points to the base of this table.
 * The table must be 2KB-aligned.
 * Each entry is 128 bytes (0x80).
 * ========================================================================= */

.section .unpaged.text, "ax"
.balign 0x800

ENTRY(arm64_vector_table)

    /* ----------------------------------------------------------
     * 0x000: EL1t Sync — Synchronous exception using SP_EL0
     * This should never happen in normal operation (kernel uses SP_EL1).
     * ---------------------------------------------------------- */
    b       arm64_sync_el1t_handler
    .space  124            /* Pad to 128 bytes */

    /* ----------------------------------------------------------
     * 0x080: EL1t IRQ — Interrupt using SP_EL0
     * ---------------------------------------------------------- */
    b       arm64_irq_el1t_handler
    .space  128 - 4

    /* ----------------------------------------------------------
     * 0x100: EL1t FIQ — Fast Interrupt using SP_EL0
     * ---------------------------------------------------------- */
    b       arm64_fiq_el1t_handler
    .space  128 - 4

    /* ----------------------------------------------------------
     * 0x180: EL1t SError — System Error using SP_EL0
     * ---------------------------------------------------------- */
    b       arm64_serror_el1t_handler
    .space  128 - 4

    /* ----------------------------------------------------------
     * 0x200: EL1h Sync — Synchronous exception using SP_EL1
     * ======== MAIN EXCEPTION HANDLER ========
     * Handles: page faults, alignment faults, undefined instructions,
     *          breakpoints, floating-point exceptions from kernel.
     * ---------------------------------------------------------- */
    b       arm64_sync_el1h_handler
    .space  128 - 4

    /* ----------------------------------------------------------
     * 0x280: EL1h IRQ — Interrupt using SP_EL1
     * ======== KERNEL IRQ HANDLER ========
     * Handles hardware interrupts (GIC, timer) while in kernel mode.
     * ---------------------------------------------------------- */
    b       arm64_irq_el1h_handler
    .space  128 - 4

    /* ----------------------------------------------------------
     * 0x300: EL1h FIQ — Fast Interrupt using SP_EL1
     * ---------------------------------------------------------- */
    b       arm64_fiq_el1h_handler
    .space  128 - 4

    /* ----------------------------------------------------------
     * 0x380: EL1h SError — System Error using SP_EL1
     * ---------------------------------------------------------- */
    b       arm64_serror_el1h_handler
    .space  128 - 4

    /* ----------------------------------------------------------
     * 0x400: EL0 Sync (AArch64) — SVC from 64-bit userspace
     * ======== SYSCALL ENTRY ========
     * Handles system calls from 64-bit user processes.
     * ---------------------------------------------------------- */
    b       arm64_sync_el0_handler
    .space  128 - 4

    /* ----------------------------------------------------------
     * 0x480: EL0 IRQ (AArch64) — Interrupt from userspace
     * ---------------------------------------------------------- */
    b       arm64_irq_el0_handler
    .space  128 - 4

    /* ----------------------------------------------------------
     * 0x500: EL0 FIQ (AArch64) — Fast Interrupt from userspace
     * ---------------------------------------------------------- */
    b       arm64_fiq_el0_handler
    .space  128 - 4

    /* ----------------------------------------------------------
     * 0x580: EL0 SError (AArch64) — System Error from userspace
     * ---------------------------------------------------------- */
    b       arm64_serror_el0_handler
    .space  128 - 4

    /* ----------------------------------------------------------
     * 0x600-0x780: AArch32 exception vectors
     * MINIX for ARM64 does not support 32-bit ARM compatibility mode.
     * All four AArch32 vectors branch to a common stub.
     * ---------------------------------------------------------- */
    b       arm64_aarch32_stub
    .space  128 - 4

    b       arm64_aarch32_stub
    .space  128 - 4

    b       arm64_aarch32_stub
    .space  128 - 4

    b       arm64_aarch32_stub
    .space  128 - 4

END(arm64_vector_table)

/* =========================================================================
 * Exception handler declarations (exported for head.S)
 * ========================================================================= */

.global arm64_vector_table

/* =========================================================================
 * PL011 UART Constants (QEMU virt)
 *
 * PL011 is at physical address 0x09000000 on QEMU virt.
 * Since we're identity-mapped after MMU enablement, it's accessible
 * directly at this address.
 * ========================================================================= */

#define PL011_BASE              0x09000000
#define PL011_DR                (PL011_BASE + 0x000)  /* Data Register */
#define PL011_FR                (PL011_BASE + 0x018)  /* Flag Register */
#define PL011_FR_TXFF           (1 << 5)              /* TX FIFO Full */
#define PL011_FR_BUSY           (1 << 3)              /* Transmit Busy */

/* =========================================================================
 * PL011 UART diagnostic helpers (inline assembly)
 *
 * These are simple assembly routines that write directly to the PL011
 * UART registers. They're used for diagnostic output during exception
 * handling, before any C code can be safely called.
 * ========================================================================= */

/*
 * pl011_putc: Write a single character to PL011 UART.
 *   x0: character to write (preserved)
 *   Clobbers: x10, x11, x12
 */
.macro pl011_putc, char_reg
    mov     x12, \char_reg     /* Save char in temp register */

    /* Wait for TX FIFO to have space.
     * Load the FR address, read it, check TXFF bit. */
    ldr     x10, =PL011_FR      /* x10 = PL011_FR address (0x09000018) */
1:  ldr     x11, [x10]          /* Read FR register */
    tst     x11, #PL011_FR_TXFF /* Test bit 5 (TX FIFO Full) */
    b.ne    1b                  /* Wait if FIFO full */

    /* Write character to Data Register */
    ldr     x10, =PL011_DR      /* x10 = PL011_DR address (0x09000000) */
    str     x12, [x10]          /* Write the char */

    /* If newline, also send carriage return */
    cmp     x12, #'\n'
    b.ne    2f
    mov     x12, #'\r'          /* Send \r as well */
    b       1b
2:
.endm

/*
 * pl011_puts: Write a null-terminated string to PL011 UART.
 *   x0: pointer to string (clobbered)
 *   Clobbers: x10, x11, x12
 */
.macro pl011_puts, str_ptr
    mov     x12, \str_ptr
1:  ldrb    w10, [x12], #1     /* Load next char, advance pointer */
    cbz     w10, 2f            /* End of string? */
    pl011_putc x10
    b       1b
2:
.endm

/*
 * pl011_puthex: Write a 64-bit value as hexadecimal to PL011 UART.
 *   value_reg: 64-bit value to print (preserved in x12)
 *   Clobbers: x10, x11, x12, x13
 *
 * Always prints 16 hex digits (no leading-zero skip for simplicity).
 */
.macro pl011_puthex, value_reg
    mov     x12, \value_reg    /* Save value */

    /* Print '0x' prefix */
    mov     x10, #'0'
    pl011_putc x10
    mov     x10, #'x'
    pl011_putc x10

    /* Print all 16 nibbles */
    mov     x11, x12            /* x11 = value to print */
    mov     x13, #16            /* 16 nibbles for 64-bit */
1:  lsl     x10, x11, #60      /* Get most significant nibble */
    lsr     x10, x10, #60
    cmp     w10, #9
    b.le    2f
    add     w10, w10, #('A' - 10)
    b       3f
2:  add     w10, w10, #'0'
3:  pl011_putc x10
    lsl     x11, x11, #4       /* Shift to next nibble */
    subs    x13, x13, #1
    b.gt    1b                  /* Loop for all 16 nibbles */
.endm

/*
 * pl011_newline: Print a newline + carriage return.
 *   Clobbers: x10, x11
 */
.macro pl011_newline
    mov     x10, #'\n'
    pl011_putc x10             /* pl011_putc handles \n → \r\n */
.endm

/* =========================================================================
 * Handler implementations
 *
 * Phase 2: All handlers print diagnostic info via PL011 UART and halt.
 * Phase 4+: Real handlers with GIC, timer, syscall dispatching.
 * ========================================================================= */

/* -----------------------------------------------------------------------
 * arm64_sync_el1h_handler — Main synchronous exception handler (kernel)
 *
 * Entry conditions:
 *   - Exception from EL1 (kernel) using SP_EL1
 *   - ESR_EL1 contains the exception syndrome
 *   - FAR_EL1 contains the fault address (for data/instruction aborts)
 *
 * Phase 2: Dispatch to mpx.S arm64_exception_entry_from_kernel.
 * Phase 4+: Full kernel exception handling.
 * ----------------------------------------------------------------------- */

arm64_sync_el1h_handler:
    /* Some registers may be live (kernel was interrupted).
     * We need to save/restore them around the handler.
     * mpx.S's arm64_exception_entry_from_kernel handles diagnostics. */
    b       arm64_exception_entry_from_kernel

/* -----------------------------------------------------------------------
 * arm64_irq_el1h_handler — IRQ handler (in kernel mode)
 *
 * Phase 2: Dispatch to mpx.S arm64_irq_entry_from_kernel.
 * Phase 4+: Real GIC interrupt handling.
 * ----------------------------------------------------------------------- */

arm64_irq_el1h_handler:
    b       arm64_irq_entry_from_kernel

/* -----------------------------------------------------------------------
 * arm64_sync_el0_handler — Sync exception from EL0 (user mode)
 *
 * This handles ALL synchronous exceptions from userspace:
 *   - SVC: System call (dispatch to arm64_svc_handler in mpx.S)
 *   - Page faults, alignment faults, undefined instructions, etc.
 *     (dispatch to arm64_exception_entry_from_user in mpx.S)
 *
 * Phase 2: Check ESR EC for SVC, dispatch accordingly.
 * Phase 5+: Full syscall dispatching.
 * ----------------------------------------------------------------------- */

arm64_sync_el0_handler:
    /* Read ESR_EL1 to check exception class.
     * EC field is in bits[31:26]. We shift right by 26 to
     * compare with small immediate (ARM64 cmp only takes
     * 12-bit unsigned immediates, and EC values like
     * 0x15 << 26 = 34603008 are too large). */
    mrs     x0, ESR_EL1
    lsr     x0, x0, #26             /* Shift EC to low bits [5:0] */
    cmp     x0, #0x15               /* EC_SVC64 = 0x15 */
    b.eq    arm64_svc_handler

    /* Non-SVC exception from user (page fault, etc.) */
    b       arm64_exception_entry_from_user

/* -----------------------------------------------------------------------
 * Unused vector stubs
 *
 * All remaining vectors print a diagnostic message and halt.
 * ----------------------------------------------------------------------- */

arm64_sync_el1t_handler:
    adr     x0, str_sync_el1t
    b       arm64_stub_with_msg

arm64_irq_el1t_handler:
    adr     x0, str_irq_el1t
    b       arm64_stub_with_msg

arm64_fiq_el1t_handler:
arm64_fiq_el1h_handler:
    adr     x0, str_fiq
    b       arm64_stub_with_msg

arm64_serror_el1t_handler:
arm64_serror_el1h_handler:
    adr     x0, str_serror
    b       arm64_stub_with_msg

arm64_irq_el0_handler:
    /* IRQ from EL0 — dispatch to mpx.S arm64_irq_entry_from_user */
    b       arm64_irq_entry_from_user

arm64_fiq_el0_handler:
    /* FIQ is uncommon, treat as vector 8 */
    adr     x0, str_fiq
    b       arm64_stub_with_msg

arm64_serror_el0_handler:
    adr     x0, str_serror
    b       arm64_stub_with_msg

arm64_aarch32_stub:
    adr     x0, str_aarch32

arm64_stub_with_msg:
    stp     x19, x20, [sp, #-16]!
    mov     x19, x0             /* Save message pointer */
    mrs     x20, ESR_EL1

    mov     x0, x19
    pl011_puts x0
    pl011_newline

    adr     x0, str_esr
    pl011_puts x0
    mov     x0, x20
    pl011_puthex x0
    pl011_newline

    ldp     x19, x20, [sp], #16

arm64_exception_halt:
    /* Disable all exceptions and halt */
    msr     DAIFSet, #0xF
1:  wfi
    b       1b

/* =========================================================================
 * Diagnostic strings (in .unpaged.rodata for identity-map accessibility)
 * ========================================================================= */

.section .unpaged.rodata, "a", %progbits

str_sync_el1t:
    .asciz  "\r\n=== UNEXPECTED: EL1t Sync ==="
str_irq_el1t:
    .asciz  "\r\n=== UNEXPECTED: EL1t IRQ ==="
str_fiq:
    .asciz  "\r\n=== FIQ ==="
str_serror:
    .asciz  "\r\n=== SError ==="
str_irq_user:
    .asciz  "\r\n=== USER IRQ (EL0 IRQ) ==="
str_aarch32:
    .asciz  "\r\n=== AArch32 mode not supported ==="
str_esr:
    .asciz  "ESR_EL1: "
str_far:
    .asciz  "FAR_EL1: "
str_elr:
    .asciz  "ELR_EL1: "
str_spsr:
    .asciz  "SPSR_EL1: "
str_sysno:
    .asciz  "SVC # (x8): "
str_halt:
    .asciz  "SYSTEM HALTED"

/* =========================================================================
 * End of vectors.S
 * ========================================================================= */
