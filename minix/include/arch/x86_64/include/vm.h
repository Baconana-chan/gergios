#ifndef __SYS_VM_X86_64_H__
#define __SYS_VM_X86_64_H__

/*
 * x86_64/vm.h
 *
 * x86_64 IA-32e paging constants (4-level paging).
 *
 * Page table structure (4 levels):
 *   PML4 (Level 4) → PDP (Level 3) → PD (Level 2) → PT (Level 1)
 *   Each table: 512 entries × 8 bytes = 4096 bytes (one page)
 *
 * Virtual address (48-bit canonical):
 *  47      39 38      30 29      21 20      12 11       0
 * +--------+---------+---------+---------+-----------+
 * | PML4   |  PDP    |   PD    |   PT    |   OFFSET  |
 * +--------+---------+---------+---------+-----------+
 *   9 bits   9 bits    9 bits    9 bits    12 bits
 *
 * Page sizes:
 *   4KB (standard page)
 *   2MB (large page, PD entry with PS=1)
 *   1GB (huge page, PDP entry with PS=1)
 *
 * Physical address: up to 52 bits (bits 12-51 in PTE)
 * NX (No Execute) bit: bit 63
 */

#define X86_64_PAGE_SIZE		4096
#define X86_64_BIG_PAGE_SIZE		(2 * 1024 * 1024)  /* 2MB large page */

/* Page table entry flags (same bit positions as i386 for common bits) */
#define X86_64_VM_PRESENT	0x001	/* Page is present */
#define X86_64_VM_WRITE		0x002	/* Read/write access allowed */
#define X86_64_VM_READ		0x000	/* Read access only */
#define X86_64_VM_USER		0x004	/* User access allowed */
#define X86_64_VM_PWT		0x008	/* Write through */
#define X86_64_VM_PCD		0x010	/* Cache disable */
#define X86_64_VM_ACC		0x020	/* Accessed */
#define X86_64_VM_DIRTY		0x040	/* Dirty (page table only) */
#define X86_64_VM_BIGPAGE	0x080	/* 2MB page (PD entry) or 1GB (PDP) */
#define X86_64_VM_GLOBAL	0x100	/* Global page (CR4.PGE required) */
#define X86_64_VM_NX		(1ULL << 63)	/* No Execute */

/* Physical address mask: bits 12-51 (40 bits) */
#define X86_64_VM_ADDR_MASK		0x000FFFFFFFFFF000ULL
#define X86_64_VM_ADDR_MASK_2MB		0x000FFFFFFFE00000ULL
#define X86_64_VM_OFFSET_MASK_2MB	0x001FFFFFULL

/* Page table entry size and counts */
#define X86_64_VM_PT_ENT_SIZE	8	/* 8 bytes per entry (64-bit) */
#define X86_64_VM_DIR_ENT_SIZE	8	/* 8 bytes per directory entry */

#define X86_64_VM_DIR_ENTRIES	512	/* Entries per PML4/PDP/PD/PT */
#define X86_64_VM_DIR_ENT_SHIFT	39	/* PML4 index: bits 39-47 */
#define X86_64_VM_PDP_ENT_SHIFT	30	/* PDP index: bits 30-38 */
#define X86_64_VM_PD_ENT_SHIFT	21	/* PD index: bits 21-29 */
#define X86_64_VM_PT_ENT_SHIFT	12	/* PT index: bits 12-20 */
#define X86_64_VM_PT_ENT_MASK	0x1FF	/* 9-bit mask for each level */
#define X86_64_VM_PT_ENTRIES	512	/* Entries per page table */

/* For compatibility with 2-level page table abstraction used by MINIX:
 * The VM server manages PD (Level 2) and PT (Level 1) entries directly.
 * PML4 and PDP are set up during boot and remain mostly static.
 *
 * PDE on x86_64 = Page Directory entry (Level 2): maps 2MB per entry
 * PTE on x86_64 = Page Table entry (Level 1): maps 4KB per entry
 */
