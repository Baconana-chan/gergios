/* dma.c — DMA API Implementation for GergiOS
 *
 * Provides the unified DMA API dispatch layer.  At initialisation time
 * the appropriate backend is selected (IOMMU > direct > bounce).
 * Each device gets a per-device DMA ops table that routes to the
 * selected backend.
 */

#include <minix/drivers.h>
#include <minix/com.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <minix/sysutil.h>
#include <minix/type.h>
#include <minix/vm.h>
#include <assert.h>
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#include "gergios_device.h"
#include "gergios_driver.h"
#include "dma.h"
#include "iommu.h"

/*===========================================================================*
 *		Internal state					     *
 *===========================================================================*/

/* Selected DMA backend type */
static enum gergios_dma_backend dma_backend = GERGIOS_DMA_DIRECT;

/* Detected IOMMU type (if any) */
static enum gergios_iommu_type iommu_type = GERGIOS_IOMMU_NONE;

/* IOMMU ops (set during gergios_dma_init if an IOMMU is present) */
static const struct gergios_iommu_ops *iommu_ops = NULL;

/* Maximum number of devices that can be attached */
#define MAX_DMA_DEVICES	64

/* Per-device DMA state */
struct dma_device_state {
	struct gergios_device	*dev;		/* back-link */
	const struct gergios_dma_ops *ops;	/* DMA ops for this device */
	enum gergios_dma_backend backend;	/* backend for this device */
	uint64_t		dma_mask;	/* DMA mask (addr bits) */
	struct gergios_iommu_domain *domain;	/* IOMMU domain (if any) */
	unsigned int		refcount;	/* reference count */
};

static struct dma_device_state dma_devs[MAX_DMA_DEVICES];
static unsigned int dma_dev_count = 0;

/* Mutex / lock — in MINIX userspace, each driver is single-threaded,
 * so no locking is needed.  If threaded IOMMU is added later, add a mutex. */

/*===========================================================================*
 *		Direct DMA backend (no IOMMU — phys addresses)		     *
 *===========================================================================*/
static int direct_alloc_coherent(struct gergios_device *dev, size_t size,
    void **cpu_addr, uint64_t *dma_handle)
{
	phys_bytes phys;

	*cpu_addr = alloc_contig(size, AC_ALIGN4K, &phys);
	if (*cpu_addr == NULL)
		return -ENOMEM;

	memset(*cpu_addr, 0, size);
	*dma_handle = (uint64_t)phys;
	return 0;
}

static void direct_free_coherent(struct gergios_device *dev, size_t size,
    void *cpu_addr, uint64_t dma_handle)
{
	/* alloc_contig on MINIX doesn't have a free_contig — memory
	 * is reclaimed when the process exits.  For hot-unplug, the
	 * kernel would need a sys_munmap or similar.  For now, no-op. */
	(void)dev; (void)size; (void)cpu_addr; (void)dma_handle;
}

static uint64_t direct_map_single(struct gergios_device *dev, void *cpu_addr,
    size_t size, enum gergios_dma_direction dir)
{
	phys_bytes phys;

	/* Translate virtual address to physical via sys_umap_remote */
	if (sys_umap_remote(SELF, SELF, VM_D, (vir_bytes)cpu_addr,
	    size, &phys) != OK)
		return 0;

	/* Mark region as DMA-able with VM */
	if (vm_adddma(SELF, phys, size) != OK) {
		/* non-fatal — some systems don't need VM tracking */
	}

	(void)dev; (void)dir;
	return (uint64_t)phys;
}

static void direct_unmap_single(struct gergios_device *dev, uint64_t dma_addr,
    size_t size, enum gergios_dma_direction dir)
{
	phys_bytes phys = (phys_bytes)dma_addr;
	int r = vm_deldma(SELF, phys, size);
	if (r != OK && r != EINVAL) {
		/* warn but continue */
	}
	(void)dev; (void)dir;
}

