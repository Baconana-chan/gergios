/* dma.h — Unified DMA API for GergiOS
 *
 * Provides a clean DMA abstraction for GergiOS drivers, supporting:
 *   - IOMMU-backed DMA (AMD-Vi / Intel VT-d)
 *   - Direct DMA (physical addressing, for legacy/IOMMU-free systems)
 *   - Bounce buffers (for devices without 64-bit DMA on >4GB systems)
 *   - Scatter/gather DMA
 *   - DMA mask handling
 *   - Streaming DMA coherency management
 *
 * Designed as a per-device, function-pointer dispatch layer so that
 * the backend (IOMMU / direct / bounce) is selected at probe time.
 */

#ifndef _GERGIOS_DMA_H
#define _GERGIOS_DMA_H

#include <minix/config.h>
#include <minix/type.h>		/* phys_bytes */
#include <minix/endpoint.h>
#include <minix/syslib.h>	/* vm_adddma, vm_deldma, alloc_contig */

/* Forward declarations */
struct gergios_device;

/*===========================================================================*
 *		DMA direction (for streaming DMA)			     *
 *===========================================================================*/
enum gergios_dma_direction {
	GERGIOS_DMA_TO_DEVICE		= 0,	/* CPU → device (TX) */
	GERGIOS_DMA_FROM_DEVICE		= 1,	/* device → CPU (RX) */
	GERGIOS_DMA_BIDIRECTIONAL	= 2,	/* both directions */
	GERGIOS_DMA_NONE		= 3,	/* for debugging / validation */
};

/*===========================================================================*
 *		Scatter/gather list entry				     *
 *===========================================================================*/
struct gergios_scatterlist {
	uint64_t	dma_addr;	/* bus address for device */
	uint64_t	dma_len;	/* length of this segment */
	void	       *cpu_addr;	/* virtual address (CPU side) */
	uint32_t	offset;		/* offset within the page */
	uint32_t	length;		/* original length before mapping */
	unsigned int	dma_length;	/* length after DMA mapping */
};

/*===========================================================================*
 *		DMA operations (per-device dispatch table)		     *
 *===========================================================================*/
struct gergios_dma_ops {

	/* --- Coherent (consistent) DMA ---------------------------------- */

	/* Allocate a coherent DMA buffer (CPU- and device-visible).
	 * Returns 0 on success, negative errno on failure.
	 * The buffer is zeroed and cache-coherent.
	 */
	int (*alloc_coherent)(struct gergios_device *dev, size_t size,
	    void **cpu_addr, uint64_t *dma_handle);

	/* Free a coherent DMA buffer previously allocated with alloc_coherent. */
	void (*free_coherent)(struct gergios_device *dev, size_t size,
	    void *cpu_addr, uint64_t dma_handle);

	/* --- Streaming DMA (single buffer) ------------------------------ */

	/* Map a single buffer for streaming DMA.
	 * Returns the DMA address (valid for device), or 0 on error.
	 * After this call, the CPU must not touch the buffer until
	 * dma_unmap_single or dma_sync_single_for_cpu is called.
	 */
	uint64_t (*map_single)(struct gergios_device *dev, void *cpu_addr,
	    size_t size, enum gergios_dma_direction dir);

	/* Unmap a single buffer previously mapped with map_single. */
	void (*unmap_single)(struct gergios_device *dev, uint64_t dma_addr,
	    size_t size, enum gergios_dma_direction dir);

	/* --- Streaming DMA (scatter/gather) ----------------------------- */

	/* Map a scatter/gather list for DMA.
	 * Returns the number of DMA segments (≤ nents), or negative errno.
	 */
	int (*map_sg)(struct gergios_device *dev,
	    struct gergios_scatterlist *sg, int nents,
	    enum gergios_dma_direction dir);

	/* Unmap a scatter/gather list previously mapped with map_sg. */
	void (*unmap_sg)(struct gergios_device *dev,
	    struct gergios_scatterlist *sg, int nents,
	    enum gergios_dma_direction dir);

	/* --- DMA coherence management ----------------------------------- */

	/* Ensure DMA buffer is visible to the device (flush CPU caches). */
	void (*sync_single_for_device)(struct gergios_device *dev,
	    uint64_t dma_addr, size_t size,
	    enum gergios_dma_direction dir);

	/* Ensure DMA buffer is visible to the CPU (invalidate caches). */
	void (*sync_single_for_cpu)(struct gergios_device *dev,
	    uint64_t dma_addr, size_t size,
	    enum gergios_dma_direction dir);

	/* --- DMA mask / capabilities ------------------------------------ */

	/* Set the DMA mask (number of address bits the device can use).
	 * Returns 0 if the mask is supported, negative errno otherwise.
	 */
	int (*set_mask)(struct gergios_device *dev, uint64_t mask);

	/* Return the maximum DMA address the device can use. */
	uint64_t (*max_address)(struct gergios_device *dev);

	/* Return the IOMMU page size for the device (0 = no IOMMU). */
	size_t (*iommu_page_size)(struct gergios_device *dev);
};

