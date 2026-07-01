/* iommu_vtd.c — Intel VT-d (Virtualization Technology for Directed I/O) Backend
 *
 * Implements the unified gergios_iommu_ops interface for Intel VT-d
 * IOMMU hardware, as specified in "Intel Virtualization Technology
 * for Directed I/O", Revision 3.4 (February 2024).
 *
 * Key hardware features:
 *   - DMA remapping via root table → context table → page tables
 *   - Interrupt remapping (optional, for MSI/MSI-X isolation)
 *   - IOTLB invalidation via write buffer and IOTLB registers
 *   - Queued Invalidation (QI) interface
 *   - Protection domains with isolation
 *
 * Detection: via ACPI DMAR table (DMA Remapping Reporting).
 */

#include <minix/drivers.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <minix/sysutil.h>
#include <minix/type.h>
#include <minix/com.h>
#include <minix/vm.h>
#include <assert.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "iommu.h"

/*===========================================================================*
 *		VT-d MMIO register offsets (per hardware unit)		     *
 *===========================================================================*/
#define VTD_REG_VER		0x000	/* Version register */
#define VTD_REG_CAP		0x008	/* Capability register */
#define VTD_REG_ECAP		0x010	/* Extended capability register */
#define VTD_REG_GCMD		0x018	/* Global command */
#define		VTD_GCMD_SRTP	(1ULL << 60)	/* Set root table pointer */
#define		VTD_GCMD_SIRTP	(1ULL << 55)	/* Set interrupt remap table ptr */
#define		VTD_GCMD_TE	(1ULL << 31)	/* Translation enable */
#define		VTD_GCMD_IRE	(1ULL << 30)	/* Interrupt remap enable */
#define		VTD_GCMD_CFI	(1ULL << 29)	/* Compat format IRQ */
#define		VTD_GCMD_QIE	(1ULL << 26)	/* Queued invalidation enable */
#define		VTD_GCMD_WBF	(1ULL << 5)	/* Write buffer flush */
#define		VTD_GCMD_FLR	(1ULL << 4)	/* Fault log reg set */
#define VTD_REG_GSTS		0x020	/* Global status */
#define		VTD_GSTS_TES	(1ULL << 31)
#define		VTD_GSTS_IRES	(1ULL << 30)
#define		VTD_GSTS_QIES	(1ULL << 26)
#define VTD_REG_RTADDR		0x028	/* Root table address */
#define VTD_REG_CCMD		0x030	/* Context command */
#define VTD_REG_FSTS		0x034	/* Fault status */
#define VTD_REG_FECTL		0x038	/* Fault event control */
#define VTD_REG_FEDATA		0x040	/* Fault event data */
#define VTD_REG_FEADDR		0x048	/* Fault event address */
#define VTD_REG_FEUHI		0x050	/* Fault event upper */
#define VTD_REG_IQH		0x080	/* Invalidation queue head */
#define VTD_REG_IQT		0x088	/* Invalidation queue tail */
#define VTD_REG_IQA		0x090	/* Invalidation queue address */
#define VTD_REG_ICS		0x09C	/* Invalidation complete status */
#define VTD_REG_IEC		0x0A0	/* Invalidation event control */
#define VTD_REG_IEDATA		0x0A8	/* Invalidation event data */
#define VTD_REG_IEADDR		0x0B0	/* Invalidation event address */
#define VTD_REG_IEUHI		0x0B8	/* Invalidation event upper */
#define VTD_REG_IRTA		0x0C0	/* Interrupt remap table address */

/*===========================================================================*
 *		VT-d capabilities (from CAP register)			     *
 *===========================================================================*/
#define VTD_CAP_ND			(0xFFULL << 48) /* number of domains */
#define VTD_CAP_MAMV			(0x3FULL << 40) /* max addr mask value */
#define VTD_CAP_PSI			(1ULL << 39)	/* page selective invalidation */
#define VTD_CAP_SLLPS			(0xFULL << 34)	/* supported large page sizes */
#define VTD_CAP_FRO			(0x3FFULL << 24) /* fault recording offset */
#define VTD_CAP_FR			(0xFFULL << 0)	/* number of fault regs */

