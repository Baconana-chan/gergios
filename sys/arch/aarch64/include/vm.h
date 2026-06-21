/* vm.h — AArch64 VM constants */
#ifndef _AARCH64_VM_H_
#define _AARCH64_VM_H_

/* ARM64 page size (4KB) */
#define ARM64_PAGE_SIZE         4096
#define ARM64_PAGE_SHIFT        12
#define AARCH64_PAGE_SIZE       ARM64_PAGE_SIZE
#define AARCH64_PAGE_SHIFT      ARM64_PAGE_SHIFT

/* ARM64 large page size (2MB) */
#define ARM64_LARGE_PAGE_SIZE   0x200000
#define ARM64_LARGE_PAGE_SHIFT  21
#define AARCH64_BIG_PAGE_SIZE   ARM64_LARGE_PAGE_SIZE
#define AARCH64_BIG_PAGE_SHIFT  ARM64_LARGE_PAGE_SHIFT

/* Page table directories */
#define AARCH64_VM_DIR_ENTRIES		512	/* Entries per L0/L1/L2/L3 table */
#define AARCH64_VM_PT_ENTRIES		512	/* Entries per L3 page table */

/* Page table sizes */
#define AARCH64_PAGEDIR_SIZE		(512 * 8)  /* 4096: L0/L1/L2 table */
#define AARCH64_PAGETABLE_SIZE		(512 * 8)  /* 4096: L3 page table */
#define AARCH64_PAGE_SIZE		ARM64_PAGE_SIZE

/* Page index helpers (2-level abstraction for VM server) */
/* PDE = L2 descriptor index (bits[29:21]), PTE = L3 (bits[20:12]) */
#define AARCH64_VM_PDE(v)		AARCH64_VM_L2E(v)
#define AARCH64_VM_PTE(v)		AARCH64_VM_L3E(v)

/* Address masks */
#define AARCH64_VM_ADDR_MASK		0x0000FFFFFFFFFFF000UL  /* Bits[47:12] */
#define AARCH64_VM_ADDR_MASK_2MB	0x0000FFFFFFFFFFE000UL  /* Bits[47:21] */
#define AARCH64_VM_OFFSET_MASK_2MB	0x001FFFFFULL          /* Offset within 2MB */

/* =========================================================================
 * AARCH64_VM_* constants (page table descriptor bits)
 *
 * ARMv8-A VMSAv8-64 page table entry format:
 *   Bits [1:0]:  Descriptor type
 *     00 → Invalid
 *     01 → Page (L3) or Block (L0-L2)
 *     11 → Page (L3 only)
 *     10 → Table (L0-L2)
 *
 *   Bit 1 = 0 → Block descriptor (at L0-L2)
 *   Bit 1 = 1 → Table descriptor (at L0-L2) or Page (at L3)
 * ========================================================================= */

/* Descriptor type bits */
#define AARCH64_VM_PRESENT		(1UL << 0)   /* Page is valid/present */
#define AARCH64_VM_TABLE		(1UL << 1)   /* Table descriptor (L0-L2) */
#define AARCH64_VM_BLOCK		(0)	   /* Block type (bit 1=0 at L0-L2) */
#define AARCH64_VM_PAGE			(3UL << 0)   /* Page type (bits[1:0]=11) at L3 */

/* Memory attributes (stage 1) */
/* AttrIndx[2:0] = bits[4:2] → index into MAIR_EL1 */
#define AARCH64_VM_ATTR(x)		((x) << 2)
#define AARCH64_VM_NORMAL		AARCH64_VM_ATTR(0)  /* Attr0: Normal WB */
#define AARCH64_VM_DEVICE		AARCH64_VM_ATTR(1)  /* Attr1: Device */

/* Shareability field (bits[11:10]) */
#define AARCH64_VM_SHIFT_SH		10
#define AARCH64_VM_SH(x)		((x) << AARCH64_VM_SHIFT_SH)
#define AARCH64_VM_SH_NON		AARCH64_VM_SH(0)  /* Non-shareable */
#define AARCH64_VM_SH_OUTER		AARCH64_VM_SH(1)  /* Outer shareable */
#define AARCH64_VM_SH_INNER		AARCH64_VM_SH(2)  /* Inner shareable */
#define AARCH64_VM_SH_IS		AARCH64_VM_SH_INNER

/* Access Flag (bit 10) */
#define AARCH64_VM_AF			(1UL << 10)

/* Access permissions (bits[7:6]) */
#define AARCH64_VM_AP_SHIFT		6
#define AARCH64_VM_AP_RW_EL1		(0UL << AARCH64_VM_AP_SHIFT)  /* R/W EL1 only */
#define AARCH64_VM_AP_RW_ALL		(1UL << AARCH64_VM_AP_SHIFT)  /* R/W EL1/EL0 */
#define AARCH64_VM_AP_RO_EL1		(2UL << AARCH64_VM_AP_SHIFT)  /* R/O EL1 only */
#define AARCH64_VM_AP_RO_ALL		(3UL << AARCH64_VM_AP_SHIFT)  /* R/O EL1/EL0 */

/* User (EL0) access enable */
#define AARCH64_VM_USER			AARCH64_VM_AP_RW_ALL

/* Read/Write permission (within EL1) */
#define AARCH64_VM_RW			AARCH64_VM_AP_RW_EL1

/* PXN (bit 53) and UXN (bit 54) — execute never */
#define AARCH64_VM_PXN			(1UL << 53)
#define AARCH64_VM_UXN			(1UL << 54)

/* Address mask (bits[47:12] for 4KB granule) */
#define AARCH64_VM_ADDR_MASK		0x0000FFFFFFFFFFF000UL
#define AARCH64_VM_ADDR_MASK_2MB	0x0000FFFFFFFFFFE000UL

/* Page table level helpers */
#define AARCH64_VM_L0E(va)		(((va) >> 39) & 0x1FF)
#define AARCH64_VM_L1E(va)		(((va) >> 30) & 0x1FF)
#define AARCH64_VM_L2E(va)		(((va) >> 21) & 0x1FF)
#define AARCH64_VM_L3E(va)		(((va) >> 12) & 0x1FF)

/* =========================================================================
 * Page fault status (ESR_EL1 decoding)
 * ========================================================================= */
#define AARCH64_VM_PFE_W        (1 << 6)   /* Write (otherwise read) */
#define AARCH64_VM_PFE_FSC_MASK 0x3F       /* Fault Status Code mask */

#define AARCH64_VM_PFE_FSC(e)   ((e) & AARCH64_VM_PFE_FSC_MASK)

#ifndef __ASSEMBLY__
#include <minix/type.h>
#endif /* __ASSEMBLY__ */

#endif /* _AARCH64_VM_H_ */
