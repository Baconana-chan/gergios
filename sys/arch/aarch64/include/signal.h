/*	$NetBSD$	*/

/*
 * Copyright (c) 2024 GergiOS.
 * All rights reserved.
 *
 * Based on NetBSD aarch64 signal.h and MINIX arm/signal.h.
 *
 * signal.h — Architecture dependent signal types and structures
 *
 * ARM64 (AArch64) signal handling differs from 32-bit ARM:
 *   31 general-purpose registers (x0–x30) × 64-bit instead of 16 (r0–r15) × 32-bit
 *   PSTATE (64-bit) instead of CPSR (32-bit)
 *   FAR_EL1 (fault address) stored in sigcontext
 */

#ifndef _AARCH64_SIGNAL_H_
#define _AARCH64_SIGNAL_H_

#include <sys/featuretest.h>

#ifndef _LOCORE
typedef int sig_atomic_t;
#endif

#if defined(_NETBSD_SOURCE)

#ifndef _LOCORE

/*
 * Signal context (sigcontext).
 *
 * Pushed onto the user stack when a signal is delivered.
 * Used by the kernel to restore state after signal handler execution.
 * Also available to the handler for non-standard exit.
 *
 * ARM64 sigcontext layout:
 *   int		sc_onstack	- sigstack state to restore
 *   int		__sc_mask13	- signal mask (old style, compatibility)
 *   uint64_t		sc_faultaddr	- FAR_EL1 at exception (fault address)
 *   uint64_t		sc_x[31]	- x0–x30 general-purpose registers
 *   uint64_t		sc_sp		- SP (user stack pointer)
 *   uint64_t		sc_pc		- PC (program counter at exception)
 *   uint64_t		sc_pstate	- PSTATE (saved processor state)
 *   sigset_t		sc_mask		- signal mask (new style)
 *   int			sc_magic	- magic number (MINIX)
 *   int			sc_flags	- flags (MINIX)
 *   int			trap_style	- trap type indicator
 */

struct sigcontext {
	/* Core signal state */
	int		sc_onstack;		/* sigstack state to restore */
	int		__sc_mask13;		/* signal mask (old style) */

	/* ARM64 exception state */
	uint64_t	sc_faultaddr;		/* FAR_EL1 (fault address) */

	/* General-purpose registers (x0–x30) */
	uint64_t	sc_x[31];

	/* Stack, program counter, and processor state */
	uint64_t	sc_sp;			/* SP_EL0 at exception */
	uint64_t	sc_pc;			/* ELR_EL1 at exception */
	uint64_t	sc_pstate;		/* SPSR_EL1 at exception */

	/* Signal mask (new style) */
	sigset_t	sc_mask;

	/* MINIX-specific fields */
	int		sc_magic;		/* SC_MAGIC for validation */
	int		sc_flags;		/* Additional flags */
	int		trap_style;		/* Trap type (KTS_*) */
};

/* Magic number for sigcontext validation */
#define SC_MAGIC	0xc0ffee4

#endif /* !_LOCORE */

/*
 * Signal code definitions.
 *
 * SIGFPE codes: use FP exception codes from ieeefp.h
 */
#define SIG_CODE_FPE_CODE_MASK	0x00000f00
#define SIG_CODE_FPE_CODE_SHIFT	8
#define SIG_CODE_FPE_TYPE_MASK	0x000000ff

/*
 * SIGBUS and SIGSEGV codes.
 *
 * The signal code is a combination of the fault address and the fault code.
 * For ARM64, the fault status code comes from ESR_EL1 bits [5:0] (FSC).
 */
#define SIG_CODE_SEGV_ADDR_MASK	0xfffffff0
#define SIG_CODE_SEGV_TYPE_MASK	0x0000000f
#define SIG_CODE_BUS_ADDR_MASK	SIG_CODE_SEGV_ADDR_MASK
#define SIG_CODE_BUS_TYPE_MASK	SIG_CODE_SEGV_TYPE_MASK

#endif	/* _NETBSD_SOURCE */

#if defined(__minix)
__BEGIN_DECLS
int sigreturn(struct sigcontext *_scp);
__END_DECLS
#endif /* defined(__minix) */

#endif /* !_AARCH64_SIGNAL_H_ */

/* End of signal.h */
