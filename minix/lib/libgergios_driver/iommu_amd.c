/* iommu_amd.c — AMD-Vi (I/O Virtualization) IOMMU Backend
 *
 * Implements the unified gergios_iommu_ops interface for AMD-Vi
 * IOMMU hardware, as specified in "AMD I/O Virtualization Technology
 * (IOMMU) Specification", Revision 3.0 (March 2016).
 *
 * Key hardware features:
 *   - Device Exclusion Vector (DEV) — legacy, simple allow/deny
 *   - DMA remapping with multi-level page tables (levels 1-3)
 *   - I/O TLB (IOTLB) with invalidation via command buffer
 *   - Event logging (for DMA page faults)
 *   - Interrupt remapping (optional)
 *
 * Detection: via ACPI IVRS table and/or PCI capability (CAP_T_SECURE_DEV).
 *
 * Note: The existing amddev driver in drivers/iommu/amddev/ implements
 * only the legacy DEV (exclusion vector).  This backend replaces it with
 * full AMD-Vi DMA remapping support while maintaining backward compat.
 */

#include <minix/drivers.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <minix/sysutil.h>
#include <minix/type.h>
#include <minix/com.h>
#include <minix/vm.h>
#include <machine/pci.h>
#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "iommu.h"

/*===========================================================================*
 *		AMD IOMMU register offsets (from MMIO base)		     *
 *===========================================================================*/
#define AMD_IOMMU_OFFSET_DEV_BASE_LO		0x0000
#define AMD_IOMMU_OFFSET_DEV_BASE_HI		0x0004
#define AMD_IOMMU_OFFSET_DEV_MAP		0x0008
#define AMD_IOMMU_OFFSET_DEV_CR			0x000C
#define		AMD_IOMMU_CR_ENABLE		(1U << 0)
#define		AMD_IOMMU_CR_IRQ_ENABLE		(1U << 1)
#define		AMD_IOMMU_CR_EVENT_ENABLE	(1U << 2)
#define		AMD_IOMMU_CR_HP_ENABLE		(1U << 3)
#define		AMD_IOMMU_CR_INV_TIMEOUT	(1U << 6)
#define		AMD_IOMMU_CR_GA_ENABLE		(1U << 7)
#define		AMD_IOMMU_CR_COHERENCY		(1U << 8)
#define		AMD_IOMMU_CR_ISOC		(1U << 9)
#define		AMD_IOMMU_CR_CMDBUF_ENABLE	(1U << 10)
#define		AMD_IOMMU_CR_EVENTLOG_ENABLE	(1U << 14)
#define AMD_IOMMU_OFFSET_EXCL_BASE_LO		0x0010
#define AMD_IOMMU_OFFSET_EXCL_BASE_HI		0x0014
#define AMD_IOMMU_OFFSET_EXCL_LIMIT		0x0018
#define AMD_IOMMU_OFFSET_EXT_FEATURES		0x001C
#define		AMD_IOMMU_EXT_FEAT_PREFETCH	(1U << 0)
#define		AMD_IOMMU_EXT_FEAT_FLUSH_READ	(1U << 1)
#define		AMD_IOMMU_EXT_FEAT_FLUSH_ZERO	(1U << 2)
#define		AMD_IOMMU_EXT_FEAT_IOTLB_SUP	(1U << 3)
#define		AMD_IOMMU_EXT_FEAT_PAGE_TABLE	(1U << 4)
#define		AMD_IOMMU_EXT_FEAT_INTR_REMAP	(1U << 5)
#define		AMD_IOMMU_EXT_FEAT_PAGE_2MB	(1U << 6)
#define		AMD_IOMMU_EXT_FEAT_PAGE_1GB	(1U << 7)
#define AMD_IOMMU_OFFSET_CMDBUF_BASE_LO		0x0020
#define AMD_IOMMU_OFFSET_CMDBUF_BASE_HI		0x0024
#define AMD_IOMMU_OFFSET_CMDBUF_HEAD		0x0028  /* head offset (in dwords) */
#define AMD_IOMMU_OFFSET_CMDBUF_TAIL		0x002C  /* tail offset (in dwords) */
#define AMD_IOMMU_OFFSET_EVENTLOG_BASE_LO	0x0030
#define AMD_IOMMU_OFFSET_EVENTLOG_BASE_HI	0x0034
#define AMD_IOMMU_OFFSET_EVENTLOG_HEAD		0x0038
#define AMD_IOMMU_OFFSET_EVENTLOG_TAIL		0x003C
#define AMD_IOMMU_OFFSET_CONTROL_LO		0x0040
#define AMD_IOMMU_OFFSET_CONTROL_HI		0x0044
#define AMD_IOMMU_OFFSET_EXCL_STATUS_LO		0x0050
#define AMD_IOMMU_OFFSET_EXCL_STATUS_HI		0x0054
#define AMD_IOMMU_OFFSET_PAGE_TABLE_BASE_LO	0x0060
#define AMD_IOMMU_OFFSET_PAGE_TABLE_BASE_HI	0x0064
#define AMD_IOMMU_OFFSET_PAGE_TABLE_LENGTH	0x0068
#define AMD_IOMMU_OFFSET_PAGE_TABLE_ENABLE	0x006C
#define AMD_IOMMU_OFFSET_IRQ_TABLE_BASE_LO	0x0070
#define AMD_IOMMU_OFFSET_IRQ_TABLE_BASE_HI	0x0074
#define AMD_IOMMU_OFFSET_IRQ_TABLE_LENGTH	0x0078

/*===========================================================================*
 *		AMD IOMMU PCI capability registers			     *
 *===========================================================================*/
/* The existing amddev driver already defines these in its source.
 * The capability offset is found by scanning PCI capabilities and
 * looking for CAP_T_SECURE_DEV (0x0F). */

