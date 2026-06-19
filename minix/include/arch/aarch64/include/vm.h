#ifndef __SYS_VM_AARCH64_H__
#define __SYS_VM_AARCH64_H__

/*
 * aarch64/vm.h
 *
 * ARM64 VMSAv8-64 paging constants (4-level translation).
 *
 * Page table structure (4 levels, 4KB pages, 48-bit VA):
 *   L0 (Level 0) → L1 (Level 1) → L2 (Level 2) → L3 (Level 3)
 *   Each table: 512 entries x 8 bytes = 4096 bytes (one page)
 *
 * Virtual address (48-bit canonical):
 *  47      39 38      30 29      21 20      12 11       0
 * +--------+---------+---------+---------+-----------+
 * |  L0    |   L1    |   L2    |   L3    |   OFFSET  |
 * +--------+---------+---------+---------+-----------+
 *   9 bits   9 bits    9 bits    9 bits    12 bits
 *
 * Page sizes:
 *   4KB (standard page, L3)
 *   2MB (large page / block, L2 descriptor with type=Block)
 *   1GB (huge page, L1 descriptor with type=Block)
 *
 * The VM server uses a 2-level abstraction (same as x86_64):
 *   PDE = L2 descriptor (maps 2MB per entry, or points to L3 table)
 *   PTE = L3 descriptor (maps 4KB per entry)
 *   L0 and L1 are set up during boot and managed by the kernel.
 *
 * Physical address: up to 48 bits (bits 47:12 in PTE)
 * PXN (Privileged Execute Never): bit 53
 * UXN (User Execute Never): bit 54
 *
 * Reference: ARM Architecture Reference Manual ARMv8-A (DDI 0487)
 */

/* Page sizes */
#define AARCH64_PAGE_SIZE		4096
#define AARCH64_BIG_PAGE_SIZE		(2 * 1024 * 1024)  /* 2MB block (L2) */

/* =========================================================================
 * Page table entry flags (L3 page descriptor)
 * =========================================================================
 *
 * ARM64 L3 page descriptor format:
 *   Bit  0 : Valid (1 = valid)
 *   Bit  1 : Type (1 = page descriptor)
 *   Bits[4:2] : AttrIndx (index into MAIR_EL1)
 *   Bit  5 : NS (Non-secure)
 *   Bit  6 : AP[1] (Access Permission bit 1: 1 = EL0 accessible)
 *   Bit  7 : AP[2] (Access Permission bit 2: 1 = Read-only)
 *   Bits[9:8] : SH[1:0] (Shareability)
 *   Bit 10 : AF (Access Flag — must be 1 for access)
 *   Bit 11 : nG (Not Global)
 *   Bits[47:12] : Output address (physical page frame)
 *   Bit 50 : Contiguous hint
 *   Bit 51 : DBM (Dirty Bit Modifier)
 *   Bit 53 : PXN (Privileged Execute Never)
 *   Bit 54 : UXN (User Execute Never)
 *   Bits[55:62] : Software available
 */

/* Core PTE flags */
#define AARCH64_VM_PRESENT		(1UL << 0)   /* Page is valid/present */
#define AARCH64_VM_PAGE			(1UL << 1)   /* Page descriptor (L3) */
#define AARCH64_VM_TABLE		(1UL << 1)   /* Table descriptor (L0-L2) */
#define AARCH64_VM_BLOCK		(0)	   /* Block type (bit 1=0 at L0-L2) */

/* AttrIndx — index into MAIR_EL1 (3 bits at [4:2]) */
#define AARCH64_VM_ATTR_SHIFT		2
#define AARCH64_VM_ATTR_MASK		(7UL << 2)
#define AARCH64_VM_ATTR(n)		((n) << 2)
#define AARCH64_VM_NORMAL		AARCH64_VM_ATTR(0)  /* Attr0: Normal WB */
#define AARCH64_VM_DEVICE		AARCH64_VM_ATTR(1)  /* Attr1: Device-nGnRE */

/* Shareability (SH[1:0] at [9:8]) */
#define AARCH64_VM_SH_SHIFT		8
#define AARCH64_VM_SH_NON		0UL
#define AARCH64_VM_SH_OUTER		(2UL << 8)   /* Outer Shareable */
#define AARCH64_VM_SH_INNER		(3UL << 8)   /* Inner Shareable */
#define AARCH64_VM_SH_IS		AARCH64_VM_SH_INNER

