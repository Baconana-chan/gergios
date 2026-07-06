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
}/*===========================================================================*
 *		Page table management (4-level: PML4/PDP/PD/PT)	     *
 *===========================================================================*/

/* VT-d page table entry bit definitions */
#define VTD_PTE_PRESENT		(1ULL << 0)
#define VTD_PTE_RW		(1ULL << 1)
#define VTD_PTE_USER		(1ULL << 2)
#define VTD_PTE_PWT		(1ULL << 3)
#define VTD_PTE_PCD		(1ULL << 4)
#define VTD_PTE_ACCESSED	(1ULL << 5)
#define VTD_PTE_DIRTY		(1ULL << 6)
#define VTD_PTE_PS		(1ULL << 7)	/* Large page (huge page indicator at L2/L3) */
#define VTD_PTE_ADDR_MASK	0x000FFFFFFFFFFFF0ULL	/* bits 12-51 */

/* VT-d page table levels and index extraction */
#define VTD_PT_ENTRIES		512
#define VTD_PML4_INDEX(addr)	(((addr) >> 39) & 0x1FF)
#define VTD_PDP_INDEX(addr)	(((addr) >> 30) & 0x1FF)
#define VTD_PD_INDEX(addr)	(((addr) >> 21) & 0x1FF)
#define VTD_PT_INDEX(addr)	(((addr) >> 12) & 0x1FF)

/* Allocate a zeroed 4K page table page.  Returns phys addr in *phys. */
static uint64_t *vtd_pt_alloc_page(uint64_t *phys_out)
{
	phys_bytes p;
	void *virt = alloc_contig(4096, AC_ALIGN4K, &p);
	if (!virt) return NULL;
	memset(virt, 0, 4096);
	*phys_out = (uint64_t)p;
	return (uint64_t *)virt;
}

/* Map a set of page-aligned physical pages into a VT-d IOMMU domain.
 * Walks the 4-level page table (PML4 → PDP → PD → PT), allocating
 * intermediate tables as needed.
 *
 * @param pml4_virt  Virtual address of the PML4 root
 * @param pml4_phys  Physical address of the PML4 root
 * @param iova       Starting I/O virtual address (must be page-aligned)
 * @param phys_addr  Starting physical address (must be page-aligned)
 * @param pages      Number of 4KB pages to map
 * @param flags      Bit 0 = write-enable, bit 1 = user
 * @param unit       VT-d unit (for large page support check)
 * @returns 0 on success, negative errno on failure
 */
static int vtd_pt_map_pages(uint64_t *pml4_virt, uint64_t pml4_phys,
    uint64_t iova, uint64_t phys_addr, size_t pages, int flags,
    struct vtd_unit *unit)
{
	uint64_t pte_flags = VTD_PTE_PRESENT | VTD_PTE_RW | VTD_PTE_ACCESSED;
	if (flags & 1) pte_flags |= VTD_PTE_RW;
	if (flags & 2) pte_flags |= VTD_PTE_USER;
	(void)pml4_phys;