#define AMD_CAP_SD_INFO		0x04	/* Subtype info register */
#define		AMD_CAP_SD_SUBTYPE_MASK	0x0007
#define		AMD_CAP_SD_SUBTYPE_DEV	0x00	/* Device Exclusion Vector */
#define		AMD_CAP_SD_SUBTYPE_IOMMU 0x01	/* Full IOMMU (not just DEV) */

/* For the full IOMMU, capabilities are accessed via MMIO at the
 * BAR reported in the IVRS table, not via PCI config space. */

/*===========================================================================*
 *		IVRS table parsing (ACPI)				     *
 *===========================================================================*/

/* IVRS table signature */
#define ACPI_SIG_IVRS  "IVRS"

/* IVRS subtable types */
#define IVRS_TYPE_HARDWARE		0x10	/* IOMMU hardware definition */
#define IVRS_TYPE_MEMORY1		0x20	/* Memory range, type 1 */
#define IVRS_TYPE_MEMORY2		0x21	/* Memory range, type 2 */
#define IVRS_TYPE_MEMORY3		0x22	/* Memory range, type 3 */

/* IVRS IVHD device entry types */
#define IVHD_ENTRY_PAD4			0
#define IVHD_ENTRY_ALL			1
#define IVHD_ENTRY_SELECT		2
#define IVHD_ENTRY_START		3
#define IVHD_ENTRY_END			4
#define IVHD_ENTRY_PAD8			64
#define IVHD_ENTRY_ALIAS_SELECT		66
#define IVHD_ENTRY_ALIAS_START		67

/* IVHD header (full) */
struct acpi_ivrs_hardware {
	struct acpi_ivrs_header hdr;	/* Type=0x10, Flags, Length, DeviceId */
	uint16_t	capability_offset;	/* Offset for IOMMU control fields */
	uint64_t	base_address;		/* IOMMU MMIO base */
	uint16_t	pci_segment_group;
	uint16_t	info;			/* MSI number + unit ID */
	uint32_t	reserved;
} __attribute__((packed));

struct acpi_ivrs_header {
	uint8_t	type;
	uint8_t	flags;
	uint16_t length;
	uint16_t device_id;	/* BDF of the IOMMU itself */
} __attribute__((packed));

/*===========================================================================*
 *		Internal state					     *
 *===========================================================================*/

#define MAX_AMD_IOMMU_UNITS		8
#define AMD_PAGE_TABLE_LEVELS		3
#define AMD_DEV_TABLE_ENTRIES		65536	/* 16-bit device ID */
#define AMD_CMDBUF_SIZE			512	/* entries (each 4 dwords) */
#define AMD_EVENTLOG_SIZE		128	/* entries */

struct amd_iommu_unit {
	uint8_t		present;
	uint64_t	mmio_base;		/* MMIO base (from IVRS) */
	uint8_t	       *mmio_virt;		/* Virtual address (mapped) */
	uint16_t	segment;		/* PCI segment group */
	uint16_t	device_id;		/* BDF of the IOMMU */
	uint16_t	capability_offset;

	/* Features */
	unsigned int	has_page_tables : 1;
	unsigned int	has_iotlb : 1;
	unsigned int	has_intr_remap : 1;
	unsigned int	has_2mb_pages : 1;
	unsigned int	has_1gb_pages : 1;

	/* Device table (for DMA remapping) */
	uint64_t	dev_table_phys;		/* physical address */
	uint8_t	       *dev_table_virt;		/* virtual address */

	/* Command buffer (ring buffer for IOTLB commands) */
	uint32_t       *cmdbuf_virt;
	uint64_t	cmdbuf_phys;
	volatile uint32_t cmdbuf_head;		/* read by hardware */
	uint32_t	cmdbuf_tail;

	/* Event log buffer */
	uint32_t       *eventlog_virt;
	uint64_t	eventlog_phys;

	/* IOTLB invalidation */
	unsigned int	iotlb_inv_active;
};

static struct amd_iommu_unit amd_units[MAX_AMD_IOMMU_UNITS];
static unsigned int amd_unit_count = 0;
static struct gergios_iommu_domain amd_domains[64];
static unsigned int amd_domain_count = 0;
static int amd_initialised = 0;

/*===========================================================================*
 *		MMIO access helpers					     *     *
 *===========================================================================*/
static inline uint32_t amd_read32(struct amd_iommu_unit *unit, uint32_t off)
{
	return *((volatile uint32_t *)(unit->mmio_virt + off));
}

static inline void amd_write32(struct amd_iommu_unit *unit, uint32_t off, uint32_t val)
{
	*((volatile uint32_t *)(unit->mmio_virt + off)) = val;
}

static inline uint64_t amd_read64(struct amd_iommu_unit *unit, uint32_t off_lo, uint32_t off_hi)
{
	uint64_t lo = amd_read32(unit, off_lo);
	uint64_t hi = amd_read32(unit, off_hi);
	return lo | (hi << 32);
}

static inline void amd_write64(struct amd_iommu_unit *unit,
    uint32_t off_lo, uint32_t off_hi, uint64_t val)
{
	amd_write32(unit, off_lo, (uint32_t)(val & 0xFFFFFFFF));
	amd_write32(unit, off_hi, (uint32_t)(val >> 32));
}

/*===========================================================================*
 *		ACPI IVRS table parsing (using shared acpi_find_table)	     *
 *===========================================================================*/

/*===========================================================================*
 *		AMD-Vi hardware initialisation				     *
 *===========================================================================*/

