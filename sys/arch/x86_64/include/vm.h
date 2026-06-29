/*	x86_64 vm.h — VM constants	*/

#ifndef _X86_64_VM_H_
#define _X86_64_VM_H_

/* x86_64 page size (4KB) */
#define X86_64_PAGE_SIZE        4096
#define X86_64_PAGE_SHIFT       12

/* x86_64 large page size (2MB) */
#define X86_64_LARGE_PAGE_SIZE  0x200000
#define X86_64_LARGE_PAGE_SHIFT 21

/* Page table (4-level paging) */
#define X86_64_VM_DIR_ENTRIES		512
#define X86_64_VM_PT_ENTRIES		512

#define X86_64_PAGEDIR_SIZE		(512 * 8)
#define X86_64_PAGETABLE_SIZE		(512 * 8)

/* Page index helpers (2-level abstraction for VM server) */
#define X86_64_VM_PDE(v)		(((v) >> 21) & 0x1FF)
#define X86_64_VM_PTE(v)		(((v) >> 12) & 0x1FF)

/* CR0 bits (x86_64 naming) */
#define X86_64_CR0_TS		0x00000008	/* Task Switched */

/* Address masks */
#define X86_64_VM_ADDR_MASK		0x0000FFFFFFFFFFF000UL
#define X86_64_VM_ADDR_MASK_2MB	0x0000FFFFFFFFFFE000UL

/* Page table entry bits (kernel-compatible names) */
#define X86_64_VM_PRESENT		(1UL << 0)
#define X86_64_VM_WRITE			(1UL << 1)	/* alias: X86_64_VM_RW */
#define X86_64_VM_RW			X86_64_VM_WRITE
#define X86_64_VM_USER			(1UL << 2)
#define X86_64_VM_PWT			(1UL << 3)
#define X86_64_VM_PCD			(1UL << 4)
#define X86_64_VM_ACC			(1UL << 5)	/* alias: X86_64_VM_ACCESSED */
#define X86_64_VM_ACCESSED		X86_64_VM_ACC
#define X86_64_VM_DIRTY			(1UL << 6)
#define X86_64_VM_BIGPAGE		(1UL << 7)	/* alias: X86_64_VM_PAT */
#define X86_64_VM_PAT			X86_64_VM_BIGPAGE
#define X86_64_VM_GLOBAL		(1UL << 8)

/* Big page size (2MB) */
#define X86_64_BIG_PAGE_SIZE		(2 * 1024 * 1024)

/* 2MB offset mask: bits 0-20 */
#define X86_64_VM_OFFSET_MASK_2MB	((1UL << 21) - 1)

/* PFA (Page Frame Address) — extract physical address from PTE */
#define X86_64_VM_PFA(e)		((e) & X86_64_VM_ADDR_MASK)

/* CR0 bits */
#define X86_64_CR0_PE		0x00000001	/* Protected Mode */
#define X86_64_CR0_TS		0x00000008	/* Task Switched */
#define X86_64_CR0_WP		0x00010000	/* Write Protect */
#define X86_64_CR0_PG		0x80000000	/* Enable Paging */

/* CR4 bits */
#define X86_64_CR4_PAE		0x00000020	/* Physical Address Extension */
#define X86_64_CR4_PGE		0x00000080	/* Page Global Enable */

/* Page fault error code bits */
#define X86_64_VM_PFE_P		(1 << 0)   /* Protection fault */
#define X86_64_VM_PFE_W		(1 << 1)   /* Write (otherwise read) */
#define X86_64_VM_PFE_U		(1 << 2)   /* User-mode access */
#define X86_64_VM_PFE_RSVD	(1 << 3)   /* Reserved bit violation */
#define X86_64_VM_PFE_I		(1 << 4)   /* Instruction fetch */

#ifndef __ASSEMBLY__
#include <minix/type.h>
#endif /* __ASSEMBLY__ */

#endif /* _X86_64_VM_H_ */