#define X86_64_VM_PFA_SHIFT	12	/* Page frame address shift */

/* Page fault error code bits */
#define X86_64_VM_PFE_P	0x01	/* Non-present page fault */
#define X86_64_VM_PFE_W	0x02	/* Caused by write (otherwise read) */
#define X86_64_VM_PFE_U	0x04	/* CPU in user mode (otherwise supervisor) */
#define X86_64_VM_PFE_RSVD	0x08	/* Reserved bit violation */
#define X86_64_VM_PFE_I	0x10	/* Instruction fetch (NX violation) */

/* CR0 bits */
#define X86_64_CR0_PE		0x00000001	/* Protected mode */
#define X86_64_CR0_MP		0x00000002	/* Monitor Coprocessor */
#define X86_64_CR0_EM		0x00000004	/* Emulate */
#define X86_64_CR0_TS		0x00000008	/* Task Switched */
#define X86_64_CR0_ET		0x00000010	/* Extension Type */
#define X86_64_CR0_NE		0x00000020	/* Numeric Error */
#define X86_64_CR0_WP		0x00010000	/* Write Protect */
#define X86_64_CR0_PG		0x80000000	/* Enable paging */

/* CR4 bits */
#define X86_64_CR4_PAE		0x00000020	/* Physical Address Extension */
#define X86_64_CR4_PGE		0x00000080	/* Global page flag enable */
#define X86_64_CR4_PCIDE	0x00020000	/* PCID enable */

/* CPUID flags */
#define CPUID1_EDX_FPU		(1L)		/* FPU presence */
#define CPUID1_EDX_PSE		(1L << 3)	/* Page Size Extension */
#define CPUID1_EDX_SYSENTER	(1L << 11)	/* Intel SYSENTER */
#define CPUID1_EDX_PAE		(1L << 6)	/* Physical Address Extension */
#define CPUID1_EDX_PGE		(1L << 13)	/* Page Global Enable */
#define CPUID1_EDX_APIC_ON_CHIP (1L << 9)	/* APIC on chip */
#define CPUID1_EDX_TSC		(1L << 4)	/* Timestamp counter */
#define CPUID1_EDX_HTT		(1L << 28)	/* Hyper-Threading */
#define CPUID1_EDX_FXSR		(1L << 24)
#define CPUID1_EDX_SSE		(1L << 25)
#define CPUID1_EDX_SSE2		(1L << 26)
#define CPUID1_ECX_SSE3		(1L)
#define CPUID1_ECX_SSSE3	(1L << 9)
#define CPUID1_ECX_SSE4_1	(1L << 19)
#define CPUID1_ECX_SSE4_2	(1L << 20)

/* Page table index calculation macros (2-level abstraction) */
#define X86_64_VM_PTE(v)	(((v) >> X86_64_VM_PT_ENT_SHIFT) & X86_64_VM_PT_ENT_MASK)
#define X86_64_VM_PDE(v)	(((v) >> X86_64_VM_PD_ENT_SHIFT) & X86_64_VM_PT_ENT_MASK)
#define X86_64_VM_PFA(e)	((e) & X86_64_VM_ADDR_MASK)

/* Full 4-level page table walk macros (for kernel MM setup) */
#define X86_64_VM_PML4E(v)	(((v) >> X86_64_VM_DIR_ENT_SHIFT) & X86_64_VM_PT_ENT_MASK)
#define X86_64_VM_PDPE(v)	(((v) >> X86_64_VM_PDP_ENT_SHIFT) & X86_64_VM_PT_ENT_MASK)

#ifndef __ASSEMBLY__

#include <minix/type.h>

/* structure used by VM to pass data to the kernel while enabling paging */
struct vm_ep_data {
	struct mem_map	* mem_map;
	vir_bytes	data_seg_limit;
};

#endif /* __ASSEMBLY__ */

#endif /* __SYS_VM_X86_64_H__ */