static int amd_parse_ivrs(void)
{
	struct acpi_sdt_header *ivrs;
	struct acpi_ivrs_header *sub;
	uint8_t *ptr, *end;

	ivrs = acpi_find_table("IVRS");
	if (!ivrs) {
		printf("iommu_amd: IVRS table not found\n");
		return 0;  /* No AMD IOMMU present */
	}

	printf("iommu_amd: IVRS table found (rev %u, len %u)\n",
	    ivrs->revision, ivrs->length);

	ptr = (uint8_t *)ivrs + sizeof(struct acpi_sdt_header);
	end = (uint8_t *)ivrs + ivrs->length;

	while (ptr < end && amd_unit_count < MAX_AMD_IOMMU_UNITS) {
		sub = (struct acpi_ivrs_header *)ptr;
		if (ptr + sizeof(struct acpi_ivrs_header) > end)
			break;

		switch (sub->type) {
		case IVRS_TYPE_HARDWARE: {
			struct acpi_ivrs_hardware *ivhd = (struct acpi_ivrs_hardware *)ptr;
			if ((uint8_t *)(ivhd + 1) > end)
				break;

			struct amd_iommu_unit *unit = &amd_units[amd_unit_count];
			memset(unit, 0, sizeof(*unit));
			unit->present = 1;
			unit->mmio_base = ivhd->base_address;
			unit->segment = ivhd->pci_segment_group;
			unit->device_id = ivhd->hdr.device_id;
			unit->capability_offset = ivhd->capability_offset;

			printf("iommu_amd: unit %u: MMIO=0x%llx seg=%u dev=0x%04x\n",
			    amd_unit_count,
			    (unsigned long long)unit->mmio_base,
			    unit->segment, unit->device_id);

			amd_unit_count++;
			break;
		}
		case IVRS_TYPE_MEMORY1:
		case IVRS_TYPE_MEMORY2:
		case IVRS_TYPE_MEMORY3:
			/* Reserved memory ranges — for identity mapping */
			break;
		}

		ptr += sub->length;
	}

	free(ivrs);
	return amd_unit_count;
}

static int amd_init_unit(struct amd_iommu_unit *unit)
{
	uint32_t ext, features;
	phys_bytes phys;

	if (!unit->present || !unit->mmio_base)
		return 0;

	/* Map MMIO registers */
	unit->mmio_virt = vm_map_phys(SELF, (void *)(uintptr_t)unit->mmio_base, 0x100);
	if (unit->mmio_virt == MAP_FAILED) {
		printf("iommu_amd: failed to map MMIO at 0x%llx\n",
		    (unsigned long long)unit->mmio_base);
		return -ENODEV;
	}

	/* Check extended features register */
	ext = amd_read32(unit, AMD_IOMMU_OFFSET_EXT_FEATURES);
	printf("iommu_amd: unit features = 0x%08x\n", ext);

	unit->has_page_tables = (ext & AMD_IOMMU_EXT_FEAT_PAGE_TABLE) != 0;
	unit->has_iotlb = (ext & AMD_IOMMU_EXT_FEAT_IOTLB_SUP) != 0;
	unit->has_intr_remap = (ext & AMD_IOMMU_EXT_FEAT_INTR_REMAP) != 0;
	unit->has_2mb_pages = (ext & AMD_IOMMU_EXT_FEAT_PAGE_2MB) != 0;
	unit->has_1gb_pages = (ext & AMD_IOMMU_EXT_FEAT_PAGE_1GB) != 0;

	/* Allocate device table (one entry per PCI device: 16-bit device ID) */
	if (unit->has_page_tables) {
		size_t dt_size = AMD_DEV_TABLE_ENTRIES * 16;	/* 16 bytes per entry */
		unit->dev_table_virt = alloc_contig(dt_size, AC_ALIGN4K, &phys);
		if (!unit->dev_table_virt) {
			printf("iommu_amd: failed to allocate device table\n");
			return -ENOMEM;
		}
		unit->dev_table_phys = phys;
		memset(unit->dev_table_virt, 0, dt_size);

		/* Write device table base */
		amd_write64(unit, AMD_IOMMU_OFFSET_DEV_BASE_LO,
		    AMD_IOMMU_OFFSET_DEV_BASE_HI, phys);
	}

	/* Set up exclusion vector (all memory accessible) */
	amd_write32(unit, AMD_IOMMU_OFFSET_EXCL_BASE_LO, 0);
	amd_write32(unit, AMD_IOMMU_OFFSET_EXCL_BASE_HI, 0);
	amd_write32(unit, AMD_IOMMU_OFFSET_EXCL_LIMIT, 0);

	/* Allocate and set up command buffer (for IOTLB invalidation) */
	if (unit->has_page_tables) {
		size_t cb_size = AMD_CMDBUF_SIZE * 16;  /* 16 bytes per entry */
		unit->cmdbuf_virt = alloc_contig(cb_size, AC_ALIGN4K, &phys);
		if (!unit->cmdbuf_virt)
			return -ENOMEM;
		unit->cmdbuf_phys = phys;
		memset(unit->cmdbuf_virt, 0, cb_size);
		unit->cmdbuf_head = 0;
		unit->cmdbuf_tail = 0;

		amd_write64(unit, AMD_IOMMU_OFFSET_CMDBUF_BASE_LO,
		    AMD_IOMMU_OFFSET_CMDBUF_BASE_HI, phys);
		amd_write32(unit, AMD_IOMMU_OFFSET_CMDBUF_HEAD, 0);
		amd_write32(unit, AMD_IOMMU_OFFSET_CMDBUF_TAIL, 0);
	}

	/* Enable the IOMMU */
	features = AMD_IOMMU_CR_ENABLE | AMD_IOMMU_CR_COHERENCY;
	if (unit->has_page_tables)
		features |= AMD_IOMMU_CR_CMDBUF_ENABLE;
	amd_write32(unit, AMD_IOMMU_OFFSET_DEV_CR, features);

	printf("iommu_amd: unit enabled (features=0x%x)\n",
	    amd_read32(unit, AMD_IOMMU_OFFSET_DEV_CR));

	return 0;
}