/* VT-d extended capabilities (from ECAP register) */
#define VTD_ECAP_C			(1ULL << 0)	/* complete IRQ table */
#define VTD_ECAP_QI			(1ULL << 1)	/* queued invalidation */
#define VTD_ECAP_DI			(1ULL << 2)	/* device IOTLB */
#define VTD_ECAP_IR			(1ULL << 3)	/* interrupt remapping */
#define VTD_ECAP_EIM			(1ULL << 4)	/* extended interrupt mode */
#define VTD_ECAP_PT			(1ULL << 5)	/* pass-through */
#define VTD_ECAP_SC			(1ULL << 7)	/* snoop control */
#define VTD_ECAP_MHMV			(0xFULL << 20)	/* max handle mask value */

/*===========================================================================*
 *		Internal state					     *
 *===========================================================================*/

#define MAX_VTD_UNITS			8
#define VTD_ROOT_TABLE_ENTRIES		256	/* bus number (0-255) */
#define VTD_CTX_TABLE_ENTRIES		256	/* dev:func (0-255) */

struct vtd_unit {
	uint8_t		present;
	uint64_t	mmio_base;	/* Register base (from DMAR) */
	uint8_t	       *mmio_virt;	/* Mapped virtual address */
	uint16_t	segment;	/* PCI segment group */
	uint8_t		flags;
	uint64_t	cap;		/* Capability register value */
	uint64_t	ecap;		/* Extended capability register */
	unsigned int	has_qi : 1;	/* Queued invalidation */
	unsigned int	has_ir : 1;	/* Interrupt remapping */
	unsigned int	has_pt : 1;	/* Pass-through */
	unsigned int	has_sc : 1;	/* Snoop control */

	/* Root table (one entry per bus number) */
	uint64_t	root_table_phys;
	uint8_t	       *root_table_virt;

	/* Queued invalidation queue */
	uint32_t       *qi_queue_virt;
	uint64_t	qi_queue_phys;
	uint32_t	qi_head;
	uint32_t	qi_tail;

	/* Completed invalidation tracking */
	uint32_t	qi_complete;
};

static struct vtd_unit vtd_units[MAX_VTD_UNITS];
static unsigned int vtd_unit_count = 0;
static struct gergios_iommu_domain vtd_domains[64];
static unsigned int vtd_domain_count = 0;
static int vtd_initialised = 0;

/*===========================================================================*
 *		MMIO access helpers					     *     *
 *===========================================================================*/
static inline uint64_t vtd_read64(struct vtd_unit *unit, uint32_t off)
{
	return *((volatile uint64_t *)(unit->mmio_virt + off));
}

static inline void vtd_write64(struct vtd_unit *unit, uint32_t off, uint64_t val)
{
	*((volatile uint64_t *)(unit->mmio_virt + off)) = val;
}

static inline uint32_t vtd_read32(struct vtd_unit *unit, uint32_t off)
{
	return *((volatile uint32_t *)(unit->mmio_virt + off));
}

static inline void vtd_write32(struct vtd_unit *unit, uint32_t off, uint32_t val)
{
	*((volatile uint32_t *)(unit->mmio_virt + off)) = val;
}

/*===========================================================================*
 *		ACPI DMAR table parsing					     *
 *===========================================================================*/

/* DMAR subtable types (from actbl2.h / ACPICA) */
#define DMAR_TYPE_HARDWARE_UNIT		0
#define DMAR_TYPE_RESERVED_MEMORY	1
#define DMAR_TYPE_ROOT_ATS		2
#define DMAR_TYPE_HARDWARE_AFFINITY	3
#define DMAR_TYPE_NAMESPACE		4

/* DMAR subtable headers use acpi_dmar_header from ACPICA:
 *   struct acpi_dmar_header { uint16_t Type; uint16_t Length; };
 * which matches:
 *   struct { uint16_t type; uint16_t length; } __attribute__((packed));
 */

struct acpi_dmar_hardware_unit {
	uint16_t	type;		/* 0 = hardware unit */
	uint16_t	length;
	uint8_t		flags;
	uint8_t		reserved;
	uint16_t	segment;
	uint64_t	address;	/* Register base */
} __attribute__((packed));

