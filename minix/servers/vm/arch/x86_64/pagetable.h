#ifndef _PAGETABLE_H
#define _PAGETABLE_H 1

#include <stdint.h>
#include <machine/vm.h>

#include "vm.h"

/*
 * x86_64 pagetable abstraction for MINIX VM server.
 *
 * Uses a 2-level abstraction on top of the 4-level x86_64 paging:
 *   PDE = PD entry (Level 2): 512 entries, each maps 2MB
 *   PTE = PT entry (Level 1): 512 entries per PD, each maps 4KB
 *
 * PML4 (Level 4) and PDP (Level 3) are set up once during boot
 * and are managed by the kernel, not by the VM server.
 *
 * For the user address space (lower half):
 *   PML4[0] → PDP[0] → PD[0..511] for user memory
 *   PML4[256] → kernel_mapping_PDP (shared across all processes)
 *
 * Page table entries are 64-bit (8 bytes) on x86_64.
 */

/* 64-bit page table entry type for x86_64 */
typedef u64_t pt_entry_t;

/* Mapping flags. */
#define PTF_WRITE	X86_64_VM_WRITE
#define PTF_READ	X86_64_VM_READ
#define PTF_PRESENT	X86_64_VM_PRESENT
#define PTF_USER	X86_64_VM_USER
#define PTF_GLOBAL	X86_64_VM_GLOBAL
#define PTF_NOCACHE	(X86_64_VM_PWT | X86_64_VM_PCD)

#define ARCH_VM_DIR_ENTRIES	X86_64_VM_DIR_ENTRIES   /* 512 */
#define ARCH_BIG_PAGE_SIZE	X86_64_BIG_PAGE_SIZE    /* 2MB */
#define ARCH_VM_ADDR_MASK	X86_64_VM_ADDR_MASK     /* 52-bit phys addr mask */
#define ARCH_VM_PAGE_PRESENT	X86_64_VM_PRESENT
#define ARCH_VM_PDE_MASK	X86_64_VM_ADDR_MASK
#define ARCH_VM_PDE_PRESENT	X86_64_VM_PRESENT
#define ARCH_VM_PTE_PRESENT	X86_64_VM_PRESENT
#define ARCH_VM_PTE_USER	X86_64_VM_USER
#define ARCH_VM_PTE_RW		X86_64_VM_WRITE
#define ARCH_PAGEDIR_SIZE	X86_64_PAGE_SIZE        /* 4096 */
#define ARCH_VM_BIGPAGE		X86_64_VM_BIGPAGE       /* 0x080 (PS bit) */
#define ARCH_VM_PT_ENTRIES	X86_64_VM_PT_ENTRIES    /* 512 */

/* For arch-specific PT routines to check if no bits outside
 * the regular flags are set.
 */
#define PTF_ALLFLAGS   (PTF_READ|PTF_WRITE|PTF_PRESENT|PTF_USER|PTF_GLOBAL|PTF_NOCACHE)

/* x86_64 page fault error code decoding */
#define PFERR_NOPAGE(e)	(!((e) & X86_64_VM_PFE_P))
#define PFERR_PROT(e)	(((e) & X86_64_VM_PFE_P))
#define PFERR_WRITE(e)	((e) & X86_64_VM_PFE_W)
#define PFERR_READ(e)	(!((e) & X86_64_VM_PFE_W))

#define VM_PAGE_SIZE	X86_64_PAGE_SIZE    /* 4096 */

/* virtual address -> pde, pte macros */
#define ARCH_VM_PTE(v)	X86_64_VM_PTE(v)
#define ARCH_VM_PDE(v)	X86_64_VM_PDE(v)

#endif