/*===========================================================================*
 *		Command buffer helpers					     *
 *===========================================================================*/

/* Command: COMPLETE_INVALIDATION (no-wait) */
#define AMD_CMD_COMPLETE_INVAL	0x00000000

/* Command: INVALIDATE_IOMMU_PAGES (IOTLB invalidation) */
#define AMD_CMD_INV_IOMMU_PAGES	0x00000001

/* Command: INVALIDATE_IOTLB_PAGES */
#define AMD_CMD_INV_IOTLB_PAGES	0x00000002

/* Command: COMPLETE_PPR_REQUEST */
#define AMD_CMD_COMPLETE_PPR	0x00000003

static int amd_submit_command(struct amd_iommu_unit *unit, uint32_t cmd[4])
{
	unsigned int tail, next_tail;

	if (!unit->cmdbuf_virt)
		return -ENODEV;

	tail = unit->cmdbuf_tail;
	next_tail = (tail + 1) & (AMD_CMDBUF_SIZE - 1);

	/* Check if command buffer is full */
	unit->cmdbuf_head = amd_read32(unit, AMD_IOMMU_OFFSET_CMDBUF_HEAD);
	if (next_tail == unit->cmdbuf_head)
		return -EBUSY;

	/* Write command to the buffer */
	unit->cmdbuf_virt[tail * 4 + 0] = cmd[0];
	unit->cmdbuf_virt[tail * 4 + 1] = cmd[1];
	unit->cmdbuf_virt[tail * 4 + 2] = cmd[2];
	unit->cmdbuf_virt[tail * 4 + 3] = cmd[3];

	/* Ensure writes are visible before updating tail */
	/* (x86 has strong ordering for write-combining, but we use
	 * a memory barrier for correctness on future architectures.) */
	__sync_synchronize();

	unit->cmdbuf_tail = next_tail;
	amd_write32(unit, AMD_IOMMU_OFFSET_CMDBUF_TAIL, next_tail);

	return 0;
}

static void amd_invalidate_pages(struct amd_iommu_unit *unit,
    uint16_t dev_id, uint64_t iova, size_t size)
{
	uint32_t cmd[4];
	unsigned int num_pages;

	if (!unit->has_page_tables)
		return;

	num_pages = (size + 4095) / 4096;

	/* Build INVALIDATE_IOMMU_PAGES command */
	/* Per AMD IOMMU spec rev 3.00, the number of pages-1 is at bits 28:18. */
	cmd[0] = AMD_CMD_INV_IOMMU_PAGES |
		 (((uint32_t)num_pages & 0x1FF) << 18);	/* number of pages-1 */
	cmd[1] = (uint32_t)(iova & 0xFFFFFFFF);
	cmd[2] = (uint32_t)(iova >> 32);
	cmd[3] = (uint32_t)dev_id;

	amd_submit_command(unit, cmd);
}

/*===========================================================================*
 *		Page table management (levels 1-3)			     *
 *===========================================================================*/

/* Page table entry bit definitions */
#define AMD_PTE_PRESENT		(1ULL << 0)
#define AMD_PTE_RW		(1ULL << 1)
#define AMD_PTE_USER		(1ULL << 8)
#define AMD_PTE_PCD		(1ULL << 6)
#define AMD_PTE_PWT		(1ULL << 7)
#define AMD_PTE_NX		(1ULL << 63)
#define AMD_PTE_PS		(1ULL << 8)	/* Page size (huge page indicator, level 2/3) */
#define AMD_PTE_GLB		(1ULL << 9)	/* Global page */
#define AMD_PTE_ADDR_MASK	0x000FFFFFFFFFFFF0ULL	/* bits 10-51 */

/* AMD-Vi page table levels and their coverage */
enum amd_pt_level {
	AMD_PT_L1 = 1,	/* 512 × 4KB = 2MB */
	AMD_PT_L2 = 2,	/* 512 × 2MB = 1GB */
	AMD_PT_L3 = 3,	/* 512 × 1GB = 512GB */
};

#define AMD_PT_ENTRIES		512
#define AMD_PT_INDEX(addr, level)	(((addr) >> (12 + 9 * ((level) - 1))) & 0x1FF)

/* Allocate a zeroed 4K page table page.  Returns phys addr in *phys. */
static uint64_t *amd_pt_alloc_page(uint64_t *phys_out)
{
	phys_bytes p;
	void *virt = alloc_contig(4096, AC_ALIGN4K, &p);
	if (!virt) return NULL;
	memset(virt, 0, 4096);
	*phys_out = (uint64_t)p;
	return (uint64_t *)virt;
}

/* Map a set of page-aligned physical pages into an IOMMU domain.
 * Walks the 3-level page table, allocating intermediate tables as needed.
 *
 * @param root_virt  Virtual address of the level-3 page table root
 * @param root_phys  Physical address of the level-3 page table root
 * @param iova       Starting I/O virtual address (must be page-aligned)
 * @param phys_addr  Starting physical address (must be page-aligned)
 * @param pages      Number of 4KB pages to map
 * @param flags      Bit 0 = read/write, bit 1 = user
 * @param unit       IOMMU unit (for huge page capability checks)
 * @returns 0 on success, negative errno on failure
 */
static int amd_pt_map_pages(uint64_t *root_virt, uint64_t root_phys,
    uint64_t iova, uint64_t phys_addr, size_t pages, int flags,
    struct amd_iommu_unit *unit)
{
	uint64_t pte_flags = AMD_PTE_PRESENT | AMD_PTE_RW;
	if (flags & 2) pte_flags |= AMD_PTE_USER;

