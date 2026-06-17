/*	$NetBSD: signal.h,v 1.29 2008/11/19 18:35:59 ad Exp $	*/

/*
 * Copyright (c) 1982, 1986, 1989, 1991 Regents of the University of California.
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 * 3. Neither the name of the University nor the names of its contributors
 *    may be used to endorse or promote products derived from this software
 *    without specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE REGENTS AND CONTRIBUTORS ``AS IS'' AND
 * ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED.  IN NO EVENT SHALL THE REGENTS OR CONTRIBUTORS BE LIABLE
 * FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
 * DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
 * OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
 * LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
 * OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
 * SUCH DAMAGE.
 *
 *	@(#)signal.h	7.16 (Berkeley) 3/17/91
 */

#ifndef _X86_64_SIGNAL_H_
#define _X86_64_SIGNAL_H_

#include <sys/featuretest.h>

typedef int sig_atomic_t;

#if defined(_NETBSD_SOURCE)
/*
 * Get the "code" values
 */
#include <machine/trap.h>

#if defined(_KERNEL) || defined(__minix)
/*
 * Information pushed on stack when a signal is delivered.
 * This is used by the kernel to restore state following
 * execution of the signal handler.  It is also made available
 * to the handler to allow it to restore state properly if
 * a non-standard exit is performed.
 *
 * x86_64 sigcontext: matches the 64-bit register layout
 * with 16 general-purpose registers + segment registers + trap info.
 */
struct sigcontext {
	unsigned long sc_r15;
	unsigned long sc_r14;
	unsigned long sc_r13;
	unsigned long sc_r12;
	unsigned long sc_r11;
	unsigned long sc_r10;
	unsigned long sc_r9;
	unsigned long sc_r8;
	unsigned long sc_rdi;
	unsigned long sc_rsi;
	unsigned long sc_rbp;
	unsigned long sc_rbx;
	unsigned long sc_rdx;
	unsigned long sc_rcx;
	unsigned long sc_rax;
	unsigned long sc_gs;
	unsigned long sc_fs;
	unsigned long sc_es;
	unsigned long sc_ds;
	unsigned long sc_trapno;
	unsigned long sc_err;
	unsigned long sc_rip;
	unsigned long sc_cs;
	unsigned long sc_rflags;
	unsigned long sc_rsp;
	unsigned long sc_ss;

	int	sc_onstack;		/* sigstack state to restore */
	int	__sc_mask13;		/* signal mask to restore (old style) */

	sigset_t sc_mask;		/* signal mask to restore (new style) */
#if defined(__minix) && (defined(_LIBMINC) || ! defined(_STANDALONE))
	union fpu_state_u sc_fpu_state;
	int trap_style;			/* KTS_* method of entering kernel */
	int sc_flags;			/* MF_FPU_INITIALIZED if fpu state valid */
#define SC_MAGIC 0xc0ffee3
	int sc_magic;
#endif /* defined(__minix) ... */
};
#endif /* _KERNEL */

#if defined(__minix) && (defined(_LIBMINC) || ! defined(_STANDALONE))
__BEGIN_DECLS
int sigreturn(struct sigcontext *_scp);
__END_DECLS
#endif /* defined(__minix) */

#endif	/* _NETBSD_SOURCE */
#endif	/* !_X86_64_SIGNAL_H_ */
