/*	$NetBSD$	*/

/*
 * machine/setjmp.h: machine dependent setjmp-related information.
 *
 * ARM64 setjmp/longjmp buffer layout (jmp_buf):
 *
 * ARM64 (AArch64) calling convention (AAPCS64):
 *   Callee-saved registers: x19–x29 (11 registers)
 *   Link register: x30 (LR) — saved because setjmp/longjmp may change it
 *   Stack pointer: SP
 *
 * Floating-point/SIMD (if VFP is present):
 *   d8–d15 (8 double-precision registers) — callee-saved per AAPCS64
 *
 * Also saved: signal mask (if _JB_MAGIC_SETJMP)
 *
 * Word layout (each entry is 8 bytes / 1 long):
 *   word   0: _JB_MAGIC (identifies the buffer type)
 *   word   1: x19
 *   word   2: x20
 *   word   3: x21
 *   word   4: x22
 *   word   5: x23
 *   word   6: x24
 *   word   7: x25
 *   word   8: x26
 *   word   9: x27
 *   word  10: x28
 *   word  11: x29 (FP)
 *   word  12: x30 (LR)
 *   word  13: SP
 *   word  14: d8 (low 64 bits)
 *   word  15: d9 (low 64 bits)
 *   ... (up to d15)
 *   word  22: d15 (low 64 bits)
 *   word  23-26: signal mask (4 longs, _JB_MAGIC_SETJMP only)
 *
 * _JBLEN must be large enough to hold all of the above.
 */

#ifndef _AARCH64_SETJMP_H_
#define _AARCH64_SETJMP_H_

#define	_JBLEN	64		/* size, in longs, of a jmp_buf */

/* Magic numbers for jmp_buf identification */
#define _JB_MAGIC__SETJMP	0x4a8f5000
#define _JB_MAGIC_SETJMP	0x4a8f5001
#define _JB_MAGIC__SETJMP_FP	0x4a8f5002
#define _JB_MAGIC_SETJMP_FP	0x4a8f5003

/* Index of magic number */
#define _JB_MAGIC		 0

/* Index of callee-saved general-purpose registers */
#define _JB_X19			 1
#define _JB_X20			 2
#define _JB_X21			 3
#define _JB_X22			 4
#define _JB_X23			 5
#define _JB_X24			 6
#define _JB_X25			 7
#define _JB_X26			 8
#define _JB_X27			 9
#define _JB_X28			10
#define _JB_X29			11	/* Frame pointer */
#define _JB_X30			12	/* Link register */
#define _JB_SP			13	/* Stack pointer */

/* Index of callee-saved VFP registers (d8–d15, double-precision) */
#define _JB_D8			14
#define _JB_D9			15
#define _JB_D10			16
#define _JB_D11			17
#define _JB_D12			18
#define _JB_D13			19
#define _JB_D14			20
#define _JB_D15			21

/* Signal mask (only valid with _JB_MAGIC_SETJMP magic) */
#define _JB_SIGMASK		22

#endif /* _AARCH64_SETJMP_H_ */