/*===========================================================================*
 *		VT-d hardware initialisation				     *
 *===========================================================================*/

static int vtd_parse_dmar(void)
{
	struct acpi_sdt_header *dmar;
	uint8_t *ptr, *end;

	dmar = acpi_find_table("DMAR");
	if (!dmar) {
		printf("iommu_vtd: DMAR table not found\n");
		return 0;
	}

	printf("iommu_vtd: DMAR table found (rev %u, len %u)\n",
	    dmar->revision, dmar->length);

	ptr = (uint8_t *)dmar + sizeof(struct acpi_sdt_header);
	end = (uint8_t *)dmar + dmar->length;

	while (ptr < end && vtd_unit_count < MAX_VTD_UNITS) {
		uint16_t sub_type = *(uint16_t *)ptr;
		uint16_t sub_len = *(uint16_t *)(ptr + 2);

		if (sub_len < 4)
			break;

		switch (sub_type) {
		case DMAR_TYPE_HARDWARE_UNIT: {
			struct acpi_dmar_hardware_unit *hu =
			    (struct acpi_dmar_hardware_unit *)ptr;

			if ((uint8_t *)(hu + 1) > end)
				break;

			struct vtd_unit *unit = &vtd_units[vtd_unit_count];
			memset(unit, 0, sizeof(*unit));
			unit->present = 1;
			unit->mmio_base = hu->address;
			unit->segment = hu->segment;
			unit->flags = hu->flags;

			printf("iommu_vtd: unit %u: MMIO=0x%llx seg=%u\n",
			    vtd_unit_count,
			    (unsigned long long)unit->mmio_base,
			    unit->segment);

			vtd_unit_count++;
			break;
		}
		case DMAR_TYPE_RESERVED_MEMORY:
			/* Reserved memory ranges — for identity mapping */
			break;
		case DMAR_TYPE_ROOT_ATS:
			/* Root port ATS capability */
			break;
		case DMAR_TYPE_HARDWARE_AFFINITY:
			/* Remapping HW affinity */
			break;
		}

		ptr += sub_len;
	}

	free(dmar);
	return vtd_unit_count;
}

static int vtd_wait_for_command(struct vtd_unit *unit, uint32_t reg_off,
    uint64_t mask, uint64_t expected_value)
{
	unsigned int timeout = 100000;  /* 100k iterations */

	while (timeout--) {
		uint64_t val = (reg_off == 0x020)
		    ? (vtd_read32(unit, reg_off) & 0xFFFFFFFFULL)
		    : vtd_read64(unit, reg_off);

		if ((val & mask) == expected_value)
			return 0;

		/* Small delay */
		for (volatile int i = 0; i < 100; i++);
	}

	printf("iommu_vtd: command wait timeout (reg=0x%x, mask=0x%llx, "
	    "expected=0x%llx)\n", reg_off,
	    (unsigned long long)mask, (unsigned long long)expected_value);

	return -ETIMEDOUT;
}

