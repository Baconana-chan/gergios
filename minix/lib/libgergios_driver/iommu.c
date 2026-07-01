/* iommu.c — IOMMU Detection and Dispatch for GergiOS
 *
 * Provides the shared ACPI table scanning logic (RSDP → RSDT/XSDT → DMAR/IVRS)
 * and the unified IOMMU type detection / ops dispatch used by both AMD-Vi
 * and Intel VT-d backends.
 *
 * Each backend (iommu_amd.c, iommu_vtd.c) registers its ops table here;
 * the highest-priority detected IOMMU is selected at boot.
 */

#include <minix/drivers.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <minix/sysutil.h>
#include <minix/type.h>
#include <minix/com.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "iommu.h"

/*===========================================================================*
 *		ACPI RSDP/RSDT scanning (shared by both backends)	     *
 *===========================================================================*/

/* Find the RSDP (Root System Description Pointer).
 * On x86_64, it's typically in the EBDA or at physical 0xE0000-0xFFFFF. */
uint64_t acpi_find_rsdp(void)
{
	unsigned long addr;
	uint8_t *p;
	char sig[8];

	/* Scan BIOS memory area (0xE0000 - 0xFFFFF) for RSDP */
	for (addr = 0xE0000; addr < 0x100000; addr += 16) {
		if (sys_readbios(addr, sig, 8) != OK)
			continue;
		if (memcmp(sig, "RSD PTR ", 8) == 0) {
			uint8_t rsdp_rev;
			if (sys_readbios(addr + 15, &rsdp_rev, 1) != OK)
				continue;
			if (rsdp_rev >= 2) {
				uint64_t xsdt_addr;
				if (sys_readbios(addr + 24, &xsdt_addr, 8) == OK)
					return xsdt_addr;
			}
			uint32_t rsdt_addr;
			if (sys_readbios(addr + 16, &rsdt_addr, 4) == OK)
				return (uint64_t)rsdt_addr;
		}
	}
	return 0;
}

/* Find an ACPI table by signature by walking the RSDT/XSDT.
 * Returns a malloc'd copy of the table, or NULL.  Caller must free. */
struct acpi_sdt_header *acpi_find_table(const char *sig)
{
	uint64_t rsdt_addr = acpi_find_rsdp();
	uint8_t buf[256];
	int entries, i;

	if (rsdt_addr == 0) {
		return NULL;
	}

	/* Read the RSDT/XSDT header */
	if (sys_readbios(rsdt_addr, buf, sizeof(struct acpi_sdt_header)) != OK) {
		return NULL;
	}

	struct acpi_sdt_header *hdr = (struct acpi_sdt_header *)buf;
	uint32_t length = hdr->length;
	int is_xsdt = (memcmp(hdr->signature, "XSDT", 4) == 0);

	/* Read the full table */
	uint8_t *table = malloc(length);
	if (!table) return NULL;
	if (sys_readbios(rsdt_addr, table, length) != OK) {
		free(table);
		return NULL;
	}

	/* Walk entry array */
	int entry_size = is_xsdt ? 8 : 4;
	entries = (length - sizeof(struct acpi_sdt_header)) / entry_size;

	for (i = 0; i < entries; i++) {
		uint64_t entry_addr;
		if (is_xsdt)
			entry_addr = ((uint64_t *)(table + sizeof(struct acpi_sdt_header)))[i];
		else
			entry_addr = ((uint32_t *)(table + sizeof(struct acpi_sdt_header)))[i];

		uint8_t entry_hdr[sizeof(struct acpi_sdt_header)];
		if (sys_readbios(entry_addr, entry_hdr, sizeof(entry_hdr)) != OK)
			continue;

		struct acpi_sdt_header *eh = (struct acpi_sdt_header *)entry_hdr;
		if (memcmp(eh->signature, sig, 4) == 0) {
			uint32_t tbl_len = eh->length;
			struct acpi_sdt_header *result = malloc(tbl_len);
			if (!result) { free(table); return NULL; }
			if (sys_readbios(entry_addr, (uint8_t *)result, tbl_len) != OK) {
				free(result); free(table); return NULL;
			}
			free(table);
			return result;
		}
	}

	free(table);
	return NULL;
}

/*===========================================================================*
 *		IOMMU detection & dispatch				     *
 *===========================================================================*/

/* External ops registration from backend files */
extern const struct gergios_iommu_ops *gergios_iommu_amd_get_ops(void);
extern const struct gergios_iommu_ops *gergios_iommu_vtd_get_ops(void);

/* Priority-ordered IOMMU backends */
static struct iommu_backend {
	enum gergios_iommu_type type;
	const char *name;
	const struct gergios_iommu_ops *(*get_ops)(void);
} backends[] = {
	{ GERGIOS_IOMMU_INTEL_VTD, "Intel VT-d", gergios_iommu_vtd_get_ops },
	{ GERGIOS_IOMMU_AMD_VI,    "AMD-Vi",    gergios_iommu_amd_get_ops },
	{ GERGIOS_IOMMU_NONE,      NULL,        NULL }
};

/* Detected state */
static enum gergios_iommu_type detected_type = GERGIOS_IOMMU_NONE;
static const struct gergios_iommu_ops *selected_ops = NULL;

enum gergios_iommu_type gergios_iommu_detect(void)
{
	if (detected_type != GERGIOS_IOMMU_NONE)
		return detected_type;

	/* Try each backend in priority order */
	for (int i = 0; backends[i].name != NULL; i++) {
		const struct gergios_iommu_ops *ops = backends[i].get_ops();
		if (!ops || !ops->detect)
			continue;

		if (ops->detect()) {
			detected_type = backends[i].type;
			selected_ops = ops;
			printf("gergios_iommu: %s detected\n", backends[i].name);
			return detected_type;
		}
	}

	printf("gergios_iommu: no IOMMU detected\n");
	return GERGIOS_IOMMU_NONE;
}

const struct gergios_iommu_ops *gergios_iommu_get_ops(void)
{
	return selected_ops;
}

unsigned int gergios_iommu_unit_count(void)
{
	/* This is backend-specific.  For now, return 1 if an IOMMU
	 * was detected, else 0.  A full implementation would track
	 * the number of hardware units per backend. */
	return (detected_type != GERGIOS_IOMMU_NONE) ? 1 : 0;
}

int gergios_iommu_device_to_unit(uint8_t bus, uint8_t dev, uint8_t func)
{
	/* Simplified: always return unit 0.  A full implementation
	 * would look up which IOMMU unit covers this bus:device. */
	(void)bus; (void)dev; (void)func;
	return (detected_type != GERGIOS_IOMMU_NONE) ? 0 : -ENODEV;
}

int gergios_iommu_domain_alloc(struct gergios_iommu_domain *domain)
{
	if (!selected_ops || !selected_ops->domain_alloc)
		return -ENODEV;
	return selected_ops->domain_alloc(domain);
}

void gergios_iommu_domain_free(struct gergios_iommu_domain *domain)
{
	if (selected_ops && selected_ops->domain_free)
		selected_ops->domain_free(domain);
}