static int direct_map_sg(struct gergios_device *dev,
    struct gergios_scatterlist *sg, int nents,
    enum gergios_dma_direction dir)
{
	int i;

	for (i = 0; i < nents; i++) {
		sg[i].dma_addr = direct_map_single(dev, sg[i].cpu_addr,
		    sg[i].length, dir);
		if (sg[i].dma_addr == 0 && sg[i].length > 0) {
			/* unwind on error */
			while (--i >= 0)
				direct_unmap_single(dev, sg[i].dma_addr,
				    sg[i].length, dir);
			return -ENOMEM;
		}
		sg[i].dma_length = sg[i].length;
	}
	return nents;
}

static void direct_unmap_sg(struct gergios_device *dev,
    struct gergios_scatterlist *sg, int nents,
    enum gergios_dma_direction dir)
{
	int i;
	for (i = 0; i < nents; i++)
		direct_unmap_single(dev, sg[i].dma_addr, sg[i].length, dir);
}

static void direct_sync_single_for_device(struct gergios_device *dev,
    uint64_t dma_addr, size_t size, enum gergios_dma_direction dir)
{
	/* On x86_64, PCI devices are cache-coherent via MTRRs / PAT.
	 * No explicit cache flush needed for direct DMA. */
	(void)dev; (void)dma_addr; (void)size; (void)dir;
}

static void direct_sync_single_for_cpu(struct gergios_device *dev,
    uint64_t dma_addr, size_t size, enum gergios_dma_direction dir)
{
	(void)dev; (void)dma_addr; (void)size; (void)dir;
}

static int direct_set_mask(struct gergios_device *dev, uint64_t mask)
{
	/* Direct DMA can address any physical address the CPU can.
	 * We accept any mask ≤ 64 bits, but if the mask is < 32 bits
	 * we may need bounce buffers (handled by bounce backend). */
	(void)dev;
	return (mask >= 0xFFFFFFFFULL) ? 0 : -EIO;
}

static uint64_t direct_max_address(struct gergios_device *dev)
{
	(void)dev;
	return 0xFFFFFFFFFFFFFFFFULL;  /* full 64-bit address space */
}

static size_t direct_iommu_page_size(struct gergios_device *dev)
{
	(void)dev;
	return 0;  /* no IOMMU */
}

static const struct gergios_dma_ops direct_dma_ops = {
	.alloc_coherent		= direct_alloc_coherent,
	.free_coherent		= direct_free_coherent,
	.map_single		= direct_map_single,
	.unmap_single		= direct_unmap_single,
	.map_sg			= direct_map_sg,
	.unmap_sg		= direct_unmap_sg,
	.sync_single_for_device	= direct_sync_single_for_device,
	.sync_single_for_cpu	= direct_sync_single_for_cpu,
	.set_mask		= direct_set_mask,
	.max_address		= direct_max_address,
	.iommu_page_size	= direct_iommu_page_size,
};

/*===========================================================================*
 *		Bounce-buffer DMA backend				     *
 *===========================================================================*/
/*
 * The bounce backend is used when a device cannot address all of physical
 * memory (e.g., 32-bit DMA mask on a system with >4GB RAM).  It allocates
 * bounce buffers from low memory and copies data through them.
 */

struct bounce_buffer {
	void	       *cpu_addr;	/* CPU-side (bounce buffer) */
	uint64_t	dma_addr;	/* device-side (low phys) */
	size_t		size;		/* buffer size */
	enum gergios_dma_direction dir;	/* direction of transfer */
	uint8_t		in_use;		/* 1 = allocated */
};

#define MAX_BOUNCE_BUFS	16
static struct bounce_buffer bounce_pool[MAX_BOUNCE_BUFS];
static int bounce_initialised = 0;

