/*	$NetBSD$	*/

/*
 * Copyright (c) 2024 GergiOS
 * All rights reserved.
 *
 * Virtual Memory parameters for AArch64 (ARMv8-A).
 *
 * AArch64 has a 48-bit virtual address space with the following split:
 *   User space:  0x0000000000000000 - 0x0000FFFFFFFFFFFE (48-bit user VA)
 *   Kernel space: 0xFFFF000000000000 - 0xFFFFFFFFFFFFFFFE (high half, TTBR1_EL1)
 *
 * Our MINIX port uses a simpler split:
 *   User space:  0x0000000000010000 - KERNEL_BASE
 *   Kernel space: KERNEL_BASE+
 *
 * Page size: 4KB (standard, matches existing MINIX assumptions).
 */

#ifndef _AARCH64_VMPARAM_H_
#define	_AARCH64_VMPARAM_H_

#if defined(_KERNEL) || defined(_KMEMUSER) || defined(__minix)

/*
 * AArch64 virtual address space layout.
 * KERNEL_BASE is the start of the kernel virtual address space,
 * mapped via TTBR1_EL1 (kernel page tables).
 */
#define	KERNEL_BASE		0xFFFF800000000000UL

/* User stack at the top of user-accessible space */
#define	USRSTACK		(KERNEL_BASE - PAGE_SIZE)

/* Top-down VM allocation */
#define	__USE_TOPDOWN_VM

/* Text, data, and stack size limits (same as ARM32 - reasonable defaults) */
#define	MAXTSIZ			(128*1024*1024)		/* max text size */
#ifndef	DFLDSIZ
#define	DFLDSIZ			(384*1024*1024)		/* initial data size limit */
#endif
#ifndef	MAXDSIZ
#define	MAXDSIZ			(1536*1024*1024)	/* max data size */
#endif
#ifndef	DFLSSIZ
#define	DFLSSIZ			(4*1024*1024)		/* initial stack size limit */
#endif
#ifndef	MAXSSIZ
#define	MAXSSIZ			(64*1024*1024)		/* max stack size */
#endif

/* Pager map default size */
#define PAGER_MAP_DEFAULT_SIZE	(4 * 1024 * 1024)

/* Page size: 4KB */
#define	PAGE_SHIFT		PGSHIFT
#define	PAGE_SIZE		(1 << PAGE_SHIFT)
#define	PAGE_MASK		(PAGE_SIZE - 1)

/* Address space constants */
#define	VM_MIN_ADDRESS		((vaddr_t) PAGE_SIZE)
#define	VM_MAXUSER_ADDRESS	((vaddr_t) KERNEL_BASE - PAGE_SIZE)
#define	VM_MAX_ADDRESS		VM_MAXUSER_ADDRESS

#define	VM_MIN_KERNEL_ADDRESS	((vaddr_t) KERNEL_BASE)
#define	VM_MAX_KERNEL_ADDRESS	((vaddr_t) -1)

/* Size of User Raw I/O map */
#define USRIOSIZE		300

/* Virtual sizes (bytes) for various kernel submaps */
#define VM_PHYS_SIZE		(USRIOSIZE * PAGE_SIZE)

/* Max number of non-contiguous physical RAM chunks */
#define	VM_PHYSSEG_MAX		32

/* Physical segment strategy: binary search */
#define	VM_PHYSSEG_STRAT	VM_PSTRAT_BSEARCH

/* Free lists */
#define	VM_NFREELIST		1
#define	VM_FREELIST_DEFAULT	0

#if !defined(__minix)
#ifndef __ASSEMBLER__
/* Max amount of KVM to be used by buffers */
#ifndef VM_MAX_KERNEL_BUF
extern vaddr_t virtual_avail;
extern vaddr_t virtual_end;

#define	VM_MAX_KERNEL_BUF	\
	((virtual_end - virtual_avail) * 4 / 10)
#endif
#endif /* __ASSEMBLER__ */
#endif /* !defined(__minix) */

#endif /* _KERNEL || _KMEMUSER */

#endif /* _AARCH64_VMPARAM_H_ */