/* Access Permission (AP[2:1] at [7:6])
 *   AP[2] = bit 7:  0=RW, 1=RO
 *   AP[1] = bit 6:  0=EL0 no access, 1=EL0 accessible
 *
 * EL1 R/W, EL0 no access:	AP[2:1] = 00  → (0)
 * EL1 R/W, EL0 R/W:		AP[2:1] = 01  → USER
 * EL1 R/O, EL0 no access:	AP[2:1] = 10  → RO
 * EL1 R/O, EL0 R/O:		AP[2:1] = 11  → USER | RO
 */
#define AARCH64_VM_USER			(1UL << 6)   /* AP[1]: User access */
#define AARCH64_VM_RO			(1UL << 7)   /* AP[2]: Read-only */

/* Access Flag (AF at bit 10 — must be 1) */
#define AARCH64_VM_AF			(1UL << 10)

/* Not Global (nG at bit 11 — 0 = global, 1 = not global) */
#define AARCH64_VM_NG			(1UL << 11)

/* Execute Never */
#define AARCH64_VM_PXN			(1UL << 53)  /* Privileged Execute Never */
#define AARCH64_VM_UXN			(1UL << 54)  /* User Execute Never */

/* Software-available bits (for MINIX use) */
#define AARCH64_VM_SW0			(1UL << 55)
#define AARCH64_VM_SW1			(1UL << 56)
#define AARCH64_VM_SW2			(1UL << 57)
#define AARCH64_VM_SW3			(1UL << 58)

/* =========================================================================
 * Composite flags for MINIX VM
 * ========================================================================= */

/* Writable by default (AP[2]=0), RO explicitly set */
#define AARCH64_VM_RW			0

/* Cached = Normal memory with Inner-Shareable Write-Back */
#define AARCH64_VM_CACHED		(AARCH64_VM_NORMAL | AARCH64_VM_SH_IS | \
					 AARCH64_VM_AF)

/* Uncached = Device-nGnRE memory */
#define AARCH64_VM_DEVICE_NG		(AARCH64_VM_DEVICE | AARCH64_VM_AF)

/* Default flags for a user page */
#define AARCH64_VM_UFLAGS		(AARCH64_VM_PRESENT | AARCH64_VM_PAGE | \
					 AARCH64_VM_AF | AARCH64_VM_CACHED | \
					 AARCH64_VM_USER | AARCH64_VM_RW)

/* =========================================================================
 * Address masks
 * ========================================================================= */

/* Physical address mask: bits 47:12 (48-bit PA, 4KB page-aligned) */
#define AARCH64_VM_ADDR_MASK		0x0000FFFFFFFFF000ULL

/* Physical address mask for 2MB-aligned addresses: bits 47:21 */
#define AARCH64_VM_ADDR_MASK_2MB	0x0000FFFFFFE00000ULL

/* Offset within a 2MB page */
#define AARCH64_VM_OFFSET_MASK_2MB	0x001FFFFFULL

/* =========================================================================
 * Page table entry sizes and counts
 * ========================================================================= */

#define AARCH64_VM_PT_ENT_SIZE		8	/* 8 bytes per entry (64-bit) */
#define AARCH64_VM_DIR_ENT_SIZE		8	/* 8 bytes per directory entry */

#define AARCH64_VM_DIR_ENTRIES		512	/* Entries per L0/L1/L2/L3 table */
#define AARCH64_VM_DIR_ENT_SHIFT	39	/* L0 index: bits 39-47 */
#define AARCH64_VM_L1_ENT_SHIFT		30	/* L1 index: bits 30-38 */
#define AARCH64_VM_L2_ENT_SHIFT		21	/* L2 (PDE) index: bits 21-29 */
#define AARCH64_VM_PT_ENT_SHIFT		12	/* L3 (PTE) index: bits 12-20 */
#define AARCH64_VM_PT_ENT_MASK		0x1FF	/* 9-bit mask for each level */
#define AARCH64_VM_PT_ENTRIES		512	/* Entries per L3 page table */

/* =========================================================================
 * PDE/PTE index calculation (2-level abstraction for VM server)
 *
 * The VM server manages L2 (PDE) and L3 (PTE) entries:
 *   PDE = L2 descriptor index: bits [29:21]
 *   PTE = L3 descriptor index: bits [20:12]
 * ========================================================================= */

