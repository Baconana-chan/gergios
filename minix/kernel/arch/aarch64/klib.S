/* ============================================================
 * klib.S — ARM64 Basic Architecture Functions
 *
 * This file contains the lowest-level architecture functions
 * for the GergiOS kernel on ARM64. It provides:
 *
 *   1. Message copy (user <-> kernel) with fault recovery
 *   2. Physical memory copy/memset with fault recovery
 *   3. Virtual-to-physical address translation (AT instruction)
 *   4. Address space switching (TTBR0_EL1)
 *   5. System control (reset, halt, interrupts)
 *   6. System register access (TTBR, ESR, FAR, ELR, SPSR)
 *   7. TLB maintenance (TLBI operations)
 *   8. Memory barriers (DMB, DSB)
 *   9. Kernel stack switching
 *  10. Per-CPU pointer access (tpidr_el1)
 *
 * ARM64 AAPCS64 calling convention:
 *   x0-x7: argument/scratch registers
 *   x0:    return value
 *   x19-x28: callee-saved
 *   x29:   frame pointer (callee-saved)
 *   x30:   link register (callee-saved)
 *   sp:    stack pointer (must be 16-byte aligned)
 *
 * All functions in this file are placed in .unpaged.text
 * because they may be called during early boot before the
 * high kernel mapping is fully active.
 *
 * Reference: ARM Architecture Reference Manual ARMv8-A (DDI 0487)
 * ============================================================ */

#include <minix/config.h>
#include <minix/const.h>
#include <machine/asm.h>
#include <machine/interrupt.h>
#include <machine/vm.h>
#include "archconst.h"
#include "kernel/const.h"
#include "sconst.h"
#include <machine/multiboot.h>

/* =========================================================================
 * SECTION: .unpaged.text
 *
 * All functions in this file must be identity-mapped because they
 * are called during early boot and at exception handling time
 * when only the identity map is guaranteed to be active.
 * ========================================================================= */

.section .unpaged.text, "ax"

/* =========================================================================
 * 1. MESSAGE COPY (user <-> kernel)
 *
 * copy_msg_from_user and copy_msg_to_user copy a 64-byte MINIX message
 * between kernel space and user process space. They are designed to
 * safely handle invalid user pointers: if a page fault occurs during
 * the copy, the exception handler redirects execution to
 * __user_copy_msg_pointer_failure, which returns -1 to the caller.
 *
 * The ARM64 exception system allows us to detect the exact instruction
 * that faulted by checking ELR_EL1. The fault handlers in vectors.S
 * check if the faulting address falls within the copy_msg functions
 * and redirect to the failure handler if so.
 *
 * ARM64 calling convention (AAPCS64):
 *   copy_msg_from_user(user_mbuf, dst)  → x0=user_mbuf, x1=dst
 *   copy_msg_to_user(src, user_mbuf)    → x0=src, x1=user_mbuf
 *
 * A message is 64 bytes = 8 qwords of 8 bytes each.
 * ========================================================================= */

/* -----------------------------------------------------------------------
 * copy_msg_from_user — Copy 64-byte message from user to kernel
 *
 * int copy_msg_from_user(message *user_mbuf, message *dst)
 *   x0 = user_mbuf (source in user address space)
 *   x1 = dst (destination in kernel address space)
 * Returns: 0 on success, -1 on fault
 * ----------------------------------------------------------------------- */