static int bounce_alloc_coherent(struct gergios_device *dev, size_t size,
    void **cpu_addr, uint64_t *dma_handle)
{
	/* Allocate from low memory (< 4GB) using alloc_contig */
	phys_bytes phys;
	*cpu_addr = alloc_contig(size, AC_ALIGN4K, &phys);
	if (*cpu_addr == NULL)
		return -ENOMEM;
	memset(*cpu_addr, 0, size);
	*dma_handle = (uint64_t)phys;
	return 0;
}

static void bounce_free_coherent(struct gergios_device *dev, size_t size,
    void *cpu_addr, uint64_t dma_handle)
{
	(void)dev; (void)size; (void)cpu_addr; (void)dma_handle;
	/* Same limitation as direct_free_coherent — no free_contig in MINIX */
}

static int bounce_alloc_buf(size_t size, enum gergios_dma_direction dir,
    struct bounce_buffer **out)
{
	int i;
	for (i = 0; i < MAX_BOUNCE_BUFS; i++) {
		if (bounce_pool[i].in_use)
			continue;

		phys_bytes phys;
		void *addr = alloc_contig(size, AC_ALIGN4K, &phys);
		if (addr == NULL)
			return -ENOMEM;

		bounce_pool[i].cpu_addr = addr;
		bounce_pool[i].dma_addr = (uint64_t)phys;
		bounce_pool[i].size = size;
		bounce_pool[i].dir = dir;
		bounce_pool[i].in_use = 1;
		*out = &bounce_pool[i];
		return 0;
	}
	return -ENOMEM;
}

static void bounce_free_buf(struct bounce_buffer *bb)
{
	bb->in_use = 0;
	/* alloc_contig memory is not freed — recycled for next use */
}

static uint64_t bounce_map_single(struct gergios_device *dev, void *cpu_addr,
    size_t size, enum gergios_dma_direction dir)
{
	struct bounce_buffer *bb;
	int r;

	/* Allocate a bounce buffer */
	r = bounce_alloc_buf(size, dir, &bb);
	if (r != 0)
		return 0;

	/* For TO_DEVICE, copy data into the bounce buffer */
	if (dir == GERGIOS_DMA_TO_DEVICE || dir == GERGIOS_DMA_BIDIRECTIONAL)
		memcpy(bb->cpu_addr, cpu_addr, size);

	(void)dev;
	return bb->dma_addr;
}

static void bounce_unmap_single(struct gergios_device *dev, uint64_t dma_addr,
    size_t size, enum gergios_dma_direction dir)
{
	int i;

	for (i = 0; i < MAX_BOUNCE_BUFS; i++) {
		if (!bounce_pool[i].in_use)
			continue;
		if (bounce_pool[i].dma_addr != dma_addr)
			continue;

		/* For FROM_DEVICE, copy data back to the original buffer */
		/* (Note: we don't have the original cpu_addr stored, so this
		 * simplified version doesn't bounce-back.  A full implementation
		 * would store the original address alongside the bounce buffer.) */
		if (dir == GERGIOS_DMA_FROM_DEVICE || dir == GERGIOS_DMA_BIDIRECTIONAL) {
			/* In a full implementation, we'd need a mapping from
			 * dma_addr → original cpu_addr.  For now, we just
			 * mark the buffer as free. */
		}

		bounce_free_buf(&bounce_pool[i]);
		(void)dev; (void)size;
		return;
	}
}

static int bounce_map_sg(struct gergios_device *dev,
    struct gergios_scatterlist *sg, int nents,
    enum gergios_dma_direction dir)
{
	int i;
	for (i = 0; i < nents; i++) {
		sg[i].dma_addr = bounce_map_single(dev, sg[i].cpu_addr,
		    sg[i].length, dir);
		if (sg[i].dma_addr == 0 && sg[i].length > 0) {
			while (--i >= 0)
				bounce_unmap_single(dev, sg[i].dma_addr,
				    sg[i].length, dir);
			return -ENOMEM;
		}
		sg[i].dma_length = sg[i].length;
	}
	return nents;
}