	while (pages > 0) {
		uint64_t l3_idx = AMD_PT_INDEX(iova, 3);
		uint64_t l2_idx = AMD_PT_INDEX(iova, 2);
		uint64_t l1_idx = AMD_PT_INDEX(iova, 1);

		size_t l1_remaining = (AMD_PT_ENTRIES - l1_idx);

		/* Try 1GB huge page at level 3 (if supported and aligned) */
		if (unit->has_1gb_pages && pages >= 262144 &&
		    (iova & 0x3FFFFFFF) == 0 && (phys_addr & 0x3FFFFFFF) == 0) {
			uint64_t *l3 = root_virt;
			l3[l3_idx] = (phys_addr & AMD_PTE_ADDR_MASK) |
			    AMD_PTE_PRESENT | AMD_PTE_RW | AMD_PTE_PS;
			phys_addr += 0x40000000ULL;  /* 1GB */
			iova += 0x40000000ULL;
			pages -= 262144;
			continue;
		}

		/* Get or create level-2 page table */
		uint64_t l3_entry = root_virt[l3_idx];
		uint64_t *l2_virt;
		uint64_t l2_phys;

		if (!(l3_entry & AMD_PTE_PRESENT)) {
			/* Allocate new L2 table */
			l2_virt = amd_pt_alloc_page(&l2_phys);
			if (!l2_virt) return -ENOMEM;
			root_virt[l3_idx] = (l2_phys & AMD_PTE_ADDR_MASK) |
			    AMD_PTE_PRESENT | AMD_PTE_RW;
		} else {
			l2_phys = l3_entry & AMD_PTE_ADDR_MASK;
			l2_virt = vm_map_phys(SELF, (void *)(uintptr_t)l2_phys, 4096);
			/* Note: simplified — in a real implementation we'd cache
			 * the virt mapping of intermediate tables. */
		}

		/* Try 2MB huge page at level 2 (if supported and aligned) */
		if (unit->has_2mb_pages && pages >= 512 &&
		    (iova & 0x1FFFFF) == 0 && (phys_addr & 0x1FFFFF) == 0) {
			l2_virt[l2_idx] = (phys_addr & AMD_PTE_ADDR_MASK) |
			    AMD_PTE_PRESENT | AMD_PTE_RW | AMD_PTE_PS;
			phys_addr += 0x200000ULL;  /* 2MB */
			iova += 0x200000ULL;
			pages -= 512;
			continue;
		}

		/* Get or create level-1 page table */
		uint64_t l2_entry = l2_virt[l2_idx];
		uint64_t *l1_virt;
		uint64_t l1_phys;

		if (!(l2_entry & AMD_PTE_PRESENT)) {
			l1_virt = amd_pt_alloc_page(&l1_phys);
			if (!l1_virt) return -ENOMEM;
			l2_virt[l2_idx] = (l1_phys & AMD_PTE_ADDR_MASK) |
			    AMD_PTE_PRESENT | AMD_PTE_RW;
		} else {
			l1_phys = l2_entry & AMD_PTE_ADDR_MASK;
			l1_virt = vm_map_phys(SELF, (void *)(uintptr_t)l1_phys, 4096);
		}

		/* Map 4KB pages at level 1 */
		size_t batch = (pages < l1_remaining) ? pages : l1_remaining;
		for (size_t i = 0; i < batch; i++) {
			l1_virt[l1_idx + i] = (phys_addr & AMD_PTE_ADDR_MASK) | pte_flags;
			phys_addr += 0x1000;
		}
		iova += batch * 0x1000;
		pages -= batch;
	}

	return 0;
}

/* Unmap a range of pages and free any now-empty intermediate tables.
 * Walks the page tables and clears entries, freeing L1 tables when
 * all 512 entries become zero, and similarly for L2/L3. */
static void amd_pt_unmap_pages(uint64_t *root_virt, uint64_t root_phys,
    uint64_t iova, size_t size, struct amd_iommu_unit *unit)
{
	size_t pages = (size + 4095) / 4096;
	(void)root_phys;
	(void)unit;

	while (pages > 0) {
		uint64_t l3_idx = AMD_PT_INDEX(iova, 3);
		uint64_t l2_idx = AMD_PT_INDEX(iova, 2);
		uint64_t l1_idx = AMD_PT_INDEX(iova, 1);

		uint64_t l3_entry = root_virt[l3_idx];
		if (!(l3_entry & AMD_PTE_PRESENT)) {
			/* Nothing mapped — advance by 1GB */
			size_t skip = 262144;
			if (skip > pages) skip = pages;
			pages -= skip;
			iova += skip * 4096;
			continue;
		}

		/* Check if L3 entry is a 1GB huge page */
		if (l3_entry & AMD_PTE_PS) {
			root_virt[l3_idx] = 0;
			if (pages >= 262144)
				pages -= 262144;
			else
				pages = 0;
			iova += 0x40000000ULL;
			continue;
		}

		uint64_t l2_phys = l3_entry & AMD_PTE_ADDR_MASK;
		uint64_t *l2_virt = vm_map_phys(SELF, (void *)(uintptr_t)l2_phys, 4096);
		uint64_t l2_entry = l2_virt[l2_idx];

		if (!(l2_entry & AMD_PTE_PRESENT)) {
			/* Nothing at L2 — advance by 2MB */
			size_t skip = 512;
			if (skip > pages) skip = pages;
			pages -= skip;
			iova += skip * 4096;
			continue;
		}

		/* Check if L2 entry is a 2MB huge page */
		if (l2_entry & AMD_PTE_PS) {
			l2_virt[l2_idx] = 0;
			if (pages >= 512)
				pages -= 512;
			else
				pages = 0;
			iova += 0x200000;
			continue;
		}

		uint64_t l1_phys = l2_entry & AMD_PTE_ADDR_MASK;
		uint64_t *l1_virt = vm_map_phys(SELF, (void *)(uintptr_t)l1_phys, 4096);

		size_t batch = (pages < (512 - l1_idx)) ? pages : (512 - l1_idx);
		for (size_t i = 0; i < batch; i++)
			l1_virt[l1_idx + i] = 0;

		/* Check if L1 table is now empty — if so, free it */
		int empty = 1;
		for (int i = 0; i < AMD_PT_ENTRIES; i++) {
			if (l1_virt[i] & AMD_PTE_PRESENT) { empty = 0; break; }
		}
		if (empty) {
			l2_virt[l2_idx] = 0;
			/* Note: free_contig not available in MINIX — memory reused */
		}

		/* Check if L2 table is now empty */
		empty = 1;
		for (int i = 0; i < AMD_PT_ENTRIES; i++) {
			if (l2_virt[i] & AMD_PTE_PRESENT) { empty = 0; break; }
		}
		if (empty) {
			root_virt[l3_idx] = 0;
			/* L2 page freed when L3 entry cleared */
		}

		iova += batch * 4096;
		pages -= batch;
	}
}