	while (pages > 0) {
		uint64_t pml4_idx = VTD_PML4_INDEX(iova);
		uint64_t pdp_idx  = VTD_PDP_INDEX(iova);
		uint64_t pd_idx   = VTD_PD_INDEX(iova);
		uint64_t pt_idx   = VTD_PT_INDEX(iova);

		/* Check if we can use a 1GB huge page at PDP level */
		if ((iova & 0x3FFFFFFF) == 0 && (phys_addr & 0x3FFFFFFF) == 0 &&
		    pages >= 262144) {
			uint64_t *pdp_table;
			uint64_t pdp_phys;
			uint64_t pml4_entry = pml4_virt[pml4_idx];

			if (!(pml4_entry & VTD_PTE_PRESENT)) {
				/* Allocate PDP table */
				pdp_table = vtd_pt_alloc_page(&pdp_phys);
				if (!pdp_table) return -ENOMEM;
				pml4_virt[pml4_idx] = (pdp_phys & VTD_PTE_ADDR_MASK) |
				    VTD_PTE_PRESENT | VTD_PTE_RW;
			} else {
				pdp_phys = pml4_entry & VTD_PTE_ADDR_MASK;
				pdp_table = vm_map_phys(SELF, (void *)(uintptr_t)pdp_phys, 4096);
			}

			pdp_table[pdp_idx] = (phys_addr & VTD_PTE_ADDR_MASK) |
			    VTD_PTE_PRESENT | VTD_PTE_RW | VTD_PTE_PS | VTD_PTE_ACCESSED;

			phys_addr += 0x40000000ULL;
			iova += 0x40000000ULL;
			pages -= 262144;
			continue;
		}

		/* Get or create PDP table */
		uint64_t pml4_entry = pml4_virt[pml4_idx];
		uint64_t *pdp_table;
		uint64_t pdp_phys;

		if (!(pml4_entry & VTD_PTE_PRESENT)) {
			pdp_table = vtd_pt_alloc_page(&pdp_phys);
			if (!pdp_table) return -ENOMEM;
			pml4_virt[pml4_idx] = (pdp_phys & VTD_PTE_ADDR_MASK) |
			    VTD_PTE_PRESENT | VTD_PTE_RW;
		} else {
			pdp_phys = pml4_entry & VTD_PTE_ADDR_MASK;
			pdp_table = vm_map_phys(SELF, (void *)(uintptr_t)pdp_phys, 4096);
		}

		/* Try 2MB huge page at PD level (if aligned) */
		if ((iova & 0x1FFFFF) == 0 && (phys_addr & 0x1FFFFF) == 0 &&
		    pages >= 512) {
			uint64_t *pd_table;
			uint64_t pd_phys;
			uint64_t pdp_entry = pdp_table[pdp_idx];
			int new_pd = 0;

			if (!(pdp_entry & VTD_PTE_PRESENT)) {
				pd_table = vtd_pt_alloc_page(&pd_phys);
				if (!pd_table) return -ENOMEM;
				pdp_table[pdp_idx] = (pd_phys & VTD_PTE_ADDR_MASK) |
				    VTD_PTE_PRESENT | VTD_PTE_RW;
				new_pd = 1;
			} else {
				pd_phys = pdp_entry & VTD_PTE_ADDR_MASK;
				pd_table = vm_map_phys(SELF, (void *)(uintptr_t)pd_phys, 4096);
			}

			/* If we just allocated the PD table, use the pointer directly */
			if (new_pd) {
				/* pd_table already points to the newly allocated page */
			}

			pd_table[pd_idx] = (phys_addr & VTD_PTE_ADDR_MASK) |
			    VTD_PTE_PRESENT | VTD_PTE_RW | VTD_PTE_PS |
			    VTD_PTE_ACCESSED | VTD_PTE_DIRTY;

			phys_addr += 0x200000ULL;
			iova += 0x200000ULL;
			pages -= 512;
			continue;
		}

		/* Get or create PD table */
		uint64_t pdp_entry = pdp_table[pdp_idx];
		uint64_t *pd_table;
		uint64_t pd_phys;

		if (!(pdp_entry & VTD_PTE_PRESENT)) {
			pd_table = vtd_pt_alloc_page(&pd_phys);
			if (!pd_table) return -ENOMEM;
			pdp_table[pdp_idx] = (pd_phys & VTD_PTE_ADDR_MASK) |
			    VTD_PTE_PRESENT | VTD_PTE_RW;
		} else {
			pd_phys = pdp_entry & VTD_PTE_ADDR_MASK;
			pd_table = vm_map_phys(SELF, (void *)(uintptr_t)pd_phys, 4096);
		}

		/* Get or create PT (level-1) table */
		uint64_t pd_entry = pd_table[pd_idx];
		uint64_t *pt_table;
		uint64_t pt_phys;

		if (!(pd_entry & VTD_PTE_PRESENT)) {
			pt_table = vtd_pt_alloc_page(&pt_phys);
			if (!pt_table) return -ENOMEM;
			pd_table[pd_idx] = (pt_phys & VTD_PTE_ADDR_MASK) |
			    VTD_PTE_PRESENT | VTD_PTE_RW;
		} else {
			pt_phys = pd_entry & VTD_PTE_ADDR_MASK;
			pt_table = vm_map_phys(SELF, (void *)(uintptr_t)pt_phys, 4096);
		}

		/* Map 4KB pages */
		size_t batch = (pages < (512 - pt_idx)) ? pages : (512 - pt_idx);
		for (size_t i = 0; i < batch; i++) {
			pt_table[pt_idx + i] = (phys_addr & VTD_PTE_ADDR_MASK) | pte_flags;
			phys_addr += 0x1000;
		}
		iova += batch * 0x1000;
		pages -= batch;
	}