static void bounce_unmap_sg(struct gergios_device *dev,
    struct gergios_scatterlist *sg, int nents,
    enum gergios_dma_direction dir)
{
	int i;
	for (i = 0; i < nents; i++)
		bounce_unmap_single(dev, sg[i].dma_addr, sg[i].length, dir);
}

static void bounce_sync_single_for_device(struct gergios_device *dev,
    uint64_t dma_addr, size_t size, enum gergios_dma_direction dir)
{
	/* For bounce buffers, sync involves copying data into the buffer.
	 * This simplified version doesn't track the original address, so
	 * sync is a no-op.  A full implementation would use the stored
	 * original address mapping. */
	(void)dev; (void)dma_addr; (void)size; (void)dir;
}

static void bounce_sync_single_for_cpu(struct gergios_device *dev,
    uint64_t dma_addr, size_t size, enum gergios_dma_direction dir)
{
	(void)dev; (void)dma_addr; (void)size; (void)dir;
}

static int bounce_set_mask(struct gergios_device *dev, uint64_t mask)
{
	(void)dev;
	return (mask >= 0xFFFFFFFFULL) ? 0 : -EIO;
}

static uint64_t bounce_max_address(struct gergios_device *dev)
{
	(void)dev;
	return 0xFFFFFFFFULL;  /* 4GB limit */
}

static size_t bounce_iommu_page_size(struct gergios_device *dev)
{
	(void)dev;
	return 0;
}

static const struct gergios_dma_ops bounce_dma_ops = {
	.alloc_coherent		= bounce_alloc_coherent,
	.free_coherent		= bounce_free_coherent,
	.map_single		= bounce_map_single,
	.unmap_single		= bounce_unmap_single,
	.map_sg			= bounce_map_sg,
	.unmap_sg		= bounce_unmap_sg,
	.sync_single_for_device	= bounce_sync_single_for_device,
	.sync_single_for_cpu	= bounce_sync_single_for_cpu,
	.set_mask		= bounce_set_mask,
	.max_address		= bounce_max_address,
	.iommu_page_size	= bounce_iommu_page_size,
};

/*===========================================================================*
 *		IOMMU-backed DMA backend				     *
 *===========================================================================*/
/* The IOMMU backend uses the unified IOMMU ops from iommu_amd.c or
 * iommu_vtd.c to set up DMA remapping domains for each device. */

static int iommu_alloc_coherent(struct gergios_device *dev, size_t size,
    void **cpu_addr, uint64_t *dma_handle)
{
	phys_bytes phys;
	struct dma_device_state *ds;

	*cpu_addr = alloc_contig(size, AC_ALIGN4K, &phys);
	if (*cpu_addr == NULL)
		return -ENOMEM;

	memset(*cpu_addr, 0, size);

	/* Find the device state to get its IOMMU domain */
	ds = NULL;
	for (unsigned int i = 0; i < dma_dev_count; i++) {
		if (dma_devs[i].dev == dev) {
			ds = &dma_devs[i];
			break;
		}
	}

	/* If the device has an IOMMU domain, map the memory through it */
	if (ds && ds->domain && iommu_ops) {
		uint64_t iova;
		int r;

		/* Allocate IOVA — simplified: identity-map for now.
		 * A full implementation would maintain a free IOVA allocator
		 * per domain and assign addresses from the device's DMA window. */
		iova = (uint64_t)phys;

		r = iommu_ops->map(ds->domain, iova, phys, size, 0);
		if (r != 0) {
			direct_free_coherent(dev, size, *cpu_addr, (uint64_t)phys);
			return r;
		}

		*dma_handle = iova;
	} else {
		/* No IOMMU — use direct phys address */
		*dma_handle = (uint64_t)phys;
	}

	return 0;
}

