/*	x86_64 vmparam.h — Virtual Memory parameters	*/

#ifndef _X86_64_VMPARAM_H_
#define	_X86_64_VMPARAM_H_

#if defined(_KERNEL) || defined(_KMEMUSER) || defined(__minix)

/* x86_64 virtual address space layout */
#define	KERNEL_BASE		0xFFFF800000000000UL

#define	USRSTACK		(KERNEL_BASE - PAGE_SIZE)

#define	__USE_TOPDOWN_VM

#define	MAXTSIZ			(128*1024*1024)
#ifndef	DFLDSIZ
#define	DFLDSIZ			(384*1024*1024)
#endif
#ifndef	MAXDSIZ
#define	MAXDSIZ			(1536*1024*1024)
#endif
#ifndef	DFLSSIZ
#define	DFLSSIZ			(4*1024*1024)
#endif
#ifndef	MAXSSIZ
#define	MAXSSIZ			(64*1024*1024)
#endif

#define PAGER_MAP_DEFAULT_SIZE	(4 * 1024 * 1024)

#define	PAGE_SHIFT		PGSHIFT
#define	PAGE_SIZE		(1 << PAGE_SHIFT)
#define	PAGE_MASK		(PAGE_SIZE - 1)

#define	VM_MIN_ADDRESS		((vaddr_t) PAGE_SIZE)
#define	VM_MAXUSER_ADDRESS	((vaddr_t) KERNEL_BASE - PAGE_SIZE)
#define	VM_MAX_ADDRESS		VM_MAXUSER_ADDRESS

#define	VM_MIN_KERNEL_ADDRESS	((vaddr_t) KERNEL_BASE)
#define	VM_MAX_KERNEL_ADDRESS	((vaddr_t) -1)

#define USRIOSIZE		300

#define VM_PHYS_SIZE		(USRIOSIZE * PAGE_SIZE)

#define	VM_PHYSSEG_MAX		32
#define	VM_PHYSSEG_STRAT	VM_PSTRAT_BSEARCH

#define	VM_NFREELIST		1
#define	VM_FREELIST_DEFAULT	0

#endif /* _KERNEL || _KMEMUSER */

#endif /* _X86_64_VMPARAM_H_ */