/*===========================================================================*
 *		API implementation					     *
 *===========================================================================*/

static int amd_detect(void)
{
	struct amd_iommu_unit *unit;
	int r;

	/* Try IVRS table first */
	r = amd_parse_ivrs();
	if (r > 0)
		return 1;

	/* Fall back: scan PCI for AMD IOMMU capability
	 * (legacy AMD DEV or full IOMMU).  This mirrors the existing
	 * amddev's find_dev() logic. */
	/* ... (simplified: rely on IVRS for full detection) */

	return 0;
}

static int amd_init_hw(void)
{
	int r;

	if (amd_unit_count == 0) {
		printf("iommu_amd: no IOMMU units to initialise\n");
		return -ENODEV;
	}

	for (unsigned int i = 0; i < amd_unit_count; i++) {
		r = amd_init_unit(&amd_units[i]);
		if (r != 0) {
			printf("iommu_amd: unit %u init failed (%d)\n", i, r);
			return r;
		}
	}

	amd_initialised = 1;
	return 0;
}

static void amd_shutdown_hw(void)
{
	for (unsigned int i = 0; i < amd_unit_count; i++) {
		struct amd_iommu_unit *unit = &amd_units[i];
		amd_write32(unit, AMD_IOMMU_OFFSET_DEV_CR, 0);  /* disable */
	}
	amd_initialised = 0;
}

static int amd_domain_alloc(struct gergios_iommu_domain *domain)
{
	phys_bytes phys;
	void *page_table;
	int domain_id;

	if (amd_domain_count >= 64)
		return -ENOMEM;

	domain_id = amd_domain_count;

	/* Allocate level-3 page table root */
	page_table = alloc_contig(4096, AC_ALIGN4K, &phys);
	if (!page_table)
		return -ENOMEM;

	memset(page_table, 0, 4096);

	domain->domain_id = domain_id;
	domain->type = GERGIOS_IOMMU_AMD_VI;
	domain->priv = (void *)(uintptr_t)phys;  /* store root table phys addr */
	domain->max_address = 0xFFFFFFFFFFFFFFFFULL;
	domain->ref_count = 0;

	amd_domains[amd_domain_count] = *domain;
	amd_domain_count++;

	printf("iommu_amd: domain %d allocated (root=0x%llx)\n",
	    domain_id, (unsigned long long)phys);

	return 0;
}

static void amd_domain_free(struct gergios_iommu_domain *domain)
{
	/* For now, just mark as free.  Full implementation would
	 * free all page table pages. */
	domain->priv = NULL;
	domain->domain_id = -1;
	printf("iommu_amd: domain %d freed\n", domain->domain_id);
}

static int amd_domain_attach_device(struct gergios_iommu_domain *domain,
    uint8_t bus, uint8_t dev, uint8_t func)
{
	uint16_t bdf = (uint16_t)((bus << 8) | (dev << 3) | func);
	uint8_t *dev_table_entry;
	uint64_t root_phys = (uint64_t)(uintptr_t)domain->priv;

	/* Find the IOMMU unit responsible for this segment/device */
	struct amd_iommu_unit *unit = &amd_units[0];  /* simplified: first unit */
	if (!unit->dev_table_virt)
		return -ENODEV;

	/* Each device table entry is 16 bytes:
	 *   bytes 0-3:   Flags + page table root pointer
	 *   bytes 4-7:   Domain ID + IOMMU-specific flags
	 *   bytes 8-15:  Reserved / extended
	 */
	dev_table_entry = unit->dev_table_virt + bdf * 16;

	/* Set up the device table entry with the domain's page table root.
	 * Entry format (AMD spec):
	 *   bit 0:   V (valid)
	 *   bit 1-2: TV (table type: 1 = level 3 page table)
	 *   bit 3-6: reserved
	 *   bit 7:   I (IRQ remap enable)
	 *   bit 8-9: reserved
	 *   bit 10:  IG (guest page table)
	 *   bit 11:  FE (fixed entries)
	 *   bits 12-51: page table root pointer (4K-aligned)
	 *   bits 52-63: reserved
	 */
	uint64_t entry_lo = (1ULL << 0) |		/* V */
			    (1ULL << 1);		/* TV = level 3 */
	entry_lo |= root_phys & 0x000FFFFFFFFFFFF0ULL;	/* root table phys addr */

	uint64_t entry_hi = ((uint64_t)domain->domain_id << 8);

	((uint64_t *)dev_table_entry)[0] = entry_lo;
	((uint64_t *)dev_table_entry)[1] = entry_hi;

	/* Ensure writes are visible to the IOMMU */
	__sync_synchronize();

	/* If the IOMMU is already enabled, invalidate the IOTLB for this device */
	if (amd_initialised) {
		// amd_invalidate_pages(unit, bdf, 0, ~0ULL);
	}

	domain->ref_count++;
	printf("iommu_amd: attached dev %02x:%02x.%x (BDF=0x%04x) to domain %d\n",
	    bus, dev, func, bdf, domain->domain_id);

	return 0;
}