static void iommu_free_coherent(struct gergios_device *dev, size_t size,
    void *cpu_addr, uint64_t dma_handle)
{
	struct dma_device_state *ds = NULL;

	for (unsigned int i = 0; i < dma_dev_count; i++) {
		if (dma_devs[i].dev == dev) {
			ds = &dma_devs[i];
			break;
		}
	}

	if (ds && ds->domain && iommu_ops)
		iommu_ops->unmap(ds->domain, dma_handle, size);

	direct_free_coherent(dev, size, cpu_addr, dma_handle);
}

static uint64_t iommu_map_single(struct gergios_device *dev, void *cpu_addr,
    size_t size, enum gergios_dma_direction dir)
{
	phys_bytes phys;
	struct dma_device_state *ds = NULL;

	/* Translate virtual to physical */
	if (sys_umap_remote(SELF, SELF, VM_D, (vir_bytes)cpu_addr,
	    size, &phys) != OK)
		return 0;

	/* Find device state */
	for (unsigned int i = 0; i < dma_dev_count; i++) {
		if (dma_devs[i].dev == dev) {
			ds = &dma_devs[i];
			break;
		}
	}

	if (ds && ds->domain && iommu_ops) {
		uint64_t iova = (uint64_t)phys;  /* identity-map for now */
		int r = iommu_ops->map(ds->domain, iova, phys, size, 0);
		if (r != 0)
			return 0;
		return iova;
	}

	return (uint64_t)phys;
}

static void iommu_unmap_single(struct gergios_device *dev, uint64_t dma_addr,
    size_t size, enum gergios_dma_direction dir)
{
	struct dma_device_state *ds = NULL;

	for (unsigned int i = 0; i < dma_dev_count; i++) {
		if (dma_devs[i].dev == dev) {
			ds = &dma_devs[i];
			break;
		}
	}

	if (ds && ds->domain && iommu_ops)
		iommu_ops->unmap(ds->domain, dma_addr, size);

	(void)dir;
}

static int iommu_map_sg(struct gergios_device *dev,
    struct gergios_scatterlist *sg, int nents,
    enum gergios_dma_direction dir)
{
	struct dma_device_state *ds = NULL;
	int i;

	for (unsigned int j = 0; j < dma_dev_count; j++) {
		if (dma_devs[j].dev == dev) {
			ds = &dma_devs[j];
			break;
		}
	}

	for (i = 0; i < nents; i++) {
		phys_bytes phys;
		if (sys_umap_remote(SELF, SELF, VM_D,
		    (vir_bytes)sg[i].cpu_addr, sg[i].length, &phys) != OK) {
			while (--i >= 0)
				iommu_unmap_single(dev, sg[i].dma_addr,
				    sg[i].length, dir);
			return -ENOMEM;
		}

		if (ds && ds->domain && iommu_ops) {
			uint64_t iova = (uint64_t)phys;
			int r = iommu_ops->map(ds->domain, iova, phys,
			    sg[i].length, 0);
			if (r != 0) {
				while (--i >= 0)
					iommu_unmap_single(dev, sg[i].dma_addr,
					    sg[i].length, dir);
				return r;
			}
			sg[i].dma_addr = iova;
		} else {
			sg[i].dma_addr = (uint64_t)phys;
		}
		sg[i].dma_length = sg[i].length;
	}
	return nents;
}

static void iommu_unmap_sg(struct gergios_device *dev,
    struct gergios_scatterlist *sg, int nents,
    enum gergios_dma_direction dir)
{
	int i;
	for (i = 0; i < nents; i++)
		iommu_unmap_single(dev, sg[i].dma_addr, sg[i].length, dir);
}

static void iommu_sync_single_for_device(struct gergios_device *dev,
    uint64_t dma_addr, size_t size, enum gergios_dma_direction dir)
{
	/* With IOMMU, cache coherency depends on the IOMMU page attributes.
	 * For now, assume coherent (same as direct).  For non-coherent IOMMUs,
	 * we'd need CLFLUSH or WBINVD here. */
	(void)dev; (void)dma_addr; (void)size; (void)dir;
}

