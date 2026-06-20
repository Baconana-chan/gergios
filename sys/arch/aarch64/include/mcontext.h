/*	$NetBSD$	*/

/*-
 * Copyright (c) 2024 GergiOS.
 * All rights reserved.
 *
 * Based on NetBSD aarch64 mcontext.h.
 *
 * ARM64 (AArch64) machine context for signal handling and thread state.
 *
 * ARM64 register state:
 *   31 general-purpose registers (x0–x30), each 64-bit
 *   SP (stack pointer, SP_EL0)
 *   PC (program counter, ELR_EL1)
 *   PSTATE (processor state, SPSR_EL1)
 *
 * Floating-point/SIMD state:
 *   32 VFP registers (v0–v31), each 128-bit (quad-word)
 *   FPSR (Floating-point Status Register)
 *   FPCR (Floating-point Control Register)
 */

#ifndef _AARCH64_MCONTEXT_H_
#define _AARCH64_MCONTEXT_H_

#include <sys/stdint.h>

/*
 * General register state: 31 GPRs (x0–x30).
 */
#define _NGREG		31
typedef uint64_t	__greg_t;
typedef __greg_t	__gregset_t[_NGREG];

/* Index aliases for general-purpose registers */
#define _REG_X0		0
#define _REG_X1		1
#define _REG_X2		2
#define _REG_X3		3
#define _REG_X4		4
#define _REG_X5		5
#define _REG_X6		6
#define _REG_X7		7
#define _REG_X8		8
#define _REG_X9		9
#define _REG_X10	10
#define _REG_X11	11
#define _REG_X12	12
#define _REG_X13	13
#define _REG_X14	14
#define _REG_X15	15
#define _REG_X16	16
#define _REG_X17	17
#define _REG_X18	18
#define _REG_X19	19
#define _REG_X20	20
#define _REG_X21	21
#define _REG_X22	22
#define _REG_X23	23
#define _REG_X24	24
#define _REG_X25	25
#define _REG_X26	26
#define _REG_X27	27
#define _REG_X28	28
#define _REG_X29	29	/* Frame pointer */
#define _REG_X30	30	/* Link register */

/* Convenience synonyms */
#define _REG_FP		_REG_X29
#define _REG_LR		_REG_X30

/*
 * Additional saved state (after GPRs).
 */
#define _REG_SP		31	/* SP_EL0 (user stack pointer) */
#define _REG_PC		32	/* ELR_EL1 (program counter) */
#define _REG_PSTATE	33	/* SPSR_EL1 (saved processor state) */

/* Total number of "general" registers */
#define _NGREG_EXTENDED	34

/*
 * Floating-point / SIMD register state.
 *
 * ARM64 has 32 × 128-bit VFP registers. Each is stored as two
 * 64-bit values (low and high parts) for compatibility.
 */
typedef struct __fpreg {
	uint64_t	fp_low;		/* Lower 64 bits */
	uint64_t	fp_high;	/* Upper 64 bits */
} __fpreg_t;

#define _NFREG		32		/* 32 VFP registers */

typedef struct __fpregset {
	uint64_t	fp_fpsr;	/* Floating-point Status Register */
	uint64_t	fp_fpcr;	/* Floating-point Control Register */
	__fpreg_t	fp_reg[_NFREG];	/* v0..v31 */
} __fpregset_t;

/*
 * Machine context structure.
 *
 * Layout:
 *   __gregset_t	__gregs		- 31 × uint64_t (x0–x30)
 *   uint64_t		__sp		- SP_EL0
 *   uint64_t		__pc		- ELR_EL1
 *   uint64_t		__pstate	- SPSR_EL1
 *   __fpregset_t	__fpregs	- FP/SIMD state
 *   int			mc_flags	- Flags (MINIX-specific)
 *   int			mc_magic	- Magic number (MINIX-specific)
 */
typedef struct {
	/* General-purpose registers */
	__gregset_t	__gregs;

	/* Stack pointer, program counter, and processor state */
	uint64_t	__sp;
	uint64_t	__pc;
	uint64_t	__pstate;

	/* Floating-point / SIMD register state */
	__fpregset_t	__fpregs;

	/* MINIX-specific fields */
	int		mc_flags;
	int		mc_magic;
} mcontext_t;

