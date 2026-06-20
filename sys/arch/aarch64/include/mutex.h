/*	$NetBSD: mutex.h,v 1.20 2015/02/25 13:52:42 joerg Exp $	*/

/*-
 * Copyright (c) 2002, 2007 The NetBSD Foundation, Inc.
 * All rights reserved.
 *
 * This code is derived from software contributed to The NetBSD Foundation
 * by Jason R. Thorpe and Andrew Doran.
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

/*
 * mutex.h — AArch64 mutex definitions.
 *
 * ARMv8-A has LDXR/STXR exclusive access (like ARMv7's LDREX/STREX),
 * and DMB ISH for multicore coherence. No pre-v6 CAS workaround needed.
 * WFE/SEV are available for SMT pause/wake.
 */

#ifndef _AARCH64_MUTEX_H_
#define	_AARCH64_MUTEX_H_

#ifndef __MUTEX_PRIVATE

struct kmutex {
	uintptr_t	mtx_pad1;
};

#else	/* __MUTEX_PRIVATE */

struct kmutex {
	union {
		/* Adaptive mutex */
		volatile uintptr_t	mtxa_owner;

		/* Spin mutex */
		struct {
			volatile uint8_t	mtxs_dummy;
			ipl_cookie_t		mtxs_ipl;
			__cpu_simple_lock_t	mtxs_lock;
			volatile uint8_t	mtxs_unused;
		} s;
	} u;
};

#define	mtx_owner		u.mtxa_owner
#define	mtx_ipl			u.s.mtxs_ipl
#define	mtx_lock		u.s.mtxs_lock

#define	__HAVE_SIMPLE_MUTEXES		1

/* Memory barriers for mutex acquire/release */
#ifdef MULTIPROCESSOR
#define	MUTEX_RECEIVE(mtx)		__asm __volatile("dmb ish" ::: "memory")
#define	MUTEX_GIVE(mtx)			__asm __volatile("dmb ish" ::: "memory")
#else
#define	MUTEX_RECEIVE(mtx)		/* nothing */
#define	MUTEX_GIVE(mtx)			/* nothing */
#endif

#define	MUTEX_CAS(p, o, n)		\
    (atomic_cas_ulong((volatile unsigned long *)(p), (o), (n)) == (o))

#ifdef MULTIPROCESSOR
#define	MUTEX_SMT_PAUSE()		__asm __volatile("wfe")
#define	MUTEX_SMT_WAKE()		__asm __volatile("sev")
#endif

#endif	/* __MUTEX_PRIVATE */

#endif /* _AARCH64_MUTEX_H_ */