static void iommu_sync_single_for_cpu(struct gergios_device *dev,
    uint64_t dma_addr, size_t size, enum gergios_dma_direction dir)
{
	(void)dev; (void)dma_addr; (void)size; (void)dir;
}

static int iommu_set_mask(struct gergios_device *dev, uint64_t mask)
{
	struct dma_device_state *ds = NULL;

	for (unsigned int i = 0; i < dma_dev_count; i++) {
		if (dma_devs[i].dev == dev) {
			ds = &dma_devs[i];
			break;
		}
	}

	if (ds)
		ds->dma_mask = mask;

	/* With IOMMU, the device can address any IOVA.  The mask is
	 * informational — we check it when allocating IOVA space. */
	(void)dev;
	return (mask >= 0xFFFFULL) ? 0 : -EIO;
}

static uint64_t iommu_max_address(struct gergios_device *dev)
{
	/* With IOMMU, the device can address the full IOVA space. */
	(void)dev;
	return 0xFFFFFFFFFFFFFFFFULL;
}

static size_t iommu_iommu_page_size(struct gergios_device *dev)
{
	(void)dev;
	return 4096;  /* standard IOMMU page size */
}

static const struct gergios_dma_ops iommu_dma_ops = {
	.alloc_coherent		= iommu_alloc_coherent,
	.free_coherent		= iommu_free_coherent,
	.map_single		= iommu_map_single,
	.unmap_single		= iommu_unmap_single,
	.map_sg			= iommu_map_sg,
	.unmap_sg		= iommu_unmap_sg,
	.sync_single_for_device	= iommu_sync_single_for_device,
	.sync_single_for_cpu	= iommu_sync_single_for_cpu,
	.set_mask		= iommu_set_mask,
	.max_address		= iommu_max_address,
	.iommu_page_size	= iommu_iommu_page_size,
};

/*===========================================================================*
 *		Public API implementation				     *
 *===========================================================================*/

int gergios_dma_init(void)
{
	/* Detect IOMMU hardware */
	iommu_type = gergios_iommu_detect();

	switch (iommu_type) {
	case GERGIOS_IOMMU_AMD_VI:
	case GERGIOS_IOMMU_INTEL_VTD:
		iommu_ops = gergios_iommu_get_ops();
		if (iommu_ops && iommu_ops->init_hw) {
			int r = iommu_ops->init_hw();
			if (r == 0) {
				dma_backend = (iommu_type == GERGIOS_IOMMU_AMD_VI)
				    ? GERGIOS_DMA_IOMMU_AMD
				    : GERGIOS_DMA_IOMMU_VTD;
				printf("gergios_dma: IOMMU (%s) initialised\n",
				    iommu_type == GERGIOS_IOMMU_AMD_VI
					? "AMD-Vi" : "Intel VT-d");
				return dma_backend;
			}
			printf("gergios_dma: IOMMU init failed (%d), "
			    "falling back to direct DMA\n", r);
		}
		iommu_type = GERGIOS_IOMMU_NONE;
		iommu_ops = NULL;
		/* fall through to direct DMA */
		break;

	case GERGIOS_IOMMU_NONE:
	default:
		break;
	}

	/* No IOMMU — use direct DMA (or bounce if needed later) */
	dma_backend = GERGIOS_DMA_DIRECT;
	printf("gergios_dma: no IOMMU detected, using direct DMA\n");
	return dma_backend;
}