#define AARCH64_VM_PTE(v)		(((v) >> AARCH64_VM_PT_ENT_SHIFT) & \
					 AARCH64_VM_PT_ENT_MASK)
#define AARCH64_VM_PDE(v)		(((v) >> AARCH64_VM_L2_ENT_SHIFT) & \
					 AARCH64_VM_PT_ENT_MASK)
#define AARCH64_VM_PFA(e)		((e) & AARCH64_VM_ADDR_MASK)

/* Full 4-level page table walk macros (for kernel MM setup) */
#define AARCH64_VM_L0E(v)		(((v) >> AARCH64_VM_DIR_ENT_SHIFT) & \
					 AARCH64_VM_PT_ENT_MASK)
#define AARCH64_VM_L1E(v)		(((v) >> AARCH64_VM_L1_ENT_SHIFT) & \
					 AARCH64_VM_PT_ENT_MASK)

/* =========================================================================
 * Page directory / table sizes
 * ========================================================================= */

#define AARCH64_VM_DIR_SIZE		(AARCH64_VM_DIR_ENTRIES * \
					 AARCH64_VM_DIR_ENT_SIZE)  /* 4096 */
#define AARCH64_PAGEDIR_SIZE		AARCH64_VM_DIR_SIZE
#define AARCH64_VM_PT_SIZE		(AARCH64_VM_PT_ENTRIES * \
					 AARCH64_VM_PT_ENT_SIZE)  /* 4096 */
#define AARCH64_PAGETABLE_SIZE		AARCH64_VM_PT_SIZE

/* =========================================================================
 * Page fault status (ESR_EL1 decoding)
 * =========================================================================
 *
 * ESR_EL1 for Data Abort:
 *   Bits[31:26]: Exception Class (0b100100 = Data Abort, same EL;
 *                                  0b100101 = Data Abort, lower EL)
 *   Bit  24: Instruction/Data fault indicator
 *   Bit  6 : WnR (Write not Read: 1 = write, 0 = read)
 *   Bits[5:0] : Fault Status Code (FSC)
 *
 * FSC values (for data/instruction aborts):
 *   0b0001xx (0x04-0x07): Translation fault (level xx)
 *   0b0010xx (0x08-0x0B): Access flag fault (level xx)
 *   0b0011xx (0x0C-0x0F): Permission fault (level xx)
 *   0b0100xx (0x10-0x13): Domain fault (level xx, FEAT)
 *   0b0101xx (0x14-0x17): Address size fault (level xx)
 *   0b1001xx (0x24-0x25): Synchronous external abort (translation)
 */

#define AARCH64_VM_PFE_W		(1 << 6)   /* Write (otherwise read) */
#define AARCH64_VM_PFE_FSC_MASK		0x3F       /* Fault Status Code mask */

/* Extract fault status code from ESR_EL1 */
#define AARCH64_VM_PFE_FSC(e)		((e) & AARCH64_VM_PFE_FSC_MASK)

/* FSC base values for each fault type */
#define AARCH64_FSC_TRANS_BASE		0x04  /* Translation fault (level 0) */
#define AARCH64_FSC_ACCESS_BASE		0x08  /* Access flag fault (level 0) */
#define AARCH64_FSC_PERM_BASE		0x0C  /* Permission fault (level 0) */

/* Check if translation fault (any level) */
#define AARCH64_FSC_IS_TRANS(fsc)	\
	((fsc) >= AARCH64_FSC_TRANS_BASE && (fsc) < AARCH64_FSC_TRANS_BASE + 4)

/* Check if permission fault or access flag fault */
#define AARCH64_FSC_IS_PERM(fsc)	\
	(((fsc) >= AARCH64_FSC_ACCESS_BASE && (fsc) < AARCH64_FSC_ACCESS_BASE + 4) || \
	 ((fsc) >= AARCH64_FSC_PERM_BASE && (fsc) < AARCH64_FSC_PERM_BASE + 4))

#ifndef __ASSEMBLY__

#include <minix/type.h>

/* Structure used by VM to pass data to the kernel while enabling paging */
struct vm_ep_data {
	struct mem_map	* mem_map;
	vir_bytes	data_seg_limit;
};

#endif /* __ASSEMBLY__ */

#endif /* __SYS_VM_AARCH64_H__ */