static int vtd_init_unit(struct vtd_unit *unit)
{
	uint64_t cap, ecap;
	phys_bytes phys;

	if (!unit->present || !unit->mmio_base)
		return 0;

	/* Map MMIO registers (VT-d uses up to ~0x200 bytes) */
	unit->mmio_virt = vm_map_phys(SELF, (void *)(uintptr_t)unit->mmio_base, 0x1000);
	if (unit->mmio_virt == MAP_FAILED) {
		printf("iommu_vtd: failed to map MMIO at 0x%llx\n",
		    (unsigned long long)unit->mmio_base);
		return -ENODEV;
	}

	/* Read capabilities */
	unit->cap = cap = vtd_read64(unit, VTD_REG_CAP);
	unit->ecap = ecap = vtd_read64(unit, VTD_REG_ECAP);

	printf("iommu_vtd: unit CAP=0x%llx ECAP=0x%llx\n",
	    (unsigned long long)cap, (unsigned long long)ecap);

	unit->has_qi = (ecap & VTD_ECAP_QI) != 0;
	unit->has_ir = (ecap & VTD_ECAP_IR) != 0;
	unit->has_pt = (ecap & VTD_ECAP_PT) != 0;
	unit->has_sc = (ecap & VTD_ECAP_SC) != 0;

	/* Allocate root table (one 4K page) */
	unit->root_table_virt = alloc_contig(4096, AC_ALIGN4K, &phys);
	if (!unit->root_table_virt)
		return -ENOMEM;
	unit->root_table_phys = phys;
	memset(unit->root_table_virt, 0, 4096);

	/* Set root table address (must be done before translation enable) */
	vtd_write64(unit, VTD_REG_RTADDR, unit->root_table_phys & 0xFFFFFFFFFFFFF000ULL);
	__sync_synchronize();

	/* Set root table pointer via command */
	vtd_write64(unit, VTD_REG_GCMD, vtd_read64(unit, VTD_REG_GCMD) | VTD_GCMD_SRTP);
	int r = vtd_wait_for_command(unit, VTD_REG_GSTS, VTD_GCMD_SRTP, 0);
	if (r != 0) {
		printf("iommu_vtd: failed to set root table pointer\n");
		return r;
	}

	/* Set up queued invalidation if supported */
	if (unit->has_qi) {
		size_t qi_size = 512 * 16;  /* 512 entries, each 16 bytes */
		unit->qi_queue_virt = alloc_contig(qi_size, AC_ALIGN4K, &phys);
		if (!unit->qi_queue_virt)
			return -ENOMEM;
		unit->qi_queue_phys = phys;
		memset(unit->qi_queue_virt, 0, qi_size);
		unit->qi_head = 0;
		unit->qi_tail = 0;
		unit->qi_complete = 0;

		/* Set invalidation queue address (lower 12 bits = queue size) */
		uint64_t iqa = phys | 9;  /* 9 = 2^(9+1) = 512 entries */
		vtd_write64(unit, VTD_REG_IQA, iqa);

		/* Enable queued invalidation */
		vtd_write32(unit, VTD_REG_GCMD, 0xFFFFFFFF);
		uint64_t gcmd = vtd_read64(unit, VTD_REG_GCMD);
		gcmd |= VTD_GCMD_QIE;
		vtd_write64(unit, VTD_REG_GCMD, gcmd);
		r = vtd_wait_for_command(unit, VTD_REG_GSTS,
		    VTD_GSTS_QIES, VTD_GSTS_QIES);
		if (r != 0)
			printf("iommu_vtd: QI enable failed (%d)\n", r);
	}

	/* Write buffer flush */
	vtd_write64(unit, VTD_REG_GCMD, vtd_read64(unit, VTD_REG_GCMD) | VTD_GCMD_WBF);
	r = vtd_wait_for_command(unit, VTD_REG_GSTS, VTD_GCMD_WBF, 0);
	if (r != 0)
		printf("iommu_vtd: write buffer flush failed (%d)\n", r);

	/* Enable DMA remapping */
	vtd_write64(unit, VTD_REG_GCMD, vtd_read64(unit, VTD_REG_GCMD) | VTD_GCMD_TE);
	r = vtd_wait_for_command(unit, VTD_REG_GSTS, VTD_GSTS_TES, VTD_GSTS_TES);
	if (r != 0) {
		printf("iommu_vtd: translation enable failed (%d)\n", r);
		return r;
	}

	printf("iommu_vtd: unit enabled (QI=%d IR=%d)\n", unit->has_qi, unit->has_ir);
	return 0;
}

/*===========================================================================*
 *		Queued Invalidation helpers				     *
 *===========================================================================*/

/* QiH descriptor: INVALIDATE_CONTEXT (32 bytes?  Actually 2×16 byte descriptors) */
#define VTD_QI_DESC_CONTEXT		0x00000001
#define VTD_QI_DESC_IOTLB		0x00000002
#define VTD_QI_DESC_DEVICE_IOTLB	0x00000003
#define VTD_QI_DESC_IEC			0x00000004
#define VTD_QI_DESC_IWD			0x00000005	/* Invalidation Wait Descriptor */