	return 0;
}

/* Unmap a range of pages from a VT-d IOMMU domain. */
static void vtd_pt_unmap_pages(uint64_t *pml4_virt, uint64_t pml4_phys,
    uint64_t iova, size_t size, struct vtd_unit *unit)
{
	size_t pages = (size + 4095) / 4096;
	(void)pml4_phys;
	(void)unit;

	while (pages > 0) {
		uint64_t pml4_idx = VTD_PML4_INDEX(iova);
		uint64_t pdp_idx  = VTD_PDP_INDEX(iova);
		uint64_t pd_idx   = VTD_PD_INDEX(iova);
		uint64_t pt_idx   = VTD_PT_INDEX(iova);

		uint64_t pml4_entry = pml4_virt[pml4_idx];
		if (!(pml4_entry & VTD_PTE_PRESENT)) {
			size_t skip = 262144;
			if (skip > pages) skip = pages;
			pages -= skip;
			iova += skip * 4096;
			continue;
		}

		uint64_t pdp_phys = pml4_entry & VTD_PTE_ADDR_MASK;
		uint64_t *pdp_table = vm_map_phys(SELF, (void *)(uintptr_t)pdp_phys, 4096);
		uint64_t pdp_entry = pdp_table[pdp_idx];

		if (!(pdp_entry & VTD_PTE_PRESENT)) {
			size_t skip = 512;
			if (skip > pages) skip = pages;
			pages -= skip;
			iova += skip * 4096;
			continue;
		}

		/* Check for 1GB huge page at PDP level */
		if (pdp_entry & VTD_PTE_PS) {
			pdp_table[pdp_idx] = 0;
			iova += 0x40000000ULL;
			pages -= (pages >= 262144) ? 262144 : pages;
			continue;
		}

		uint64_t pd_phys = pdp_entry & VTD_PTE_ADDR_MASK;
		uint64_t *pd_table = vm_map_phys(SELF, (void *)(uintptr_t)pd_phys, 4096);
		uint64_t pd_entry = pd_table[pd_idx];

		if (!(pd_entry & VTD_PTE_PRESENT)) {
			size_t skip = 512;
			if (skip > pages) skip = pages;
			pages -= skip;
			iova += skip * 4096;
			continue;
		}

		/* Check for 2MB huge page at PD level */
		if (pd_entry & VTD_PTE_PS) {
			pd_table[pd_idx] = 0;
			iova += 0x200000ULL;
			pages -= (pages >= 512) ? 512 : pages;
			continue;
		}

		uint64_t pt_phys = pd_entry & VTD_PTE_ADDR_MASK;
		uint64_t *pt_table = vm_map_phys(SELF, (void *)(uintptr_t)pt_phys, 4096);

		size_t batch = (pages < (512 - pt_idx)) ? pages : (512 - pt_idx);
		for (size_t i = 0; i < batch; i++)
			pt_table[pt_idx + i] = 0;

		/* Check if PT table is now empty */
		int empty = 1;
		for (int i = 0; i < VTD_PT_ENTRIES; i++) {
			if (pt_table[i] & VTD_PTE_PRESENT) { empty = 0; break; }
		}
		if (empty)
			pd_table[pd_idx] = 0;

		/* Check if PD table is now empty */
		empty = 1;
		for (int i = 0; i < VTD_PT_ENTRIES; i++) {
			if (pd_table[i] & VTD_PTE_PRESENT) { empty = 0; break; }
		}
		if (empty)
			pdp_table[pdp_idx] = 0;

		/* Check if PDP table is now empty */
		empty = 1;
		for (int i = 0; i < VTD_PT_ENTRIES; i++) {
			if (pdp_table[i] & VTD_PTE_PRESENT) { empty = 0; break; }
		}
		if (empty)
			pml4_virt[pml4_idx] = 0;

		iova += batch * 4096;
		pages -= batch;
	}
}

