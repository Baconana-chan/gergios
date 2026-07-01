/* iommu.h — Unified IOMMU Interface for GergiOS
 *
 * Provides a common abstraction over AMD-Vi and Intel VT-d IOMMU hardware.
 * The IOMMU backend is selected at boot time based on ACPI tables
 * (IVRS for AMD, DMAR for Intel) and PCI capabilities.
 *
 * Each IOMMU instance manages:
 *   - DMA remapping (address translation for PCI devices)
 *   - Interrupt remapping (optional, for MSI/MSI-X isolation)
 *   - Protection domains (isolated address spaces)
 *
 * The backend ops are registered by the AMD/VT-d implementations at init
 * and called through the unified API in dma.c.
 */

#ifndef _GERGIOS_IOMMU_H
#define _GERGIOS_IOMMU_H

#include <minix/config.h>
#include <minix/type.h>		/* phys_bytes */
#include <minix/endpoint.h>

/* Forward declarations */
struct gergios_device;

/*===========================================================================*
 *		IOMMU hardware types					     *
 *===========================================================================*/
enum gergios_iommu_type {
	GERGIOS_IOMMU_NONE	= 0,	/* No IOMMU present */
	GERGIOS_IOMMU_AMD_VI	= 1,	/* AMD-Vi (I/O Virtualization) */
	GERGIOS_IOMMU_INTEL_VTD = 2,	/* Intel VT-d (Virtualization Tech) */
};

/*===========================================================================*
 *		IOMMU domain — an isolated DMA address space		     *
 *===========================================================================*/
struct gergios_iommu_domain {
	int		domain_id;	/* Hardware domain/context number */
	enum gergios_iommu_type type;	/* Which IOMMU owns this domain */
	void	       *priv;		/* Backend-specific data (page table root) */
	uint64_t	max_address;	/* Maximum addressable bus address */
	unsigned int	ref_count;	/* Number of devices in this domain */
};

/*===========================================================================*
 *		IOMMU operations (backend dispatch table)		     *
 *===========================================================================*/
struct gergios_iommu_ops {

	/* --- Detection & initialisation --------------------------------- */

	/* Probe for IOMMU hardware.  Returns 1 if present, 0 if not. */
	int (*detect)(void);

	/* Initialise the IOMMU hardware.
	 * Called once at boot, before any device is attached.
	 * Returns 0 on success, negative errno on failure.
	 */
	int (*init_hw)(void);

	/* Shut down the IOMMU hardware (e.g., for S3 suspend). */
	void (*shutdown_hw)(void);

	/* --- Domain management ------------------------------------------ */

	/* Allocate a new IOMMU domain (address space).
	 * Returns 0 on success with domain populated, negative on error.
	 */
	int (*domain_alloc)(struct gergios_iommu_domain *domain);

	/* Free an IOMMU domain.  All page table entries are cleared. */
	void (*domain_free)(struct gergios_iommu_domain *domain);

	/* Attach a PCI device (identified by bus:dev:func) to a domain.
	 * After this, DMA from the device goes through the domain's page tables.
	 * Returns 0 on success, negative errno on failure.
	 */
	int (*domain_attach_device)(struct gergios_iommu_domain *domain,
	    uint8_t bus, uint8_t dev, uint8_t func);

	/* Detach a PCI device from a domain. */
	void (*domain_detach_device)(struct gergios_iommu_domain *domain,
	    uint8_t bus, uint8_t dev, uint8_t func);

	/* --- Page table management -------------------------------------- */

	/* Map a contiguous physical region into an IOMMU domain.
	 * 'iova' is the desired bus address (I/O virtual address);
	 * 'phys_addr' is the physical address; 'size' is the region size.
	 * Returns 0 on success, negative errno on failure.
	 */
	int (*map)(struct gergios_iommu_domain *domain,
	    uint64_t iova, phys_bytes phys_addr, size_t size, int flags);

	/* Unmap a region previously mapped with map(). */
	void (*unmap)(struct gergios_iommu_domain *domain,
	    uint64_t iova, size_t size);

	/* Create a 1:1 mapping (identity map) for a physical region.
	 * Used for legacy devices that require specific bus addresses.
	 */
	int (*identity_map)(struct gergios_iommu_domain *domain,
	    phys_bytes phys_addr, size_t size);

	/* --- TLB invalidation ------------------------------------------- */

	/* Invalidate IOTLB entries for a specific domain. */
	void (*iotlb_invalidate_domain)(struct gergios_iommu_domain *domain);

	/* Invalidate IOTLB entries for a specific address range. */
	void (*iotlb_invalidate_range)(struct gergios_iommu_domain *domain,
	    uint64_t iova, size_t size);

	/* Global IOTLB invalidation (all domains, all devices). */
	void (*iotlb_invalidate_all)(void);

	/* --- Interrupt remapping (optional) ----------------------------- */

	/* Enable interrupt remapping for this IOMMU.
	 * Returns 0 on success, negative errno if not supported.
	 */
	int (*intr_remap_enable)(void);

	/* Set up interrupt remapping entry for a device. */
	int (*intr_remap_set)(uint8_t bus, uint8_t dev, uint8_t func,
	    unsigned int vector, uint64_t destination);
};

/*===========================================================================*
 *		ACPI table header (used by both AMD and Intel backends)      *
 *===========================================================================*/

/* Minimal ACPI table header — matches ACPICA / UEFI spec */
struct acpi_sdt_header {
	char	signature[4];		/* "DMAR", "IVRS", etc. */
	uint32_t length;
	uint8_t	revision;
	uint8_t	checksum;
	char	oem_id[6];
	char	oem_table_id[8];
	uint32_t oem_revision;
	uint32_t creator_id;
	uint32_t creator_revision;
} __attribute__((packed));

/*===========================================================================*
 *		Shared ACPI table lookup (implemented in iommu.c)	     *
 *===========================================================================*/

/* Find the RSDP (Root System Description Pointer) in BIOS memory. */
uint64_t acpi_find_rsdp(void);

/* Find an ACPI system table by 4-byte signature (e.g. "DMAR", "IVRS").
 * Walks the RSDT/XSDT and returns a malloc'd copy of the table.
 * Caller must free() the returned pointer.  Returns NULL if not found. */
struct acpi_sdt_header *acpi_find_table(const char *sig);

/*===========================================================================*
 *		Public API — called by dma.c and drivers		     *
 *===========================================================================*/

/* Detect and initialise the system IOMMU.
 * Scans ACPI tables (RSDP → RSDT/XSDT → DMAR/IVRS) and PCI capabilities.
 * Returns the IOMMU type detected, or GERGIOS_IOMMU_NONE.
 */
enum gergios_iommu_type gergios_iommu_detect(void);

/* Get the IOMMU operations table for the detected IOMMU type.
 * Returns NULL if no IOMMU is present or initialised.
 */
const struct gergios_iommu_ops *gergios_iommu_get_ops(void);

/* Get the number of IOMMU hardware units detected. */
unsigned int gergios_iommu_unit_count(void);

/* Map a PCI device (bus:dev:func) to its IOMMU unit.
 * Returns the IOMMU unit index (0-based), or negative errno on failure.
 */
int gergios_iommu_device_to_unit(uint8_t bus, uint8_t dev, uint8_t func);

/* Allocate a DMA domain for a device.
 * Returns 0 on success, negative errno on failure.
 */
int gergios_iommu_domain_alloc(struct gergios_iommu_domain *domain);

/* Free a DMA domain. */
void gergios_iommu_domain_free(struct gergios_iommu_domain *domain);

#endif /* _GERGIOS_IOMMU_H */