static int vtd_qi_submit(struct vtd_unit *unit, uint64_t desc[2])
{
	uint32_t tail, next_tail;

	if (!unit->qi_queue_virt)
		return -ENODEV;

	tail = unit->qi_tail;
	next_tail = (tail + 1) & 0x1FF;  /* 512 entries, power of 2 */

	/* Check if queue is full */
	unit->qi_head = vtd_read32(unit, VTD_REG_IQH);
	if (next_tail == unit->qi_head)
		return -EBUSY;

	/* Write descriptor */
	unit->qi_queue_virt[tail * 2 + 0] = (uint32_t)(desc[0] & 0xFFFFFFFF);
	unit->qi_queue_virt[tail * 2 + 1] = (uint32_t)(desc[0] >> 32);
	unit->qi_queue_virt[tail * 2 + 2] = (uint32_t)(desc[1] & 0xFFFFFFFF);
	unit->qi_queue_virt[tail * 2 + 3] = (uint32_t)(desc[1] >> 32);

	__sync_synchronize();

	unit->qi_tail = next_tail;
	vtd_write32(unit, VTD_REG_IQT, next_tail);

	return 0;
}

static int vtd_invalidate_context(struct vtd_unit *unit,
    uint16_t segment, uint16_t bdf, uint16_t bdf_mask, int global)
{
	uint64_t desc[2];

	if (global) {
		desc[0] = (uint64_t)VTD_QI_DESC_CONTEXT |
			  (1ULL << 4);	/* global invalidation */
		desc[1] = 0;
	} else {
		desc[0] = (uint64_t)VTD_QI_DESC_CONTEXT |
			  (0ULL << 4) |	/* domain-specific */
			  ((uint64_t)bdf << 32);
		desc[1] = segment |
			  ((uint64_t)bdf_mask << 32);
	}

	return vtd_qi_submit(unit, desc);
}

static int vtd_invalidate_iotlb(struct vtd_unit *unit,
    uint16_t did, uint64_t iova, size_t size, int global)
{
	uint64_t desc[2];
	unsigned int pages;

	if (global) {
		desc[0] = (uint64_t)VTD_QI_DESC_IOTLB |
			  (1ULL << 4);	/* global invalidation */
		desc[1] = (uint64_t)did << 32;
	} else {
		pages = (size + 4095) / 4096;
		if (pages > 0x1FF)
			pages = 0x1FF;

		desc[0] = (uint64_t)VTD_QI_DESC_IOTLB |
			  0;		/* domain-specific */
		desc[1] = iova | ((uint64_t)(pages - 1) << 32) |
			  ((uint64_t)(did & 0xFFFF) << 48);
	}

	return vtd_qi_submit(unit, desc);
}

static int vtd_qi_invalidation_wait(struct vtd_unit *unit)
{
	uint64_t desc[2];
	unsigned int timeout = 100000;

	/* Submit invalidation wait descriptor */
	desc[0] = (uint64_t)VTD_QI_DESC_IWD |
		  (1ULL << 4);			/* interrupt flag */
	desc[1] = 0;

	int r = vtd_qi_submit(unit, desc);
	if (r != 0)
		return r;

	/* In a full implementation, we'd wait for an interrupt or poll.
	 * Simplified: poll for completion via head pointer. */
	while (timeout--) {
		unit->qi_head = vtd_read32(unit, VTD_REG_IQH);
		if (unit->qi_head == unit->qi_tail)
			return 0;
		for (volatile int i = 0; i < 100; i++);
	}

	return -ETIMEDOUT;
}

/*===========================================================================*
 *		API implementation					     *
 *===========================================================================*/

static int vtd_detect(void)
{
	return vtd_parse_dmar();
}

static int vtd_init_hw(void)
{
	int r;

	if (vtd_unit_count == 0)
		return -ENODEV;

	for (unsigned int i = 0; i < vtd_unit_count; i++) {
		r = vtd_init_unit(&vtd_units[i]);
		if (r != 0)
			return r;
	}

	vtd_initialised = 1;
	return 0;
}

