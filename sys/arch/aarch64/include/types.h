/*	$NetBSD: types.h,v 1.30 2015/08/27 12:30:50 pooka Exp $	*/

#ifndef _AARCH64_TYPES_H_
#define _AARCH64_TYPES_H_

#include <sys/cdefs.h>
#include <sys/featuretest.h>
#include <machine/int_types.h>

/* AArch64 64-bit types */
typedef unsigned long	paddr_t;
typedef unsigned long	psize_t;
typedef unsigned long	vaddr_t;
typedef unsigned long	vsize_t;
#define	PRIxPADDR	"lx"
#define	PRIxPSIZE	"lu"
#define	PRIuPSIZE	"lu"
#define	PRIxVADDR	"lx"
#define	PRIxVSIZE	"lx"
#define	PRIuVSIZE	"lu"

typedef long		register_t;
typedef int		register32_t;	/* 32-bit register (e.g. W0 on ARM64) */
#define	PRIxREGISTER	"lx"

typedef unsigned long	pmc_evid_t;
#define PMC_INVALID_EVID	(-1UL)
typedef unsigned long	pmc_ctr_t;
typedef unsigned short	tlb_asid_t;

typedef unsigned char	__cpu_simple_lock_nv_t;
#define	__SIMPLELOCK_LOCKED	1
#define	__SIMPLELOCK_UNLOCKED	0

#define	__HAVE_SYSCALL_INTERN
#define	__HAVE_MINIMAL_EMUL
#define __HAVE_CPU_DATA_FIRST
#define	__HAVE_OLD_DISKLABEL

#if defined(_KERNEL)
typedef struct label_t {
        long val[11];
} label_t;
#endif

#endif /* _AARCH64_TYPES_H_ */
