/*	$NetBSD: frame.h,v 1.35 2012/02/19 21:06:11 rmind Exp $	*/

/*-
 * Copyright (c) 1998 The NetBSD Foundation, Inc.
 * All rights reserved.
 *
 * This code is derived from software contributed to The NetBSD Foundation
 * by Charles M. Hannum.
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
 *	@(#)frame.h	5.2 (Berkeley) 1/18/91
 */

#ifndef _X86_64_FRAME_H_
#define _X86_64_FRAME_H_

#include <sys/signal.h>

/*
 * System stack frames.
 */

/*
 * Exception/Trap Stack Frame (x86_64)
 */
struct trapframe {
	uint64_t	tf_r15;
	uint64_t	tf_r14;
	uint64_t	tf_r13;
	uint64_t	tf_r12;
	uint64_t	tf_r11;
	uint64_t	tf_r10;
	uint64_t	tf_r9;
	uint64_t	tf_r8;
	uint64_t	tf_rdi;
	uint64_t	tf_rsi;
	uint64_t	tf_rbp;
	uint64_t	tf_rbx;
	uint64_t	tf_rdx;
	uint64_t	tf_rcx;
	uint64_t	tf_rax;
	uint64_t	tf_trapno;
	/* below portion defined in x86_64 hardware */
	uint64_t	tf_err;
	uint64_t	tf_rip;
	uint64_t	tf_cs;
	uint64_t	tf_rflags;
	/* below used when transitting rings (e.g. user to kernel) */
	uint64_t	tf_rsp;
	uint64_t	tf_ss;
};

/*
 * Interrupt stack frame
 */
struct intrframe {
	uint64_t	if_ppl;
	uint64_t	if_r15;
	uint64_t	if_r14;
	uint64_t	if_r13;
	uint64_t	if_r12;
	uint64_t	if_r11;
	uint64_t	if_r10;
	uint64_t	if_r9;
	uint64_t	if_r8;
	uint64_t	if_rdi;
	uint64_t	if_rsi;
	uint64_t	if_rbp;
	uint64_t	if_rbx;
	uint64_t	if_rdx;
	uint64_t	if_rcx;
	uint64_t	if_rax;
	uint64_t	__if_trapno;	/* for compat with trap frame - trapno */
	uint64_t	__if_err;	/* for compat with trap frame - err */
	/* below portion defined in x86_64 hardware */
	uint64_t	if_rip;
	uint64_t	if_cs;
	uint64_t	if_rflags;
	/* below only when transitting rings (e.g. user to kernel) */
	uint64_t	if_rsp;
	uint64_t	if_ss;
};

/*
 * Stack frame inside cpu_switchto()
 */
struct switchframe {
	uint64_t	sf_r15;
	uint64_t	sf_r14;
	uint64_t	sf_r13;
	uint64_t	sf_r12;
	uint64_t	sf_rbx;
	uint64_t	sf_rbp;
	uint64_t	sf_rip;
};

#if defined(_KERNEL) || defined(__minix)
/*
 * Old-style signal frame (x86_64)
 */
struct sigframe_sigcontext {
#if defined(__minix)
	/* ret addr + stackframe for handler */
	uint64_t	sf_ra_sigreturn;	/* first return to sigreturn */
#else
	uint64_t	sf_ra;			/* return address for handler */
#endif /* defined(__minix) */
	int		sf_signum;		/* "signum" argument for handler */
	int		sf_code;		/* "code" argument for handler */
	struct sigcontext *sf_scp;		/* "scp" argument for handler */
#if defined(__minix)
	/* ret addr + stackframe for sigreturn */
	uint64_t	sf_fp;			/* saved FP */
	uint64_t	sf_ra;			/* actual return address for handler */
	struct sigcontext *sf_scpcopy;		/* minix scp copy */
#endif /* defined(__minix) */
	struct sigcontext sf_sc;		/* actual saved context */
};
#endif

/*
 * New-style signal frame
 */
struct sigframe_siginfo {
	uint64_t	sf_ra;		/* return address for handler */
	int		sf_signum;	/* "signum" argument for handler */
	siginfo_t	*sf_sip;	/* "sip" argument for handler */
	ucontext_t	*sf_ucp;	/* "ucp" argument for handler */
	siginfo_t	sf_si;		/* actual saved siginfo */
	ucontext_t	sf_uc;		/* actual saved ucontext */
};

#ifdef _KERNEL
void *getframe(struct lwp *, int, int *);
void buildcontext(struct lwp *, int, void *, void *);
void sendsig_sigcontext(const ksiginfo_t *, const sigset_t *);
#endif

#endif  /* _X86_64_FRAME_H_ */
