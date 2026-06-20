/*	$NetBSD$	*/

/*
 * Copyright (c) 2024 GergiOS.
 * All rights reserved.
 *
 * Based on NetBSD aarch64 frame.h and MINIX arm/frame.h.
 *
 * frame.h — Stack frames structures for ARM64 (AArch64).
 *
 * ARM64 trap/signal frame definitions:
 *   trapframe_t:  Pushed onto the kernel stack on a trap (exception).
 *   sigframe_sigcontext: Pushed onto the user stack before sigcode.
 *   sigframe_siginfo: Pushed for SA_SIGINFO signals.
 *
 * ARM64 exception entry (from EL0 to EL1):
 *   - CPU auto-saves: ELR_EL1 (return PC), SPSR_EL1 (saved PSTATE)
 *   - CPU does NOT auto-save: SP_EL0, GPRs
 *   - The kernel handler saves all registers manually
 *
 * The trapframe is the complete register save area on the kernel stack.
 */

#ifndef _AARCH64_FRAME_H_
#define _AARCH64_FRAME_H_

#ifndef _LOCORE

#include <sys/signal.h>
#include <sys/ucontext.h>

/*
 * Trap frame.
 *
 * Pushed onto the kernel stack on a trap (synchronous exception).
 * Contains the full register state of the interrupted process.
 *
 * Stack layout (from high to low):
 *   [SP+272]  <- bottom of trapframe (highest address)
 *   [SP+264]  SPSR_EL1 (saved processor state)
 *   [SP+256]  ELR_EL1 (exception return address)
 *   [SP+248]  SP_EL0 (user stack pointer)
 *   [SP+240]  x30 (LR)
 *   [SP+232]  x29 (FP)
 *   ...       x28..x19 (callee-saved)
 *   [SP+0]    x0 (first argument)
 */
typedef struct trapframe {
	/* General-purpose registers (x0–x30), saved manually */
	uint64_t	tf_x[31];

	/* User stack pointer (SP_EL0), saved manually */
	uint64_t	tf_sp;

	/* Exception return address (ELR_EL1), from CPU */
	uint64_t	tf_pc;

	/* Saved processor state (SPSR_EL1), from CPU */
	uint64_t	tf_spsr;
} trapframe_t;

/* Register number access macros */
#define tf_r0		tf_x[0]
#define tf_r1		tf_x[1]
#define tf_x0		tf_x[0]
#define tf_x30		tf_x[30]
#define tf_lr		tf_x[30]
#define tf_fp		tf_x[29]
#define tf_sp		tf_sp
#define tf_pc		tf_pc
#define tf_spsr		tf_spsr

/* Check if the trap came from user mode */
#define TRAP_USERMODE(tf)	\
	(((tf)->tf_spsr & 0x0F) == 0)	/* SPSR_EL1.M[3:0] == 0 (EL0t) */

/*
 * Signal frame: sigcontext variant.
 *
 * Pushed onto the user stack before calling the signal handler via sigcode.
 * Used for sigreturn() to restore the interrupted context.
 */
struct sigframe_sigcontext {
	struct sigcontext *sf_scp;	/* Pointer to sigcontext (for sigreturn) */
	struct sigcontext  sf_sc;	/* Actual sigcontext */
};

/*
 * Signal frame: siginfo variant.
 *
 * Pushed for signals with SA_SIGINFO. Contains both siginfo and ucontext.
 * The trampoline code uses these pointers to locate the ucontext.
 */
struct sigframe_siginfo {
	siginfo_t	sf_si;		/* Actual saved siginfo */
	ucontext_t	sf_uc;		/* Actual saved ucontext */
};

#ifdef _KERNEL
__BEGIN_DECLS
void sendsig_sigcontext(const ksiginfo_t *, const sigset_t *);
void *getframe(struct lwp *, int, int *);
__END_DECLS

/* Macros to access an LWP's trapframe */
#define lwp_trapframe(l)		((l)->l_md.md_tf)
#define lwp_settrapframe(l, tf)		((l)->l_md.md_tf = (tf))
#endif /* _KERNEL */

#endif /* _LOCORE */

#endif /* _AARCH64_FRAME_H_ */

/* End of frame.h */
