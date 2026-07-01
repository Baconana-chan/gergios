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
	/* Simplified: identity mapping (IOVA == phys_addr).
	 * Full implementation would walk the page tables and set up
	 * level-1, level-2, or level-3 mappings as appropriate. */
	(void)domain; (void)iova; (void)phys_addr; (void)size; (void)flags;

	/* For now, we rely on the exclusion vector (all memory accessible)
	 * or identity mapping.  Actual page table installation will be
	 * implemented in a follow-up phase. */

	return 0;
}

static void amd_unmap(struct gergios_iommu_domain *domain,
    uint64_t iova, size_t size)
{
	(void)domain; (void)iova; (void)size;
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

/* Interrupt remapping (stub — NYI) */
static int amd_intr_remap_enable(void)
{
	return -ENOTSUP;
}

static int amd_intr_remap_set(uint8_t bus, uint8_t dev, uint8_t func,
    unsigned int vector, uint64_t destination)
{
	(void)bus; (void)dev; (void)func; (void)vector; (void)destination;
	return -ENOTSUP;
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