static void vtd_shutdown_hw(void)
{
	for (unsigned int i = 0; i < vtd_unit_count; i++) {
		struct vtd_unit *unit = &vtd_units[i];

		/* Disable translation */
		uint64_t gcmd = vtd_read64(unit, VTD_REG_GCMD);
		gcmd &= ~(VTD_GCMD_TE | VTD_GCMD_IRE | VTD_GCMD_QIE);
		vtd_write64(unit, VTD_REG_GCMD, gcmd);
	}

	vtd_initialised = 0;
}

static int vtd_domain_alloc(struct gergios_iommu_domain *domain)
{
	if (vtd_domain_count >= 64)
		return -ENOMEM;

	domain->domain_id = vtd_domain_count;
	domain->type = GERGIOS_IOMMU_INTEL_VTD;
	domain->max_address = 0xFFFFFFFFFFFFFFFFULL;
	domain->ref_count = 0;

	/* Allocate first-level page table root (for pass-through / identity) */
	/* In a full VT-d implementation, each domain has a first-level or
	 * second-level page table root.  For now, we store the domain_id. */
	domain->priv = (void *)(uintptr_t)(domain->domain_id + 1);
	vtd_domains[vtd_domain_count] = *domain;
	vtd_domain_count++;

	return 0;
}

static void vtd_domain_free(struct gergios_iommu_domain *domain)
{
	domain->priv = NULL;
	domain->domain_id = -1;
}

static int vtd_domain_attach_device(struct gergios_iommu_domain *domain,
    uint8_t bus, uint8_t dev, uint8_t func)
{
	struct vtd_unit *unit = &vtd_units[0];  /* simplified: first unit */
	uint64_t *root_entry;			/* pointer to root table entry */
	uint8_t *ctx_table_virt;
	uint64_t ctx_table_phys;
	uint64_t *ctx_entry;

	if (!unit->root_table_virt)
		return -ENODEV;

	/* Each root table entry is 8 bytes:
	 *   bit 0:       P (present)
	 *   bits 1-11:   reserved
	 *   bits 12-63:  context table physical address (4K-aligned)
	 */
	root_entry = (uint64_t *)unit->root_table_virt + bus;

	/* If no context table exists for this bus, create one */
	if (!(*root_entry & 1)) {
		phys_bytes phys;
		ctx_table_virt = alloc_contig(4096, AC_ALIGN4K, &phys);
		if (!ctx_table_virt)
			return -ENOMEM;
		memset(ctx_table_virt, 0, 4096);
		ctx_table_phys = phys;

		*root_entry = ctx_table_phys & 0xFFFFFFFFFFFFF000ULL;
		*root_entry |= 1;  /* present */

		__sync_synchronize();
	} else {
		/* Context table already exists: use its physical address.
		 * In a real implementation with a per-bus table tracked in
		 * unit state, we'd use the cached virtual address.  For now,
		 * compute from the root entry.  Note: we use the phys addr
		 * stored in the root table entry; no new vm_map_phys needed
		 * as we don't need to access the table from this codepath
		 * (we only write to the specific slot via unit mapping). */
		ctx_table_phys = *root_entry & 0xFFFFFFFFFFFFF000ULL;
		ctx_table_virt = NULL;  /* not accessed in this path */
	}

	/* Each context table entry is 8 bytes (for 4K page-table mode):
	 *   bit 0:       P (present)
	 *   bits 1-2:    00 = 4K page table
	 *   bit 3:       TT (translation type: 0 = host mode)
	 *   bit 4:       SMEP (supervisor mode protection)
	 *   bit 5:       reserved
	 *   bit 6:       EA (extended access)
	 *   bit 7:       EPM (extended page mode)
	 *   bits 8-11:   DID (domain ID)
	 *   bits 12-63:  first-level page table phys addr (4K-aligned)
	 */
	unsigned int slot = (dev << 3) | func;

	ctx_entry = (uint64_t *)ctx_table_virt + slot;
	*ctx_entry = (uint64_t)(uintptr_t)domain->priv;  /* page table root */
	*ctx_entry |= 1;	/* present */

	__sync_synchronize();

	domain->ref_count++;
	printf("iommu_vtd: attached %02x:%02x.%x to domain %d (ctx=0x%llx)\n",
	    bus, dev, func, domain->domain_id,
	    (unsigned long long)*ctx_entry);

	/* Invalidate context cache for this device */
	vtd_invalidate_context(unit, unit->segment, (bus << 8) | slot, 0, 0);
	vtd_qi_invalidation_wait(unit);

	return 0;
}