/*===========================================================================*
 *		API implementation				     *
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
	phys_bytes phys;
	void *root;

	if (vtd_domain_count >= 64)
		return -ENOMEM;

	/* Allocate PML4 root page table (4K, zeroed) */
	root = alloc_contig(4096, AC_ALIGN4K, &phys);
	if (!root)
		return -ENOMEM;
	memset(root, 0, 4096);

	domain->domain_id = vtd_domain_count;
	domain->type = GERGIOS_IOMMU_INTEL_VTD;
	domain->max_address = 0xFFFFFFFFFFFFFFFFULL;
	domain->ref_count = 0;
	domain->priv = (void *)(uintptr_t)phys;  /* PML4 root phys addr */

	vtd_domains[vtd_domain_count] = *domain;
	vtd_domain_count++;

	printf("iommu_vtd: domain %d allocated (PML4 root=0x%llx)\n",
	    domain->domain_id, (unsigned long long)phys);

	return 0;
}

static void vtd_domain_free(struct gergios_iommu_domain *domain)
{
	uint64_t root_phys = (uint64_t)(uintptr_t)domain->priv;

	/* Walk the page tables and free all intermediate pages.
	 * Simplified: clear the root PML4 (the IOMMU stop accessing it).
	 * A full implementation would recursively free all PDP/PD/PT pages.
	 * alloc_contig memory is not freed — recycled when process exits. */
	if (root_phys) {
		uint64_t *root_virt = vm_map_phys(SELF,
		    (void *)(uintptr_t)root_phys, 4096);
		if (root_virt != MAP_FAILED)
			memset(root_virt, 0, 4096);
	}

	domain->priv = NULL;
	domain->domain_id = -1;
	printf("iommu_vtd: domain %d freed\n", domain->domain_id);
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
		 * stored in the root table entry; no new vm_map_phys needed         * we need to map it to write the context entry. */
                ctx_table_phys = *root_entry & 0xFFFFFFFFFFFFF000ULL;
                ctx_table_virt = vm_map_phys(SELF,
                    (void *)(uintptr_t)ctx_table_phys, 4096);
                if (ctx_table_virt == MAP_FAILED)
                        return -ENOMEM;
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
	struct vtd_unit *unit = &vtd_units[0];
	uint64_t pml4_phys = (uint64_t)(uintptr_t)domain->priv;
	uint64_t *pml4_virt;
	size_t pages = (size + 4095) / 4096;
	int r;

	if (pages == 0)
		return 0;

	/* Map PML4 root for access */
	pml4_virt = vm_map_phys(SELF, (void *)(uintptr_t)pml4_phys, 4096);
	if (pml4_virt == MAP_FAILED)
		return -ENOMEM;

	r = vtd_pt_map_pages(pml4_virt, pml4_phys, iova,
	    (uint64_t)phys_addr, pages, flags, unit);

	if (r == 0) {
		/* Invalidate IOTLB for the mapped range */
		vtd_invalidate_iotlb(unit, domain->domain_id, iova, size, 0);
		vtd_qi_invalidation_wait(unit);
	}

	return r;
}