ENTRY(copy_msg_from_user)
	/* Save callee-saved registers */
	stp	x19, x20, [sp, #-16]!
	stp	x21, x22, [sp, #-16]!
	stp	x23, x24, [sp, #-16]!
	stp	x25, x26, [sp, #-16]!
	stp	x27, x28, [sp, #-16]!

	/* Save source and destination pointers in callee-saved regs */
	mov	x19, x0			/* x19 = user source */
	mov	x20, x1			/* x20 = kernel dest */

	/* Copy 64 bytes = 8 qwords */
	ldp	x0, x1, [x19, #0]
	stp	x0, x1, [x20, #0]
	ldp	x2, x3, [x19, #16]
	stp	x2, x3, [x20, #16]
	ldp	x4, x5, [x19, #32]
	stp	x4, x5, [x20, #32]
	ldp	x6, x7, [x19, #48]
	stp	x6, x7, [x20, #48]

LABEL(__copy_msg_from_user_end)
	/* Success: restore callee-saved registers and return 0 */
	ldp	x27, x28, [sp], #16
	ldp	x25, x26, [sp], #16
	ldp	x23, x24, [sp], #16
	ldp	x21, x22, [sp], #16
	ldp	x19, x20, [sp], #16
	mov	x0, #0
	ret
END(copy_msg_from_user)

/* -----------------------------------------------------------------------
 * copy_msg_to_user — Copy 64-byte message from kernel to user
 *
 * void copy_msg_to_user(message *src, message *user_mbuf)
 *   x0 = src (source in kernel address space)
 *   x1 = user_mbuf (destination in user address space)
 * Returns: 0 on success, -1 on fault
 * ----------------------------------------------------------------------- */
ENTRY(copy_msg_to_user)
	/* Save callee-saved registers */
	stp	x19, x20, [sp, #-16]!
	stp	x21, x22, [sp, #-16]!
	stp	x23, x24, [sp, #-16]!
	stp	x25, x26, [sp, #-16]!
	stp	x27, x28, [sp, #-16]!

	/* Save source and destination pointers */
	mov	x19, x0			/* x19 = kernel source */
	mov	x20, x1			/* x20 = user dest */

	/* Copy 64 bytes = 8 qwords */
	ldp	x0, x1, [x19, #0]
	stp	x0, x1, [x20, #0]
	ldp	x2, x3, [x19, #16]
	stp	x2, x3, [x20, #16]
	ldp	x4, x5, [x19, #32]
	stp	x4, x5, [x20, #32]
	ldp	x6, x7, [x19, #48]
	stp	x6, x7, [x20, #48]

LABEL(__copy_msg_to_user_end)
	/* Success: restore callee-saved registers and return 0 */
	ldp	x27, x28, [sp], #16
	ldp	x25, x26, [sp], #16
	ldp	x23, x24, [sp], #16
	ldp	x21, x22, [sp], #16
	ldp	x19, x20, [sp], #16
	mov	x0, #0
	ret
END(copy_msg_to_user)

/* -----------------------------------------------------------------------
 * __user_copy_msg_pointer_failure — Fault recovery for message copy
 *
 * When a page fault occurs during copy_msg_from_user() or
 * copy_msg_to_user(), the exception handler vectors here instead
 * of the normal return point. This function restores the callee-saved
 * registers and returns -1 to indicate the fault.
 *
 * Returns: -1 (always, indicating a bad user pointer)
 * ----------------------------------------------------------------------- */
ENTRY(__user_copy_msg_pointer_failure)
	/* Restore callee-saved registers (same order as saved) */
	ldp	x27, x28, [sp], #16
	ldp	x25, x26, [sp], #16
	ldp	x23, x24, [sp], #16
	ldp	x21, x22, [sp], #16
	ldp	x19, x20, [sp], #16
	mov	x0, #-1			/* Return -1 on fault */
	ret
END(__user_copy_msg_pointer_failure)

/* =========================================================================
 * 2. PHYSICAL MEMORY COPY
 *
 * phys_copy — Copy a block of physical memory.
 *
 * phys_bytes phys_copy(phys_bytes source, phys_bytes destination,
 *                      phys_bytes bytecount)
 *   x0 = source physical address
 *   x1 = destination physical address
 *   x2 = byte count
 * Returns: 0 on success, fault address on fault
 *
 * ARM64: physical addresses are accessed directly via the identity map
 * (which covers 0-2GB) or via the kernel high mapping. For Phase 2,
 * we use the identity map (physical address == virtual address).
 *
 * The copy uses 16-byte pairs (LDP/STP) for maximum throughput.
 * On page fault, execution continues at phys_copy_fault_in_kernel
 * which returns the fault address (FAR_EL1) for recovery.
 * ----------------------------------------------------------------------- */
ENTRY(phys_copy)
	/* Save callee-saved registers */
	stp	x19, x20, [sp, #-16]!
	stp	x21, x22, [sp, #-16]!
	stp	x23, x24, [sp, #-16]!

	/* Save arguments in callee-saved regs */
	mov	x19, x0			/* x19 = source */
	mov	x20, x1			/* x20 = dest */
	mov	x21, x2			/* x21 = count */

	/* For small copies (< 32 bytes), skip alignment */
	cmp	x21, #32
	b.lo	pc_small

	/* Align source to 16-byte boundary */
	neg	x22, x19
	ands	x22, x22, #15		/* x22 = misalignment (0-15) */
	b.eq	pc_aligned

	/* Copy misaligned bytes at the start */
	sub	x21, x21, x22		/* Reduce count */
1:	subs	x22, x22, #1
	b.lo	pc_aligned
	ldrb	w23, [x19], #1
	strb	w23, [x20], #1
	b	1b

pc_aligned:
	/* Copy 16-byte chunks */
	mov	x22, x21
	lsr	x22, x22, #4		/* Number of 16-byte chunks */
	lsl	x22, x22, #4		/* Round down to 16 */
	sub	x21, x21, x22		/* Remainder */
	cbz	x22, pc_small

	/* Main copy loop: 16 bytes per iteration */
2:	ldp	x23, x24, [x19], #16
	stp	x23, x24, [x20], #16
	subs	x22, x22, #16
	b.gt	2b

pc_small:
	/* Copy remaining bytes */
	cbz	x21, 4f
3:	ldrb	w22, [x19], #1
	strb	w22, [x20], #1
	subs	x21, x21, #1
	b.gt	3b

4:	/* Success: return 0 */
	mov	x0, #0
	ldp	x23, x24, [sp], #16
	ldp	x21, x22, [sp], #16
	ldp	x19, x20, [sp], #16
	ret

LABEL(phys_copy_fault)
	/* Fault occurred — return fault address from FAR_EL1.
	 * The fault handler redirects here if the faulting PC
	 * is within the phys_copy range. */
	mrs	x0, FAR_EL1
	ldp	x23, x24, [sp], #16
	ldp	x21, x22, [sp], #16
	ldp	x19, x20, [sp], #16
	ret

LABEL(phys_copy_fault_in_kernel)
	/* Fault from kernel context — return fault address */
	mrs	x0, FAR_EL1
	ldp	x23, x24, [sp], #16
	ldp	x21, x22, [sp], #16
	ldp	x19, x20, [sp], #16
	ret
END(phys_copy)

/* =========================================================================
 * 3. PHYSICAL MEMORY SET
 *
 * phys_memset — Fill a block of physical memory with a pattern.
 *
 * phys_bytes phys_memset(phys_bytes dst, unsigned long pattern,
 *                        phys_bytes bytecount)
 *   x0 = destination physical address
 *   x1 = pattern value (byte, but passed as unsigned long)
 *   x2 = byte count
 * Returns: 0 on success, fault address on fault
 * ----------------------------------------------------------------------- */
ENTRY(phys_memset)
	/* Save callee-saved registers */
	stp	x19, x20, [sp, #-16]!
	stp	x21, x22, [sp, #-16]!

	mov	x19, x0			/* x19 = dest */
	mov	x20, x1			/* x20 = pattern */
	mov	x21, x2			/* x21 = count */

	/* Broadcast the pattern byte to fill a qword */
	and	x20, x20, #0xFF		/* Keep only lowest byte */
	mov	x22, #0x0101010101010101
	mul	x20, x20, x22		/* Broadcast to all 8 bytes */

	/* Fill using 8-byte stores */
	mov	x22, x21
	lsr	x22, x22, #3		/* Number of qwords */
	lsl	x22, x22, #3		/* Round down to 8 */
	sub	x21, x21, x22		/* Remainder */
	cbz	x22, ms_remain

1:	str	x20, [x19], #8
	subs	x22, x22, #8
	b.gt	1b

ms_remain:
	/* Fill remaining bytes */
	cbz	x21, 3f
2:	strb	w20, [x19], #1
	subs	x21, x21, #1
	b.gt	2b

3:	/* Success */
	mov	x0, #0
	ldp	x21, x22, [sp], #16
	ldp	x19, x20, [sp], #16
	ret

LABEL(memset_fault)
	mrs	x0, FAR_EL1
	ldp	x21, x22, [sp], #16
	ldp	x19, x20, [sp], #16
	ret

LABEL(memset_fault_in_kernel)
	mrs	x0, FAR_EL1
	ldp	x21, x22, [sp], #16
	ldp	x19, x20, [sp], #16
	ret
END(phys_memset)

/* =========================================================================
 * 4. VIRTUAL-TO-PHYSICAL ADDRESS TRANSLATION
 *
 * vir2phys — Translate a virtual address to physical using AT instruction.
 *
 * phys_bytes vir2phys(void *vir_addr)
 *   x0 = virtual address to translate
 * Returns: physical address, or ~0 on failure
 *
 * Uses the AT S1E1R (Address Translation, Stage 1, EL1, Read) instruction
 * to perform a page table walk and return the physical address.
 * The result is in PAR_EL1:
 *   [47:12] = physical address frame
 *   [0]     = FAULT (0 = success, 1 = fault)
 * ----------------------------------------------------------------------- */
ENTRY(vir2phys)
	/* Save original VA. PAR_EL1 returns the physical page frame
	 * in bits [47:12], but bits [11:0] contain memory attributes,
	 * NOT the page offset. We must combine PAR_EL1's frame with
	 * the original VA's page offset. */
	mov	x1, x0			/* x1 = original virtual address */

	/* Execute AT instruction for the given virtual address */
	ats1e1r	x1			/* Translate VA using current page tables */

	/* Synchronization barrier */
	isb

	/* Read PAR_EL1 (Physical Address Register) */
	mrs	x2, PAR_EL1

	/* Check for translation fault (bit 0 = FAULT) */
	tbnz	x2, #0, 1f

	/* Success: combine PAR_EL1 frame with original VA page offset.
	 * PAR_EL1[47:12] = physical page frame
	 *   x1[11:0]    = page offset from original VA
	 * Result: x0 = (PAR_EL1[47:12] << 12) | (VA[11:0]) */
	and	x1, x1, #0xFFF		/* Page offset from original VA */
	and	x2, x2, #0xFFFFFFFFFFF000 /* Physical frame from PAR_EL1 */
	orr	x0, x1, x2		/* Combine */
	ret

1:	/* Fault: return ~0 */
	mov	x0, #-1
	ret
END(vir2phys)

/* =========================================================================
 * 5. ADDRESS SPACE SWITCHING
 *
 * __switch_address_space — Switch TTBR0_EL1 for a process.
 *
 * void __switch_address_space(struct proc *p, struct proc **__ptproc)
 *   x0 = proc pointer (struct proc *p)
 *   x1 = pointer to ptproc pointer (struct proc **__ptproc)
 *
 * Loads the process's TTBR0_EL1 value (page table base) into the
 * TTBR0_EL1 system register. Avoids unnecessary TLB flushes by
 * only switching if the value is different from the current one.
 * ----------------------------------------------------------------------- */
ENTRY(__switch_address_space)
	/* Save callee-saved registers */
	stp	x19, x20, [sp, #-16]!

	mov	x19, x0			/* x19 = proc pointer */
	mov	x20, x1			/* x20 = ptproc pointer */

	/* Load the TTBR0_EL1 value from proc->p_seg.ttbr0_el1.
	 * P_TTBR is defined in procoffsets.h */
	ldr	x0, [x19, #P_TTBR]

	/* If TTBR is 0, no address space to switch */
	cbz	x0, 1f

	/* Read current TTBR0_EL1 */
	mrs	x1, ttbr0_el1

	/* Compare — skip if same (avoids TLB flush) */
	cmp	x0, x1
	b.eq	1f

	/* Write new TTBR0_EL1 (this flushes all TLB entries
	 * associated with the old TTBR0, but NOT global entries) */
	msr	ttbr0_el1, x0
	isb

	/* Update *ptproc = p to track which process's address space is active */
	str	x19, [x20]

1:	ldp	x19, x20, [sp], #16
	ret
END(__switch_address_space)

/* =========================================================================
 * 6. KERNEL STACK SWITCHING
 *
 * switch_k_stack — Switch kernel stack and jump to continuation.
 *
 * void switch_k_stack(void *sp, void (*continuation)(void))
 *   x0 = new stack pointer
 *   x1 = continuation function address
 *
 * Sets SP to the given value, resets FP, and jumps to the
 * given function. This function does NOT return.
 * ----------------------------------------------------------------------- */
ENTRY(switch_k_stack)
	/* x0 = new stack pointer
	 * x1 = continuation function */
	mov	x2, x0			/* Save new SP */
	mov	x3, x1			/* Save continuation */

	/* Reset frame pointer */
	mov	x29, #0

	/* Switch stack */
	mov	sp, x2

	/* Jump to continuation (tail call) */
	br	x3

	/* NOT REACHED */
0:	wfi
	b	0b
END(switch_k_stack)

/* =========================================================================
 * 7. SYSTEM CONTROL
 *
 * reset — System reset using PSCI.
 *
 * _Noreturn void reset(void)
 *
 * Attempts to reset the system via PSCI (Power State Coordination
 * Interface) SYSTEM_RESET function (function ID 0x84000009).
 * If PSCI is not available, enters an infinite WFI loop.
 * ----------------------------------------------------------------------- */
ENTRY(reset)
	/* PSCI SYSTEM_RESET:
	 * Function ID: 0x84000009 (PSCI 1.0 SYSTEM_RESET)
	 * Calling convention: HVC #0 (or SMC #0)
	 * x0 = function ID
	 *
	 * We use HVC #0 (Hypervisor Call) which works on QEMU virt.
	 * On systems with EL3 firmware, SMC #0 may be needed. */
	mov	x0, #0x84000009		/* PSCI SYSTEM_RESET */
	hvc	#0

	/* If PSCI reset failed, try SMC */
	mov	x0, #0x84000009
	smc	#0

	/* If all else fails, halt */
	msr	DAIFSet, #0xF		/* Mask all exceptions */
1:	wfi
	b	1b
END(reset)

/* -----------------------------------------------------------------------
 * halt_cpu — Halt the current CPU.
 *
 * void halt_cpu(void)
 *
 * Masks all interrupts and enters WFI (Wait For Interrupt) loop.
 * ----------------------------------------------------------------------- */
ENTRY(halt_cpu)
	msr	DAIFSet, #0xF		/* Mask all exceptions */
	isb
1:	wfi
	b	1b
END(halt_cpu)

/* -----------------------------------------------------------------------
 * intr_enable / interrupts_enable — Enable interrupts (PSTATE.I).
 *
 * void intr_enable(void)
 * void interrupts_enable(void)
 *
 * Clears the I (IRQ mask) bit in DAIF to enable interrupts.
 * ----------------------------------------------------------------------- */
ENTRY(intr_enable)
ENTRY(interrupts_enable)
	msr	DAIFClr, #2		/* Clear I bit (IRQ enable) */
	dsb	sy
	isb
	ret
END(intr_enable)
END(interrupts_enable)

/* -----------------------------------------------------------------------
 * intr_disable / interrupts_disable — Disable interrupts (PSTATE.I).
 *
 * void intr_disable(void)
 * void interrupts_disable(void)
 *
 * Sets the I (IRQ mask) bit in DAIF to disable interrupts.
 * ----------------------------------------------------------------------- */
ENTRY(intr_disable)
ENTRY(interrupts_disable)
	msr	DAIFSet, #2		/* Set I bit (IRQ disable) */
	dsb	sy
	isb
	ret
END(intr_disable)
END(interrupts_disable)

/* =========================================================================
 * 8. SYSTEM REGISTER ACCESS
 *
 * Read/write functions for ARM64 system registers.
 * These provide C-callable wrappers around MRS/MSR instructions.
 * ========================================================================= */

/* -----------------------------------------------------------------------
 * read_ttbr0 / read_ttbr1 / write_ttbr0 — Page table base access
 * ----------------------------------------------------------------------- */
ENTRY(read_ttbr0)
	mrs	x0, ttbr0_el1
	ret
END(read_ttbr0)

ENTRY(read_ttbr1)
	mrs	x0, ttbr1_el1
	ret
END(read_ttbr1)

ENTRY(write_ttbr0)
	msr	ttbr0_el1, x0
	isb
	ret
END(write_ttbr0)

/* -----------------------------------------------------------------------
 * read_esr — Read ESR_EL1 (Exception Syndrome Register)
 * read_far — Read FAR_EL1 (Fault Address Register)
 * read_elr — Read ELR_EL1 (Exception Link Register)
 * read_spsr — Read SPSR_EL1 (Saved Program Status Register)
 * ----------------------------------------------------------------------- */
ENTRY(read_esr)
	mrs	x0, esr_el1
	ret
END(read_esr)

ENTRY(read_far)
	mrs	x0, far_el1
	ret
END(read_far)

ENTRY(read_elr)
	mrs	x0, elr_el1
	ret
END(read_elr)

ENTRY(read_spsr)
	mrs	x0, spsr_el1
	ret
END(read_spsr)

/* -----------------------------------------------------------------------
 * read_tpidr_el1 / write_tpidr_el1 — Per-CPU pointer access
 *
 * tpidr_el1 (Thread Pointer ID Register) is used to store a pointer
 * to the current CPU's local data area (per-CPU variables).
 * ----------------------------------------------------------------------- */
ENTRY(read_tpidr_el1)
	mrs	x0, tpidr_el1
	ret
END(read_tpidr_el1)

ENTRY(write_tpidr_el1)
	msr	tpidr_el1, x0
	isb
	ret
END(write_tpidr_el1)

/* =========================================================================
 * 9. TLB MAINTENANCE
 *
 * ARM64 TLB invalidation operations. These must be followed by DSB+ISB
 * to ensure the invalidation is complete before continuing.
 * ========================================================================= */

/* -----------------------------------------------------------------------
 * tlb_flush_all — Invalidate all TLB entries (EL1 and EL0).
 *
 * void tlb_flush_all(void)
 * ----------------------------------------------------------------------- */
ENTRY(tlb_flush_all)
	/* Invalidate all EL1&EL0 TLB entries (non-global) */
	tlbi	vmalle1
	dsb	sy
	isb
	ret
END(tlb_flush_all)

/* -----------------------------------------------------------------------
 * tlb_flush_addr — Invalidate TLB entry for a specific virtual address.
 *
 * void tlb_flush_addr(uint64_t addr)
 *   x0 = virtual address to invalidate
 *
 * Uses TLBI VAE1 (VA, EL1) to invalidate a single TLB entry.
 * ----------------------------------------------------------------------- */
ENTRY(tlb_flush_addr)
	/* Invalidate TLB entry for the given VA at EL1 */
	tlbi	vae1, x0
	dsb	sy
	isb
	ret
END(tlb_flush_addr)

/* =========================================================================
 * 10. MEMORY BARRIERS
 *
 * ARM64 memory barrier instructions for ordering memory accesses.
 * ========================================================================= */

/* -----------------------------------------------------------------------
 * dmb_sy — Data Memory Barrier (full system)
 *
 * void dmb_sy(void)
 *
 * Ensures all explicit memory accesses before the DMB are observed
 * before any explicit memory accesses after the DMB.
 * ----------------------------------------------------------------------- */
ENTRY(dmb_sy)
	dmb	sy
	ret
END(dmb_sy)

/* -----------------------------------------------------------------------
 * dsb_sy — Data Synchronization Barrier (full system)
 *
 * void dsb_sy(void)
 *
 * Ensures all instructions before the DSB complete before any
 * instruction after the DSB is executed.
 * ----------------------------------------------------------------------- */
ENTRY(dsb_sy)
	dsb	sy
	isb
	ret
END(dsb_sy)

/* =========================================================================
 * 11. MISC FUNCTIONS
 * ========================================================================= */

/* -----------------------------------------------------------------------
 * read_current_sp — Read the current stack pointer.
 *
 * uint64_t read_current_sp(void)
 * ----------------------------------------------------------------------- */
ENTRY(read_current_sp)
	mov	x0, sp
	ret
END(read_current_sp)

/* -----------------------------------------------------------------------
 * arch_pause — Hint that the CPU is in a spin-wait loop.
 *
 * void arch_pause(void)
 *
 * On ARM64, the YIELD instruction hints that the current thread
 * is in a spin-wait loop, allowing the CPU to optimize (e.g.,
 * in SMT implementations, switch to another thread).
 * ----------------------------------------------------------------------- */
ENTRY(arch_pause)
	yield
	ret
END(arch_pause)

/* =========================================================================
 * End of klib.S
 * ========================================================================= */
