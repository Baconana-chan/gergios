/* $NetBSD$ */

/*-
 * Copyright (c) 2025 MINIX 3 Contributors
 *
 * Minimal C library (libminc) AArch64 longjmp.
 *
 * Restores the callee-saved registers per AAPCS64:
 *   - x19-x29 (11 general-purpose registers)
 *   - x30 (LR)
 *   - SP
 *   - d8-d15 (8 VFP/NEON double-precision registers)
 *
 * Validates the magic number in the jmp_buf before restoring.
 * If validation fails, calls longjmperror() and abort().
 */

#include <machine/asm.h>
#include <machine/setjmp.h>

/*
 * longjmp(jmp_buf env, int val)
 *
 * Restore the register context from env.
 * Returns val (or 1 if val == 0).
 */
ENTRY(longjmp)
	/* Validate magic number */
	adrp	x7, .L_longjmp_magic
	ldr	x7, [x7, #:lo12:.L_longjmp_magic]
	ldr	x2, [x0, #(_JB_MAGIC * 8)]
	cmp	x2, x7
	b.ne	.Lbotch

	/* Validate SP is non-NULL (separate load — _JB_SP is NOT adjacent to magic) */
	ldr	x3, [x0, #(_JB_SP * 8)]
	cbz	x3, .Lbotch

	/* Validate x29 (FP) is non-NULL */
	ldr	x4, [x0, #(_JB_X29 * 8)]
	cbz	x4, .Lbotch

	/* Restore callee-saved GPRs x19-x28 */
	ldp	x19, x20, [x0, #(_JB_X19 * 8)]
	ldp	x21, x22, [x0, #(_JB_X21 * 8)]
	ldp	x23, x24, [x0, #(_JB_X23 * 8)]
	ldp	x25, x26, [x0, #(_JB_X25 * 8)]
	ldp	x27, x28, [x0, #(_JB_X27 * 8)]

	/* Restore x29 (FP) and x30 (LR) */
	ldp	x29, x30, [x0, #(_JB_X29 * 8)]

	/* Restore callee-saved VFP registers d8-d15 */
	ldp	d8,  d9,  [x0, #(_JB_D8 * 8)]
	ldp	d10, d11, [x0, #(_JB_D10 * 8)]
	ldp	d12, d13, [x0, #(_JB_D12 * 8)]
	ldp	d14, d15, [x0, #(_JB_D14 * 8)]

	/* Restore SP */
	mov	sp, x3

	/* Return val (x1), or 1 if val == 0 per longjmp spec */
	mov	x0, x1
	cbnz	x1, 1f
	mov	x0, #1
1:
	ret

	/* Validation failed — report and abort */
.Lbotch:
	stp	x29, x30, [sp, #-16]!
	bl	_C_LABEL(longjmperror)
	bl	_C_LABEL(abort)
	/* Unreachable */
1:	b	1b
END(longjmp)

	.section .rodata.cst8,"aM",@progbits,8
	.align 3
.L_longjmp_magic:
	.xword	_JB_MAGIC_AARCH64__SETJMP