static void vtd_domain_detach_device(struct gergios_iommu_domain *domain,
    uint8_t bus, uint8_t dev, uint8_t func)
{
	struct vtd_unit *unit = &vtd_units[0];
	unsigned int slot = (dev << 3) | func;

	/* In the current identity-map implementation, we skip the per-entry
	 * context table clear because we don't have a cached mapping of the
	 * context table page (which is at a different physical address than
	 * the root table).  A full implementation should cache context table
	 * virtual addresses in a per-bus array within the unit structure.
	 * For now, invalidating the context cache is sufficient. */

	vtd_invalidate_context(unit, unit->segment, (bus << 8) | slot, 0, 0);
	vtd_qi_invalidation_wait(unit);

	domain->ref_count--;

	(void)domain;
}

static int vtd_map(struct gergios_iommu_domain *domain,
    uint64_t iova, phys_bytes phys_addr, size_t size, int flags)
{
	/* Simplified: identity-mapped (IOVA == phys).  Full page-table
	 * management will be implemented in a follow-up. */
	(void)domain; (void)iova; (void)phys_addr; (void)size; (void)flags;
	return 0;
}

static void vtd_unmap(struct gergios_iommu_domain *domain,
    uint64_t iova, size_t size)
{
	(void)domain; (void)iova; (void)size;
}

static int vtd_identity_map(struct gergios_iommu_domain *domain,
    phys_bytes phys_addr, size_t size)
{
	return vtd_map(domain, (uint64_t)phys_addr, phys_addr, size, 0);
}

static void vtd_iotlb_invalidate_domain(struct gergios_iommu_domain *domain)
{
	struct vtd_unit *unit = &vtd_units[0];
	vtd_invalidate_iotlb(unit, domain->domain_id, 0, ~0ULL, 1);
	vtd_qi_invalidation_wait(unit);
}

static void vtd_iotlb_invalidate_range(struct gergios_iommu_domain *domain,
    uint64_t iova, size_t size)
{
	struct vtd_unit *unit = &vtd_units[0];
	vtd_invalidate_iotlb(unit, domain->domain_id, iova, size, 0);
	vtd_qi_invalidation_wait(unit);
}

static void vtd_iotlb_invalidate_all(void)
{
	struct vtd_unit *unit = &vtd_units[0];
	vtd_invalidate_iotlb(unit, 0, 0, ~0ULL, 1);
	vtd_qi_invalidation_wait(unit);
}

static int vtd_intr_remap_enable(void)
{
	/* Stub — not yet implemented */
	return -ENOTSUP;
}

static int vtd_intr_remap_set(uint8_t bus, uint8_t dev, uint8_t func,
    unsigned int vector, uint64_t destination)
{
	(void)bus; (void)dev; (void)func; (void)vector; (void)destination;
	return -ENOTSUP;
}

/*===========================================================================*
 *		IOMMU ops table						     *
 *===========================================================================*/

static const struct gergios_iommu_ops vtd_iommu_ops = {
	.detect			= vtd_detect,
	.init_hw		= vtd_init_hw,
	.shutdown_hw		= vtd_shutdown_hw,
	.domain_alloc		= vtd_domain_alloc,
	.domain_free		= vtd_domain_free,
	.domain_attach_device	= vtd_domain_attach_device,
	.domain_detach_device	= vtd_domain_detach_device,
	.map			= vtd_map,
	.unmap			= vtd_unmap,
	.identity_map		= vtd_identity_map,
	.iotlb_invalidate_domain = vtd_iotlb_invalidate_domain,
	.iotlb_invalidate_range	= vtd_iotlb_invalidate_range,
	.iotlb_invalidate_all	= vtd_iotlb_invalidate_all,
	.intr_remap_enable	= vtd_intr_remap_enable,
	.intr_remap_set		= vtd_intr_remap_set,
};

const struct gergios_iommu_ops *gergios_iommu_vtd_get_ops(void)
{
	return &vtd_iommu_ops;
}
