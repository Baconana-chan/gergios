/*	$NetBSD: mcontext.h,v 1.12 2014/02/15 22:20:42 dsl Exp $	*/

/*-
 * Copyright (c) 1999 The NetBSD Foundation, Inc.
 * All rights reserved.
 *
 * This code is derived from software contributed to The NetBSD Foundation
 * by Klaus Klein.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED BY THE NETBSD FOUNDATION, INC. AND CONTRIBUTORS
 * ``AS IS'' AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED
 * TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR
 * PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE FOUNDATION OR CONTRIBUTORS
 * BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
 * CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
 * SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
 * CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

#ifndef _X86_64_MCONTEXT_H_
#define _X86_64_MCONTEXT_H_

/*
 * mcontext extensions to handle signal delivery.
 */
#define _UC_SETSTACK	0x00010000
#define _UC_CLRSTACK	0x00020000
#define _UC_VM		0x00040000
#define	_UC_TLSBASE	0x00080000

/*
 * Layout of mcontext_t according to the System V Application Binary
 * Interface, AMD64(tm) Architecture Processor Supplement.
 *
 * General register state — 26 entries, 8 bytes each.
 */
#define _NGREG		26

typedef unsigned long	__greg_t;
typedef __greg_t	__gregset_t[_NGREG];

#define _REG_R15	0
#define _REG_R14	1
#define _REG_R13	2
#define _REG_R12	3
#define _REG_R11	4
#define _REG_R10	5
#define _REG_R9		6
#define _REG_R8		7
#define _REG_RDI	8
#define _REG_RSI	9
#define _REG_RBP	10
#define _REG_RBX	11
#define _REG_RDX	12
#define _REG_RCX	13
#define _REG_RAX	14
#define _REG_TRAPNO	15
#define _REG_ERR	16
#define _REG_RIP	17
#define _REG_CS		18
#define _REG_RFLAGS	19
#define _REG_RSP	20
#define _REG_SS		21
#define _REG_GS		22
#define _REG_FS		23
#define _REG_ES		24
#define _REG_DS		25

/*
 * Floating point register state — same FXSAVE format as i386.
 */
typedef struct {
	union {
		struct {
			int	__fp_state[27];	/* Environment and registers */
		} __fpchip_state;		/* x87 regs in fsave format */
		struct {
			char	__fp_xmm[512];
		} __fp_xmm_state;		/* x87 and xmm regs in fxsave format */
		int	__fp_fpregs[128];
	} __fp_reg_set;
	int 	__fp_pad[33];			/* Historic padding */
} __fpregset_t;
__CTASSERT(sizeof (__fpregset_t) == 512 + 33 * 4);

typedef struct {
	__gregset_t	__gregs;
	__fpregset_t	__fpregs;
	__greg_t	_mc_tlsbase;
#ifdef __minix
	int	mc_magic;
	int	mc_flags;
#endif
} mcontext_t;

#define _UC_FXSAVE	0x20	/* FP state is in FXSAVE format in XMM space */

#define _UC_MACHINE_PAD	4	/* Padding appended to ucontext_t */

#define _UC_UCONTEXT_ALIGN	(~0xf)

#define _UC_MACHINE_SP(uc)	((uc)->uc_mcontext.__gregs[_REG_RSP])
#define _UC_MACHINE_PC(uc)	((uc)->uc_mcontext.__gregs[_REG_RIP])
#define _UC_MACHINE_INTRV(uc)	((uc)->uc_mcontext.__gregs[_REG_RAX])

#define	_UC_MACHINE_SET_PC(uc, pc)	_UC_MACHINE_PC(uc) = (pc)

#if defined(__minix)
#define	_UC_MACHINE_STACK(uc)		((uc)->uc_mcontext.__gregs[_REG_RSP])
#define	_UC_MACHINE_SET_STACK(uc, sp)	_UC_MACHINE_STACK(uc) = (sp)

#define	_UC_MACHINE_RBP(uc)		((uc)->uc_mcontext.__gregs[_REG_RBP])
#define	_UC_MACHINE_SET_RBP(uc, rbp)	_UC_MACHINE_RBP(uc) = (rbp)

#define	_UC_MACHINE_R12(uc)		((uc)->uc_mcontext.__gregs[_REG_R12])
#define	_UC_MACHINE_SET_R12(uc, val)	_UC_MACHINE_R12(uc) = (val)

#define	_UC_MACHINE_RDI(uc)		((uc)->uc_mcontext.__gregs[_REG_RDI])
#define	_UC_MACHINE_SET_RDI(uc, val)	_UC_MACHINE_RDI(uc) = (val)

#define	_UC_MACHINE_RSI(uc)		((uc)->uc_mcontext.__gregs[_REG_RSI])
#define	_UC_MACHINE_SET_RSI(uc, val)	_UC_MACHINE_RSI(uc) = (val)

#define	_UC_MACHINE_RDX(uc)		((uc)->uc_mcontext.__gregs[_REG_RDX])
#define	_UC_MACHINE_SET_RDX(uc, val)	_UC_MACHINE_RDX(uc) = (val)

#define	_UC_MACHINE_RCX(uc)		((uc)->uc_mcontext.__gregs[_REG_RCX])
#define	_UC_MACHINE_SET_RCX(uc, val)	_UC_MACHINE_RCX(uc) = (val)

#define	_UC_MACHINE_R8(uc)		((uc)->uc_mcontext.__gregs[_REG_R8])
#define	_UC_MACHINE_SET_R8(uc, val)	_UC_MACHINE_R8(uc) = (val)

#define	_UC_MACHINE_R9(uc)		((uc)->uc_mcontext.__gregs[_REG_R9])
#define	_UC_MACHINE_SET_R9(uc, val)	_UC_MACHINE_R9(uc) = (val)

int setmcontext(const mcontext_t *mcp);
int getmcontext(mcontext_t *mcp);

#define MCF_MAGIC 0xc0ffee
#define _MC_FPU_SAVED	0x001

#endif /* defined(__minix) */

#define	__UCONTEXT_SIZE	776

static __inline void *
__lwp_getprivate_fast(void)
{
	void *__tmp;
	__asm volatile("movq %%fs:0, %0" : "=r" (__tmp));
	return __tmp;
}

#endif	/* !_X86_64_MCONTEXT_H_ */