/* Machine-dependent uc_flags */
#define	_UC_VFP		0x00010000	/* FPU field is VFP */

/* Signal stack management flags */
#define _UC_SETSTACK	0x00020000
#define _UC_CLRSTACK	0x00040000

#define _UC_MACHINE_PAD	1		/* Padding appended to ucontext_t */

/* Macros to access key registers from ucontext_t */
#define _UC_MACHINE_SP(uc)	((uc)->uc_mcontext.__sp)
#define _UC_MACHINE_PC(uc)	((uc)->uc_mcontext.__pc)
#define	_UC_MACHINE_INTRV(uc)	((uc)->uc_mcontext.__gregs[_REG_X0])

#define	_UC_MACHINE_SET_PC(uc, pc)	_UC_MACHINE_PC(uc) = (pc)

/* MINIX-specific convenience macros */
#define _UC_MACHINE_STACK(uc)		((uc)->uc_mcontext.__sp)
#define	_UC_MACHINE_SET_STACK(uc, sp)	_UC_MACHINE_STACK(uc) = (sp)

#define _UC_MACHINE_FP(uc)		((uc)->uc_mcontext.__gregs[_REG_FP])
#define	_UC_MACHINE_SET_FP(uc, fp)	_UC_MACHINE_FP(uc) = (fp)

#define _UC_MACHINE_LR(uc)		((uc)->uc_mcontext.__gregs[_REG_LR])
#define	_UC_MACHINE_SET_LR(uc, lr)	_UC_MACHINE_LR(uc) = (lr)

#define _UC_MACHINE_R0(uc)		((uc)->uc_mcontext.__gregs[_REG_X0])
#define	_UC_MACHINE_SET_R0(uc, setreg)	_UC_MACHINE_R0(uc) = (setreg)

#define _UC_MACHINE_R1(uc)		((uc)->uc_mcontext.__gregs[_REG_X1])
#define	_UC_MACHINE_SET_R1(uc, setreg)	_UC_MACHINE_R1(uc) = (setreg)

#define _UC_MACHINE_R2(uc)		((uc)->uc_mcontext.__gregs[_REG_X2])
#define	_UC_MACHINE_SET_R2(uc, setreg)	_UC_MACHINE_R2(uc) = (setreg)

#define _UC_MACHINE_R3(uc)		((uc)->uc_mcontext.__gregs[_REG_X3])
#define	_UC_MACHINE_SET_R3(uc, setreg)	_UC_MACHINE_R3(uc) = (setreg)

#define _UC_MACHINE_R4(uc)		((uc)->uc_mcontext.__gregs[_REG_X4])
#define	_UC_MACHINE_SET_R4(uc, setreg)	_UC_MACHINE_R4(uc) = (setreg)

/*
 * ucontext_t structure size hint for stack allocation.
 *
 * ARM64 mcontext_t size:
 *   __gregset_t[31]:  31 × 8  = 248 bytes
 *   __sp:                     =   8
 *   __pc:                     =   8
 *   __pstate:                 =   8
 *   __fpregset_t:             = 528  (FPSR+FPCR+32×128-bit registers)
 *   mc_flags+mc_magic:       =   8
 *   Padding/alignment:        ≈  16
 *   Total:                    ≈ 824 bytes
 *
 * __UCONTEXT_SIZE must cover the full ucontext_t (mcontext_t + overhead).
 */
#define	__UCONTEXT_SIZE	1024

__BEGIN_DECLS
#if !defined(_KERNEL) && !defined(_LOCORE)
/* User-space functions for getcontext/setcontext */
int	setmcontext(const mcontext_t *mcp);
int	getmcontext(mcontext_t *mcp);
#endif /* !_KERNEL && !_LOCORE */
__END_DECLS

/* Magic number for MINIX mcontext validation */
#define MCF_MAGIC	0xc0ffee

#endif /* !_AARCH64_MCONTEXT_H_ */