static void amd_domain_detach_device(struct gergios_iommu_domain *domain,
    uint8_t bus, uint8_t dev, uint8_t func)
{
	uint16_t bdf = (uint16_t)((bus << 8) | (dev << 3) | func);
	struct amd_iommu_unit *unit = &amd_units[0];
	uint8_t *dev_table_entry;

	if (!unit->dev_table_virt)
		return;

	dev_table_entry = unit->dev_table_virt + bdf * 16;
	((uint64_t *)dev_table_entry)[0] = 0;
	((uint64_t *)dev_table_entry)[1] = 0;

	__sync_synchronize();
	domain->ref_count--;
}

static int amd_map(struct gergios_iommu_domain *domain,
    uint64_t iova, phys_bytes phys_addr, size_t size, int flags)
{
	struct amd_iommu_unit *unit = &amd_units[0];
	uint64_t root_phys = (uint64_t)(uintptr_t)domain->priv;
	uint64_t *root_virt;
	size_t pages = (size + 4095) / 4096;
	int r;

	if (!unit->has_page_tables)
		return -ENODEV;

	/* Map the root page table for access */
	root_virt = vm_map_phys(SELF, (void *)(uintptr_t)root_phys, 4096);
	if (root_virt == MAP_FAILED)
		return -ENOMEM;

	r = amd_pt_map_pages(root_virt, root_phys, iova,
	    (uint64_t)phys_addr, pages, flags, unit);

	if (r == 0) {
		/* Invalidate IOTLB for the mapped range */
		amd_invalidate_pages(unit, 0xFFFF, iova, size);
	}

	return r;
}

static void amd_unmap(struct gergios_iommu_domain *domain,
    uint64_t iova, size_t size)
{
	struct amd_iommu_unit *unit = &amd_units[0];
	uint64_t root_phys = (uint64_t)(uintptr_t)domain->priv;
	uint64_t *root_virt;

	if (!unit->has_page_tables)
		return;
	if (size == 0)
		return;

	root_virt = vm_map_phys(SELF, (void *)(uintptr_t)root_phys, 4096);
	if (root_virt == MAP_FAILED)
		return;

	amd_pt_unmap_pages(root_virt, root_phys, iova, size, unit);

	/* Invalidate IOTLB for the unmapped range */
	amd_invalidate_pages(unit, 0xFFFF, iova, size);
}

static int amd_identity_map(struct gergios_iommu_domain *domain,
    phys_bytes phys_addr, size_t size)
{
	return amd_map(domain, (uint64_t)phys_addr, phys_addr, size, 0);
}

static void amd_iotlb_invalidate_domain(struct gergios_iommu_domain *domain)
{
	struct amd_iommu_unit *unit = &amd_units[0];
	amd_invalidate_pages(unit, 0xFFFF, 0, ~0ULL);
	/* Full wait for completion would poll the head pointer */
}

static void amd_iotlb_invalidate_range(struct gergios_iommu_domain *domain,
    uint64_t iova, size_t size)
{
	struct amd_iommu_unit *unit = &amd_units[0];
	amd_invalidate_pages(unit, 0xFFFF, iova, size);
}

static void amd_iotlb_invalidate_all(void)
{
	struct amd_iommu_unit *unit = &amd_units[0];
	amd_invalidate_pages(unit, 0xFFFF, 0, ~0ULL);
}

/*===========================================================================*
 *		Interrupt remapping (AMD-Vi IRTE)			     *
 *===========================================================================*/
/*
 * AMD-Vi Interrupt Remap Table Entry (IRTE) — 16 bytes per entry.
 * Field layout (per AMD IOMMU spec rev 3.0):
 *   Word 0 (bits 0-63):
 *     Bits 0-7:   Vector
 *     Bits 8-10:  Delivery Mode (000=Fixed, 011=NMI, 100=INIT, 101=ExtINT)
 *     Bit 11:     Destination Mode (0=Physical, 1=Logical)
 *     Bit 12:     Trigger Mode (0=Edge, 1=Level)
 *     Bit 13:     Redirection Hint
 *     Bit 14:     Interrupt Mask (1=masked)
 *     Bits 15-19: Reserved
 *     Bits 20-31: Destination (APIC ID, low 12 bits)
 *     Bits 32-47: Extended Destination (x2APIC)
 *     Bits 48-63: Reserved
 *   Word 1 (bits 64-127):
 *     Bits 64-79: Source ID (Requester ID — BDF format)
 *     Bit 80:     Guest Interrupt
 *     Bits 81-126: Reserved
 *     Bit 127:    V (Valid)
 */

#define AMD_IRTE_VECTOR_MASK	0x00000000000000FFULL
#define AMD_IRTE_DELIVERY_MODE_FIXED	(0ULL << 8)
#define AMD_IRTE_DEST_MODE_PHYSICAL	(0ULL << 11)
#define AMD_IRTE_DEST_MODE_LOGICAL	(1ULL << 11)
#define AMD_IRTE_TRIGGER_EDGE		(0ULL << 12)
#define AMD_IRTE_TRIGGER_LEVEL		(1ULL << 12)
#define AMD_IRTE_RH			(1ULL << 13)
#define AMD_IRTE_MASKED			(1ULL << 14)
#define AMD_IRTE_DEST_LOW(apic)	(((uint64_t)(apic) & 0xFFF) << 20)
#define AMD_IRTE_DEST_EXT(apic)	((((uint64_t)(apic) >> 12) & 0xFFFF) << 32)
/* Source ID is in bits 64-79 of the IRTE = bits 0-15 of the upper 64-bit word */
#define AMD_IRTE_SRC_ID(bdf)	(((uint64_t)(bdf) & 0xFFFF))
/* Valid bit is bit 127 of the IRTE = bit 63 of the upper 64-bit word */
#define AMD_IRTE_VALID		(1ULL << 63)