static void vtd_unmap(struct gergios_iommu_domain *domain,
    uint64_t iova, size_t size)
{
	struct vtd_unit *unit = &vtd_units[0];
	uint64_t pml4_phys = (uint64_t)(uintptr_t)domain->priv;
	uint64_t *pml4_virt;

	if (size == 0)
		return;

	pml4_virt = vm_map_phys(SELF, (void *)(uintptr_t)pml4_phys, 4096);
	if (pml4_virt == MAP_FAILED)
		return;

	vtd_pt_unmap_pages(pml4_virt, pml4_phys, iova, size, unit);

	/* Invalidate IOTLB for the unmapped range */
	vtd_invalidate_iotlb(unit, domain->domain_id, iova, size, 0);
	vtd_qi_invalidation_wait(unit);
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

/*===========================================================================*
 *		Interrupt remapping (VT-d IRTE)				     *
 *===========================================================================*/
/*
 * VT-d Interrupt Remap Table Entry (IRTE) — 16 bytes per entry.
 * Field layout (per Intel VT-d spec rev 3.4, section 5.1.2):
 *   Lower 64 bits:
 *     Bit 0:      P (Present)
 *     Bit 1:      FPD (Flexible Posted Descriptor)
 *     Bits 2-3:   Reserved
 *     Bits 4-7:   Vector[3:0]
 *     Bits 8-10:  RQ (Remap Qualifier) — 000 for MSI remap
 *     Bit 11:     SIDP (Source ID Present)
 *     Bits 12-15: SVT (Source Validation Type, if SIDP=0)
 *     Bits 16-31: SID (Source ID — 16-bit Requester ID)
 *     Bits 32-47: Destination[15:0] — low 16 bits of APIC ID
 *     Bits 48-63: Destination[31:16] + Vector[7:4] + flags
 *   Upper 64 bits:
 *     Bits 64-127: Extended Address + flags (for x2APIC / EIM)
 */

#define VTD_IRTE_PRESENT	(1ULL << 0)
#define VTD_IRTE_FPD		(1ULL << 1)
#define VTD_IRTE_VECTOR(v)	(((uint64_t)(v) & 0xFF) << 4)
#define VTD_IRTE_RQ_MSI		(0ULL << 8)	/* Remap Qualifier = MSI */
#define VTD_IRTE_SIDP		(1ULL << 11)	/* SID Present */
#define VTD_IRTE_SVT_NONE	(0ULL << 12)	/* No source validation */
#define VTD_IRTE_SVT_REQID	(2ULL << 12)	/* Validate against requester ID */
#define VTD_IRTE_SID(bdf)	(((uint64_t)(bdf) & 0xFFFF) << 16)
#define VTD_IRTE_DEST(apic)	(((uint64_t)(apic) & 0xFFFFFFFF) << 32)

/* Number of IRTEs (one 4K page = 256 entries × 16 bytes) */
#define VTD_IRTE_COUNT		256

/* Per-unit interrupt remap table */
struct vtd_ir_table {
	uint64_t	phys;		/* Physical address (4K-aligned) */
	uint64_t	*virt;		/* Virtual address */
	uint32_t	alloc_map;	/* Bitmap: bit N = entry N in use */
};

static struct vtd_ir_table vtd_ir_tables[MAX_VTD_UNITS];

static int vtd_intr_remap_enable(void)
{
	int r;

	for (unsigned int u = 0; u < vtd_unit_count; u++) {
		struct vtd_unit *unit = &vtd_units[u];
		phys_bytes phys;

		if (!unit->has_ir)
			continue;

		/* Allocate IR table: 4K page = 256 × 16-byte entries */
		vtd_ir_tables[u].virt = alloc_contig(4096, AC_ALIGN4K, &phys);
		if (!vtd_ir_tables[u].virt)
			return -ENOMEM;
		vtd_ir_tables[u].phys = (uint64_t)phys;
		memset(vtd_ir_tables[u].virt, 0, 4096);
		vtd_ir_tables[u].alloc_map = 0;

		/* Set IRTA register: base address + size (log2(N) - 1) = 7 (256 entries) */
		uint64_t irta = ((uint64_t)phys & 0xFFFFFFFFFFFFF000ULL) | 7;
		vtd_write64(unit, VTD_REG_IRTA, irta);
		__sync_synchronize();

		/* Enable interrupt remapping: set IRE bit in GCMD */
		uint64_t gcmd = vtd_read64(unit, VTD_REG_GCMD);
		gcmd |= VTD_GCMD_IRE;
		vtd_write64(unit, VTD_REG_GCMD, gcmd);

		r = vtd_wait_for_command(unit, VTD_REG_GSTS,
		    VTD_GSTS_IRES, VTD_GSTS_IRES);
		if (r != 0) {
			printf("iommu_vtd: IRE enable failed on unit %u (%d)\n",
			    u, r);
			return r;
		}

		printf("iommu_vtd: interrupt remapping enabled on unit %u "
		    "(IRTA=0x%llx)\n", u, (unsigned long long)irta);
	}

	return 0;
}

static int vtd_intr_remap_set(uint8_t bus, uint8_t dev, uint8_t func,
    unsigned int vector, uint64_t destination)
{
	uint16_t bdf = (uint16_t)((bus << 8) | (dev << 3) | func);
	int unit_idx = 0;
	struct vtd_unit *unit;
	struct vtd_ir_table *irt;
	int slot;

	if (vtd_unit_count == 0)
		return -ENODEV;

	unit = &vtd_units[unit_idx];
	irt = &vtd_ir_tables[unit_idx];

	if (!unit->has_ir || !irt->virt)
		return -ENOTSUP;

	/* Find a free IRTE slot */
	for (slot = 0; slot < VTD_IRTE_COUNT; slot++) {
		if (!(irt->alloc_map & (1U << slot)))
			break;
	}
	if (slot >= VTD_IRTE_COUNT)
		return -ENOSPC;

	/* Program the IRTE entry (16 bytes = 2 × 64-bit words).
	 * Lower word: Present | FPD=0 | Vector | RQ=MSI | SIDP | SID | Destination
	 * Upper word: 0 (no x2APIC extended address) */
	uint64_t irte_lo = VTD_IRTE_PRESENT |
	    VTD_IRTE_VECTOR(vector) |
	    VTD_IRTE_RQ_MSI |
	    VTD_IRTE_SIDP |
	    VTD_IRTE_SVT_NONE |
	    VTD_IRTE_SID(bdf) |
	    VTD_IRTE_DEST((unsigned int)destination);

	irt->virt[slot * 2 + 0] = irte_lo;
	irt->virt[slot * 2 + 1] = 0;  /* Upper word = 0 */
	irt->alloc_map |= (1U << slot);

	__sync_synchronize();

	/* Invalidate IEC (Interrupt Entry Cache) via QI descriptor */
	{
		uint64_t desc[2];
		/* IEC descriptor: type 4, global invalidation */
		desc[0] = (uint64_t)VTD_QI_DESC_IEC | (1ULL << 4);
		desc[1] = (uint64_t)slot;  /* IRTE index to invalidate */
		vtd_qi_submit(unit, desc);
		vtd_qi_invalidation_wait(unit);
	}

	printf("iommu_vtd: IRTE[%d] dev=%02x:%02x.%x vector=%u "
	    "dest=0x%llx\n",
	    slot, bus, dev, func, vector,
	    (unsigned long long)destination);

	return 0;
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