/*===========================================================================*
 *		DMA backend types					     *
 *===========================================================================*/
enum gergios_dma_backend {
	GERGIOS_DMA_DIRECT	= 0,	/* Direct phys-addr DMA (no IOMMU) */
	GERGIOS_DMA_BOUNCE	= 1,	/* Software bounce buffers */
	GERGIOS_DMA_IOMMU_AMD	= 2,	/* AMD-Vi IOMMU */
	GERGIOS_DMA_IOMMU_VTD	= 3,	/* Intel VT-d IOMMU */
};

/*===========================================================================*
 *		Public API — called by drivers			     *
 *===========================================================================*/

/* Initialise the DMA subsystem.
 * Called once at system startup (before any driver probes).
 * Scans for available IOMMU hardware and selects the best backend.
 * Returns the selected backend type, or negative errno on failure.
 */
int gergios_dma_init(void);

/* Attach a device to the DMA subsystem.
 * Selects the appropriate DMA backend for the device based on
 * the system's IOMMU configuration.
 * Returns 0 on success, negative errno on failure.
 */
int gergios_dma_attach_device(struct gergios_device *dev);

/* Detach a device from the DMA subsystem (cleanup on driver unload). */
void gergios_dma_detach_device(struct gergios_device *dev);

/* Get the DMA ops for a device.
 * Devices must be attached first (gergios_dma_attach_device).
 * Returns NULL if the device is not attached.
 */
const struct gergios_dma_ops *gergios_dma_get_ops(struct gergios_device *dev);

/* Get the current DMA backend type for diagnostic / debug purposes. */
enum gergios_dma_backend gergios_dma_get_backend(void);

/* Convenience wrappers (call through dev->ops->dma, but check for NULL) */

static inline int
gergios_dma_alloc_coherent(struct gergios_device *dev, size_t size,
    void **cpu_addr, uint64_t *dma_handle)
{
	const struct gergios_dma_ops *ops = gergios_dma_get_ops(dev);
	if (!ops || !ops->alloc_coherent)
		return ENODEV;
	return ops->alloc_coherent(dev, size, cpu_addr, dma_handle);
}

static inline void
gergios_dma_free_coherent(struct gergios_device *dev, size_t size,
    void *cpu_addr, uint64_t dma_handle)
{
	const struct gergios_dma_ops *ops = gergios_dma_get_ops(dev);
	if (ops && ops->free_coherent)
		ops->free_coherent(dev, size, cpu_addr, dma_handle);
}

static inline uint64_t
gergios_dma_map_single(struct gergios_device *dev, void *cpu_addr,
    size_t size, enum gergios_dma_direction dir)
{
	const struct gergios_dma_ops *ops = gergios_dma_get_ops(dev);
	if (!ops || !ops->map_single)
		return 0;
	return ops->map_single(dev, cpu_addr, size, dir);
}

static inline void
gergios_dma_unmap_single(struct gergios_device *dev, uint64_t dma_addr,
    size_t size, enum gergios_dma_direction dir)
{
	const struct gergios_dma_ops *ops = gergios_dma_get_ops(dev);
	if (ops && ops->unmap_single)
		ops->unmap_single(dev, dma_addr, size, dir);
}

static inline int
gergios_dma_map_sg(struct gergios_device *dev,
    struct gergios_scatterlist *sg, int nents,
    enum gergios_dma_direction dir)
{
	const struct gergios_dma_ops *ops = gergios_dma_get_ops(dev);
	if (!ops || !ops->map_sg)
		return -ENODEV;
	return ops->map_sg(dev, sg, nents, dir);
}

static inline void
gergios_dma_unmap_sg(struct gergios_device *dev,
    struct gergios_scatterlist *sg, int nents,
    enum gergios_dma_direction dir)
{
	const struct gergios_dma_ops *ops = gergios_dma_get_ops(dev);
	if (ops && ops->unmap_sg)
		ops->unmap_sg(dev, sg, nents, dir);
}

static inline void
gergios_dma_sync_single_for_device(struct gergios_device *dev,
    uint64_t dma_addr, size_t size, enum gergios_dma_direction dir)
{
	const struct gergios_dma_ops *ops = gergios_dma_get_ops(dev);
	if (ops && ops->sync_single_for_device)
		ops->sync_single_for_device(dev, dma_addr, size, dir);
}

static inline void
gergios_dma_sync_single_for_cpu(struct gergios_device *dev,
    uint64_t dma_addr, size_t size, enum gergios_dma_direction dir)
{
	const struct gergios_dma_ops *ops = gergios_dma_get_ops(dev);
	if (ops && ops->sync_single_for_cpu)
		ops->sync_single_for_cpu(dev, dma_addr, size, dir);
}

static inline int
gergios_dma_set_mask(struct gergios_device *dev, uint64_t mask)
{
	const struct gergios_dma_ops *ops = gergios_dma_get_ops(dev);
	if (!ops || !ops->set_mask)
		return 0;  /* no IOMMU → always succeed */
	return ops->set_mask(dev, mask);
}

#endif /* _GERGIOS_DMA_H */