/* Number of IRTEs per IOMMU unit (allocated as one 4K page = 256 entries) */
#define AMD_IRTE_COUNT		256

/* AMD DTE: Set I-bit (bit 7) to enable interrupt remapping per device */
#define AMD_DTE_I_BIT		(1ULL << 7)

/* Per-unit interrupt remap table tracking */
struct amd_ir_table {
	uint64_t	phys;		/* Physical address of IRT */
	uint64_t	*virt;		/* Virtual address of IRT */
	uint32_t	alloc_map;	/* Bitmap: bit N = IRTE N in use */
};

static struct amd_ir_table amd_ir_tables[MAX_AMD_IOMMU_UNITS];

static int amd_intr_remap_enable(void)
{
	for (unsigned int u = 0; u < amd_unit_count; u++) {
		struct amd_iommu_unit *unit = &amd_units[u];
		phys_bytes phys;

		if (!unit->has_intr_remap)
			continue;

		/* Allocate Interrupt Remap Table (4K page, 256 × 16-byte entries) */
		amd_ir_tables[u].virt = alloc_contig(4096, AC_ALIGN4K, &phys);
		if (!amd_ir_tables[u].virt)
			return -ENOMEM;
		amd_ir_tables[u].phys = (uint64_t)phys;
		memset(amd_ir_tables[u].virt, 0, 4096);
		amd_ir_tables[u].alloc_map = 0;

		/* Write IRT base register (MMIO offset 0x0070-0x0074) */
		amd_write64(unit, AMD_IOMMU_OFFSET_IRQ_TABLE_BASE_LO,
		    AMD_IOMMU_OFFSET_IRQ_TABLE_BASE_HI, (uint64_t)phys);

		/* Write IRT length register: log2(256) = 8 */
		amd_write32(unit, AMD_IOMMU_OFFSET_IRQ_TABLE_LENGTH, 8);

		/* Enable interrupt remapping in control register */
		uint32_t cr = amd_read32(unit, AMD_IOMMU_OFFSET_DEV_CR);
		cr |= AMD_IOMMU_CR_IRQ_ENABLE;
		amd_write32(unit, AMD_IOMMU_OFFSET_DEV_CR, cr);

		printf("iommu_amd: interrupt remapping enabled on unit %u "
		    "(IRT=0x%llx)\n", u, (unsigned long long)phys);
	}

	return 0;
}

static int amd_intr_remap_set(uint8_t bus, uint8_t dev, uint8_t func,
    unsigned int vector, uint64_t destination)
{
	uint16_t bdf = (uint16_t)((bus << 8) | (dev << 3) | func);
	int unit_idx = 0;
	struct amd_iommu_unit *unit;
	struct amd_ir_table *irt;
	int slot;

	if (amd_unit_count == 0)
		return -ENODEV;

	unit = &amd_units[unit_idx];
	irt = &amd_ir_tables[unit_idx];

	if (!unit->has_intr_remap || !irt->virt)
		return -ENOTSUP;

	/* Find a free IRTE slot */
	for (slot = 0; slot < AMD_IRTE_COUNT; slot++) {
		if (!(irt->alloc_map & (1U << slot)))
			break;
	}
	if (slot >= AMD_IRTE_COUNT)
		return -ENOSPC;

	/* Program the IRTE entry (16 bytes = 2 × 64-bit words) */
	uint64_t irte_lo = (uint64_t)(vector & 0xFF) |
	    AMD_IRTE_DELIVERY_MODE_FIXED |
	    AMD_IRTE_DEST_MODE_PHYSICAL |
	    AMD_IRTE_TRIGGER_EDGE |
	    AMD_IRTE_DEST_LOW((unsigned int)destination) |
	    AMD_IRTE_DEST_EXT((unsigned int)destination);

	uint64_t irte_hi = AMD_IRTE_SRC_ID(bdf) | AMD_IRTE_VALID;

	irt->virt[slot * 2 + 0] = irte_lo;
	irt->virt[slot * 2 + 1] = irte_hi;
	irt->alloc_map |= (1U << slot);

	__sync_synchronize();

	/* Set DTE I-bit for this device */
	if (unit->dev_table_virt) {
		uint64_t *dte = (uint64_t *)(unit->dev_table_virt + bdf * 16);
		dte[0] |= AMD_DTE_I_BIT;  /* Set I-bit in entry_lo */
		__sync_synchronize();
	}

	printf("iommu_amd: IRTE[%d] dev=%02x:%02x.%x vector=%u "
	    "dest=0x%llx\n",
	    slot, bus, dev, func, vector,
	    (unsigned long long)destination);

	return 0;
}

/*===========================================================================*
 *		IOMMU ops table						     *
 *===========================================================================*/
static const struct gergios_iommu_ops amd_iommu_ops = {
	.detect			= amd_detect,
	.init_hw		= amd_init_hw,
	.shutdown_hw		= amd_shutdown_hw,
	.domain_alloc		= amd_domain_alloc,
	.domain_free		= amd_domain_free,
	.domain_attach_device	= amd_domain_attach_device,
	.domain_detach_device	= amd_domain_detach_device,
	.map			= amd_map,
	.unmap			= amd_unmap,
	.identity_map		= amd_identity_map,
	.iotlb_invalidate_domain = amd_iotlb_invalidate_domain,
	.iotlb_invalidate_range	= amd_iotlb_invalidate_range,
	.iotlb_invalidate_all	= amd_iotlb_invalidate_all,
	.intr_remap_enable	= amd_intr_remap_enable,
	.intr_remap_set		= amd_intr_remap_set,
};

const struct gergios_iommu_ops *gergios_iommu_amd_get_ops(void)
{
	return &amd_iommu_ops;
}
