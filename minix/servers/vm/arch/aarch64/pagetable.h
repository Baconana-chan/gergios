#ifndef _PAGETABLE_H
#define _PAGETABLE_H 1

#include <stdint.h>
#include <machine/vm.h>

#include "vm.h"

/*
 * ARM64 pagetable abstraction for MINIX VM server.
 *
 * Uses a 2-level abstraction on top of the 4-level ARM64 paging:
 *   PDE = L2 (Level 2) descriptor: maps 2MB per entry (block descriptor)
 *         or points to L3 table (table descriptor)
 *   PTE = L3 (Level 3) descriptor: maps 4KB per entry
 *
 * L0 and L1 are set up once during boot and managed by the kernel,
 * not by the VM server (same as PML4/PDP on x86_64).
 *
 * Page table entries are 64-bit (8 bytes) on ARM64.
 */

/* 64-bit page table entry type for ARM64 */
typedef u64_t pt_entry_t;

/* Mapping flags exposed to the VM server */
#define PTF_WRITE	AARCH64_VM_RW		/* 0 (RW is default, RO is explicit) */
#define PTF_READ	AARCH64_VM_RO		/* Read-only bit */
#define PTF_PRESENT	AARCH64_VM_PRESENT	/* Page is present/valid */
#define PTF_USER	AARCH64_VM_USER		/* User space accessible */
#define PTF_CACHEWB	AARCH64_VM_CACHED	/* Normal write-back cached */
#define PTF_NOCACHE	AARCH64_VM_DEVICE	/* Device-nGnRE (uncached) */
#define PTF_SHARE	AARCH64_VM_SH_IS	/* Inner shareable */

/* Architecture constants */
#define ARCH_VM_DIR_ENTRIES	AARCH64_VM_DIR_ENTRIES		/* 512 */
#define ARCH_BIG_PAGE_SIZE	AARCH64_BIG_PAGE_SIZE		/* 2MB */
#define ARCH_VM_ADDR_MASK	AARCH64_VM_ADDR_MASK		/* 48-bit phys addr mask */
#define ARCH_VM_PDE_MASK	AARCH64_VM_ADDR_MASK		/* Same for PDE */
#define ARCH_VM_PDE_PRESENT	AARCH64_VM_PRESENT		/* Valid bit */
#define ARCH_VM_PTE_PRESENT	AARCH64_VM_PRESENT		/* Valid bit */
#define ARCH_VM_PTE_USER	AARCH64_VM_USER			/* User access bit */
#define ARCH_VM_PTE_RW		AARCH64_VM_RW			/* RW (default) */
#define ARCH_VM_PTE_RO		AARCH64_VM_RO			/* RO bit */
#define ARCH_PAGEDIR_SIZE	AARCH64_PAGEDIR_SIZE		/* 4096 bytes */
#define ARCH_VM_PT_ENTRIES	AARCH64_VM_PT_ENTRIES		/* 512 */
#define ARCH_VM_ADDR_MASK_2MB	AARCH64_VM_ADDR_MASK_2MB	/* 2MB-aligned phys addr mask */

/*
 * BIGPAGE handling for ARM64:
 *
 * ARM64 L2 block descriptors have bit 1 = 0 (block type),
 * while table descriptors have bit 1 = 1 (table type).
 *
 * This is the INVERSE of ARM32 where section descriptors have bit 1 = 1.
 *
 * Since ARCH_VM_BIGPAGE is used as a positive check:
 *   if((entry & ARCH_VM_BIGPAGE)) continue;  // Skip big pages
 *
 * For ARM64, we set ARCH_VM_BIGPAGE = 0 and check for blocks via
 * the TABLE bit: a block entry is one where bit 1 is NOT set.
 * The pagetable.c code has __aarch64__ specific paths to handle this.
 */
#define ARCH_VM_BIGPAGE		0	/* Block detected via !AARCH64_VM_TABLE */

/* All valid PTF flags combined */
#define PTF_ALLFLAGS	(PTF_READ | PTF_WRITE | PTF_PRESENT | PTF_USER | \
			 PTF_CACHEWB | PTF_NOCACHE | PTF_SHARE)

/* ARM64 page fault error code decoding (ESR_EL1) */
#define PFERR_NOPAGE(e)		AARCH64_FSC_IS_TRANS(AARCH64_VM_PFE_FSC(e))
#define PFERR_PROT(e)		(!PFERR_NOPAGE(e))  /* Protection or access flag */
#define PFERR_WRITE(e)		((e) & AARCH64_VM_PFE_W)
#define PFERR_READ(e)		(!((e) & AARCH64_VM_PFE_W))

#define VM_PAGE_SIZE		AARCH64_PAGE_SIZE		/* 4096 */

/* Virtual address -> PDE, PTE index calculation */
#define ARCH_VM_PTE(v)		AARCH64_VM_PTE(v)
#define ARCH_VM_PDE(v)		AARCH64_VM_PDE(v)

#endif /* _PAGETABLE_H */
