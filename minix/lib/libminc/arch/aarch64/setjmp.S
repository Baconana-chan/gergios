/* $NetBSD$ */

/*-
 * Copyright (c) 2025 MINIX 3 Contributors
 *
 * Minimal C library (libminc) AArch64 setjmp.
 *
 * Saves the callee-saved registers per AAPCS64:
 *   - x19-x29 (11 general-purpose registers)
 *   - x30 (LR)
 *   - SP
 *   - d8-d15 (8 VFP/NEON double-precision registers)
 *
 * The jmp_buf layout is defined in <machine/setjmp.h>.
 * No signal mask is saved (libminc targets use _setjmp semantics).
 */

#include <machine/asm.h>
#include <machine/setjmp.h>

/*
 * setjmp(jmp_buf env)
 *
 * Save the current register context into env.
 * Returns 0.
 */
ENTRY(setjmp)
	/* Save magic number */
	adrp	x7, .L_setjmp_magic
	ldr	x7, [x7, #:lo12:.L_setjmp_magic]
	str	x7, [x0, #(_JB_MAGIC * 8)]

	/* Save SP (at _JB_SP * 8, separate from magic slot) */
	mov	x3, sp
	str	x3, [x0, #(_JB_SP * 8)]

	/* Save callee-saved GPRs x19-x28 */
	stp	x19, x20, [x0, #(_JB_X19 * 8)]
	stp	x21, x22, [x0, #(_JB_X21 * 8)]
	stp	x23, x24, [x0, #(_JB_X23 * 8)]
	stp	x25, x26, [x0, #(_JB_X25 * 8)]
	stp	x27, x28, [x0, #(_JB_X27 * 8)]

	/* Save x29 (FP) and x30 (LR) */
	stp	x29, x30, [x0, #(_JB_X29 * 8)]

	/* Save callee-saved VFP registers d8-d15 */
	stp	d8,  d9,  [x0, #(_JB_D8 * 8)]
	stp	d10, d11, [x0, #(_JB_D10 * 8)]
	stp	d12, d13, [x0, #(_JB_D12 * 8)]
	stp	d14, d15, [x0, #(_JB_D14 * 8)]

	/* Return 0 */
	mov	x0, xzr
	ret
END(setjmp)

	.section .rodata.cst8,"aM",@progbits,8
	.align 3
.L_setjmp_magic:
	.xword	_JB_MAGIC_AARCH64__SETJMP