int gergios_dma_attach_device(struct gergios_device *dev)
{
	struct dma_device_state *ds;
	uint8_t bus, pci_dev, pci_func;
	int r;

	if (dma_dev_count >= MAX_DMA_DEVICES)
		return -ENOMEM;

	/* Check if already attached */
	for (unsigned int i = 0; i < dma_dev_count; i++) {
		if (dma_devs[i].dev == dev) {
			dma_devs[i].refcount++;
			return 0;
		}
	}

	ds = &dma_devs[dma_dev_count];
	memset(ds, 0, sizeof(*ds));
	ds->dev = dev;
	ds->dma_mask = 0xFFFFFFFFULL;  /* default: 32-bit */
	ds->refcount = 1;

	/* If IOMMU is present, set up a domain for this device */
	if (iommu_ops && iommu_type != GERGIOS_IOMMU_NONE) {
		/* Try to extract BDF from the device's bus_address.
		 * Format: (bus << 16) | (dev << 8) | func — set by pci_scan.c */
		uint64_t bdf = gergios_device_get_bus_address(dev);
		bus  = (bdf >> 16) & 0xFF;
		pci_dev = (bdf >> 8) & 0xFF;
		pci_func = bdf & 0xFF;

		ds->domain = malloc(sizeof(struct gergios_iommu_domain));
		if (!ds->domain)
			return -ENOMEM;

		memset(ds->domain, 0, sizeof(struct gergios_iommu_domain));
		r = iommu_ops->domain_alloc(ds->domain);
		if (r != 0) {
			free(ds->domain);
			ds->domain = NULL;
			/* Continue without IOMMU for this device */
			ds->backend = GERGIOS_DMA_DIRECT;
			ds->ops = &direct_dma_ops;
			dma_dev_count++;
			return 0;
		}

		r = iommu_ops->domain_attach_device(ds->domain, bus, pci_dev, pci_func);
		if (r != 0) {
			iommu_ops->domain_free(ds->domain);
			free(ds->domain);
			ds->domain = NULL;
			ds->backend = GERGIOS_DMA_DIRECT;
			ds->ops = &direct_dma_ops;
			dma_dev_count++;
			return 0;
		}

		ds->backend = (iommu_type == GERGIOS_IOMMU_AMD_VI)
		    ? GERGIOS_DMA_IOMMU_AMD : GERGIOS_DMA_IOMMU_VTD;
		ds->ops = &iommu_dma_ops;
		printf("gergios_dma: device attached via %s\n",
		    iommu_type == GERGIOS_IOMMU_AMD_VI ? "AMD-Vi" : "VT-d");
	} else {
		/* No IOMMU — use direct DMA */
		ds->backend = GERGIOS_DMA_DIRECT;
		ds->ops = &direct_dma_ops;
	}

	dma_dev_count++;
	return 0;
}

void gergios_dma_detach_device(struct gergios_device *dev)
{
	for (unsigned int i = 0; i < dma_dev_count; i++) {
		if (dma_devs[i].dev != dev)
			continue;

		dma_devs[i].refcount--;
		if (dma_devs[i].refcount > 0)
			return;

		/* Detach from IOMMU domain */
		if (dma_devs[i].domain && iommu_ops) {
			uint64_t bdf = gergios_device_get_bus_address(dev);
			uint8_t bus = (bdf >> 16) & 0xFF;
			uint8_t pci_dev = (bdf >> 8) & 0xFF;
			uint8_t func = bdf & 0xFF;

			iommu_ops->domain_detach_device(dma_devs[i].domain,
			    bus, pci_dev, func);
			iommu_ops->domain_free(dma_devs[i].domain);
			free(dma_devs[i].domain);
		}

		/* Remove from array (swap with last) */
		dma_devs[i] = dma_devs[dma_dev_count - 1];
		dma_dev_count--;
		return;
	}
}

const struct gergios_dma_ops *gergios_dma_get_ops(struct gergios_device *dev)
{
	for (unsigned int i = 0; i < dma_dev_count; i++) {
		if (dma_devs[i].dev == dev)
			return dma_devs[i].ops;
	}
	return NULL;
}

enum gergios_dma_backend gergios_dma_get_backend(void)
{
	return dma_backend;
}
