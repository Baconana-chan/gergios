/* kernel_shim.c — Linux Kernel API Shim Implementation
 *
 * Provides ~50 Linux kernel API functions for use by LKM drivers loaded
 * via the ELF .ko loader.  Each function maps to the corresponding
 * MINIX/GergiOS facility (alloc_contig, sys_irqsetpolicy, vm_map_phys, etc.)
 *
 * The shim is compiled into libgergios_driver and registered as
 * host symbols for resolution during ELF relocation.
 */

#include <minix/drivers.h>
#include <minix/syslib.h>
#include <minix/sysutil.h>
#include <minix/type.h>
#include <minix/com.h>
#include <minix/endpoint.h>
#include <minix/ds.h>

#include <sys/mman.h>
#include <sys/stat.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>
#include <time.h>

#include "kernel_shim.h"

/*===========================================================================*
 *		Internal state                                             *
 *===========================================================================*/

/* Maximum number of registered IRQ handlers */
#define MAX_IRQ_HANDLERS    32

struct irq_handler {
	unsigned int irq;
	irq_handler_t handler;
	irq_handler_t thread_fn;
	void *dev_id;
	const char *name;
	int hook_id;        /* MINIX IRQ hook ID */
	int in_use;
};

static struct irq_handler irq_handlers[MAX_IRQ_HANDLERS];
static int irq_initialised = 0;

/* jiffies — tick counter updated by klkm_timer_dispatch */
unsigned long volatile jiffies = 0;

/*===========================================================================*
 *		jiffies helpers                                           *
 *===========================================================================*/

unsigned long get_jiffies_64(void)
{
	return jiffies;
}

unsigned long msecs_to_jiffies(const unsigned int m)
{
	unsigned long hz = sys_hz();
	return (m * hz + 999) / 1000;
}

unsigned int jiffies_to_msecs(const unsigned long j)
{
	unsigned long hz = sys_hz();
	return (j * 1000 + hz - 1) / hz;
}

unsigned long usecs_to_jiffies(const unsigned int u)
{
	unsigned long hz = sys_hz();
	return (u * hz + 999999) / 1000000;
}

unsigned int jiffies_to_usecs(const unsigned long j)
{
	unsigned long hz = sys_hz();
	return (j * 1000000 + hz - 1) / hz;
}

/*===========================================================================*
 *		Memory allocation                                          *
 *===========================================================================*/

void *kmalloc(size_t size, int gfp_flags)
{
	void *p = malloc(size);
	(void)gfp_flags;
	if (p && (gfp_flags & __GFP_ZERO))
		memset(p, 0, size);
	return p;
}

void *kzalloc(size_t size, int gfp_flags)
{
	void *p = malloc(size);
	(void)gfp_flags;
	if (p)
		memset(p, 0, size);
	return p;
}

void kfree(const void *ptr)
{
	free((void *)ptr);
}

void *kcalloc(size_t n, size_t size, int gfp_flags)
{
	(void)gfp_flags;
	return calloc(n, size);
}

void *krealloc(void *ptr, size_t size, int gfp_flags)
{
	(void)gfp_flags;
	return realloc(ptr, size);
}

void *vzalloc(size_t size)
{
	void *p = malloc(size);
	if (p)
		memset(p, 0, size);
	return p;
}

void vfree(const void *addr)
{
	free((void *)addr);
}

/*===========================================================================*
 *		PCI config space access wrappers                          *
 *===========================================================================*/

/* These wrap the MINIX PCI config space access functions.
 * pci_attr_r8/r16/r32 are declared in libsys but not in public headers. */
extern u8_t  pci_attr_r8(int devind, int port);
extern u16_t pci_attr_r16(int devind, int port);
extern u32_t pci_attr_r32(int devind, int port);
extern void  pci_attr_w32(int devind, int port, u32_t val);

int pci_read_config_byte(const struct pci_dev *dev, int where, u8 *val)
{
	if (!dev || !val) return -EINVAL;
	*val = pci_attr_r8(dev->devind, where);
	return 0;
}

int pci_read_config_word(const struct pci_dev *dev, int where, u16 *val)
{
	if (!dev || !val) return -EINVAL;
	*val = pci_attr_r16(dev->devind, where);
	return 0;
}

int pci_read_config_dword(const struct pci_dev *dev, int where, u32 *val)
{
	if (!dev || !val) return -EINVAL;
	*val = pci_attr_r32(dev->devind, where);
	return 0;
}

int pci_write_config_byte(const struct pci_dev *dev, int where, u8 val)
{
	if (!dev) return -EINVAL;
	/* MINIX doesn't have pci_attr_w8 — use RMW via w32 where possible */
	u32 tmp = pci_attr_r32(dev->devind, where & ~3);
	int shift = (where & 3) * 8;
	tmp = (tmp & ~(0xFFu << shift)) | ((u32)val << shift);
	pci_attr_w32(dev->devind, where & ~3, tmp);
	return 0;
}

int pci_write_config_word(const struct pci_dev *dev, int where, u16 val)
{
	if (!dev) return -EINVAL;
	u32 tmp = pci_attr_r32(dev->devind, where & ~3);
	int shift = (where & 2) ? 16 : 0;
	tmp = (tmp & ~(0xFFFFu << shift)) | ((u32)val << shift);
	pci_attr_w32(dev->devind, where & ~3, tmp);
	return 0;
}

int pci_write_config_dword(const struct pci_dev *dev, int where, u32 val)
{
	if (!dev) return -EINVAL;
	pci_attr_w32(dev->devind, where, val);
	return 0;
}

/*===========================================================================*
 *		PCI BAR access wrappers                                   *
 *===========================================================================*/

extern int pci_get_bar(int devind, int port, u32_t *base, u32_t *size,
                       int *ioflag);

resource_size_t pci_resource_start(const struct pci_dev *dev, int bar)
{
	if (!dev || bar < 0 || bar > 5) return 0;
	return dev->resource[bar].start;
}

resource_size_t pci_resource_end(const struct pci_dev *dev, int bar)
{
	if (!dev || bar < 0 || bar > 5) return 0;
	return dev->resource[bar].end;
}

resource_size_t pci_resource_len(const struct pci_dev *dev, int bar)
{
	if (!dev || bar < 0 || bar > 5) return 0;
	return dev->resource[bar].end - dev->resource[bar].start + 1;
}

unsigned int pci_irq_vector(const struct pci_dev *dev, unsigned int nr)
{
	if (!dev) return 0;
	return dev->irq;
}

/*===========================================================================*
 *		MMIO mapping (with size tracking)                         *
 *===========================================================================*/

#define MAX_IOMMAP_REGIONS	32

struct iommap_entry {
	void  *virt;
	void  *phys;
	size_t size;
	int    in_use;
};

static struct iommap_entry iommap_regions[MAX_IOMMAP_REGIONS];
static int iommap_initialised = 0;

void __iomem *ioremap(phys_addr_t phys_addr, unsigned long size)
{
	int i;

	/* Initialise tracking table on first use */
	if (!iommap_initialised) {
		memset(iommap_regions, 0, sizeof(iommap_regions));
		iommap_initialised = 1;
	}

	/* MINIX uses vm_map_phys() for mapping physical addresses.
	 * Returns MAP_FAILED on error. */
	void *virt = vm_map_phys(SELF, (void *)(uintptr_t)phys_addr, size);
	if (virt == MAP_FAILED)
		return NULL;

	/* Track the mapping for proper unmap later */
	for (i = 0; i < MAX_IOMMAP_REGIONS; i++) {
		if (!iommap_regions[i].in_use) {
			iommap_regions[i].virt   = virt;
			iommap_regions[i].phys   = (void *)(uintptr_t)phys_addr;
			iommap_regions[i].size   = size;
			iommap_regions[i].in_use = 1;
			break;
		}
	}

	if (i >= MAX_IOMMAP_REGIONS)
		printf("kernel_shim: WARNING — ioremap tracking table full!\n");

	return (void __iomem *)virt;
}

void __iomem *ioremap_nocache(phys_addr_t phys_addr, unsigned long size)
{
	return ioremap(phys_addr, size);
}

void iounmap(void __iomem *addr)
{
	if (!addr) return;

	/* Find the mapping in the tracking table */
	for (int i = 0; i < MAX_IOMMAP_REGIONS; i++) {
		if (iommap_regions[i].in_use && iommap_regions[i].virt == addr) {
			vm_unmap_phys(SELF, iommap_regions[i].virt,
			              iommap_regions[i].size);
			iommap_regions[i].in_use = 0;
			return;
		}
	}

	/* Not found in tracking table — try unmap with size 0 as fallback */
	vm_unmap_phys(SELF, (void *)addr, 0);
}

/*===========================================================================*
 *		PCI device enable / master                                *
 *===========================================================================*/

int pci_enable_device(struct pci_dev *dev)
{
	if (!dev) return -EINVAL;

	/* Read the current command register */
	u16 cmd;
	pci_read_config_word(dev, 0x04, &cmd);

	/* Enable bus mastering, memory space, and I/O space */
	cmd |= 0x0007;  /* IO | MEM | BUS_MASTER */
	pci_write_config_word(dev, 0x04, cmd);

	return 0;
}

void pci_disable_device(struct pci_dev *dev)
{
	if (!dev) return;

	/* Disable memory and I/O space (keep bus mastering until set_master) */
	u16 cmd;
	pci_read_config_word(dev, 0x04, &cmd);
	cmd &= ~0x0003;  /* IO | MEM */
	pci_write_config_word(dev, 0x04, cmd);
}

void pci_set_master(struct pci_dev *dev)
{
	if (!dev) return;

	u16 cmd;
	pci_read_config_word(dev, 0x04, &cmd);
	cmd |= 0x0004;  /* BUS_MASTER */
	pci_write_config_word(dev, 0x04, cmd);
}

/*===========================================================================*
 *		PCI iomap / iounmap                                       *
 *===========================================================================*/

void *pci_iomap(struct pci_dev *dev, int bar, unsigned long maxlen)
{
	if (!dev || bar < 0 || bar > 5)
		return NULL;

	resource_size_t start = pci_resource_start(dev, bar);
	resource_size_t len   = pci_resource_len(dev, bar);

	if (start == 0 || len == 0)
		return NULL;

	if (maxlen && len > maxlen)
		len = maxlen;

	void *virt = ioremap(start, len);

	/* Cache the mapping for quick access */
	if (bar == 0)
		dev->mmio_base = virt;
	else if (bar == 1)
		dev->mmio_base2 = virt;

	return virt;
}

void pci_iounmap(struct pci_dev *dev, void *addr)
{
	if (addr)
		iounmap(addr);
	if (dev) {
		if (dev->mmio_base == addr)
			dev->mmio_base = NULL;
		if (dev->mmio_base2 == addr)
			dev->mmio_base2 = NULL;
	}
}

/*===========================================================================*
 *		PCI region request / release                              *
 *===========================================================================*/

int pci_request_region(struct pci_dev *dev, int bar, const char *name)
{
	/* In MINIX userspace, regions are not tracked by the kernel.
	 * Reserve the device to mark it as in-use.  Also call pci_reserve. */
	if (!dev) return -EINVAL;

	extern int pci_reserve_ok(int devind);
	pci_reserve_ok(dev->devind);

	(void)name;
	return 0;
}

void pci_release_region(struct pci_dev *dev, int bar)
{
	/* No-op — MINIX doesn't have a release mechanism for individual BARs */
	(void)dev; (void)bar;
}

/*===========================================================================*
 *		PCI device enumeration (get_device / put_device)          *
 *===========================================================================*/

struct pci_dev *pci_get_device(unsigned int vendor, unsigned int device,
                               struct pci_dev *from)
{
	int devind;
	u16_t vid, did;

	/* Initialise or continue enumeration */
	if (from) {
		devind = from->devind;
		pci_dev_put(from);
		if (!pci_next_dev(&devind, &vid, &did))
			return NULL;
	} else {
		pci_init();
		devind = 0;
		if (!pci_first_dev(&devind, &vid, &did))
			return NULL;
	}

	/* Loop through devices until we find a match or exhaust the bus */
	while (1) {
		/* Check vendor/device match */
		if ((vendor == PCI_ANY_ID || vendor == vid) &&
		    (device == PCI_ANY_ID || device == did)) {
			/* Allocate and populate pci_dev */
			struct pci_dev *pdev = kzalloc(sizeof(*pdev), GFP_KERNEL);
			if (!pdev) return NULL;

	pdev->vendor  = vid;
	pdev->device  = did;
	pdev->devind  = devind;
	pdev->bus     = 0;  /* MINIX doesn't expose BDF easily */
	pdev->devfn   = 0;
	pdev->driver  = NULL;
	pdev->driver_data = NULL;
	pdev->mmio_base = NULL;
	pdev->mmio_base2 = NULL;

	/* Read subsystem IDs */
	pci_read_config_dword(pdev, 0x2C, &pdev->subsystem_vendor);
	pdev->subsystem_device = pdev->subsystem_vendor >> 16;
	pdev->subsystem_vendor &= 0xFFFF;

	/* Read class code and revision */
	u32 class_rev;
	pci_read_config_dword(pdev, 0x08, &class_rev);
	pdev->class    = class_rev >> 16;
	pdev->revision = class_rev & 0xFF;

	/* Read IRQ line */
	u8 irq_line;
	pci_read_config_byte(pdev, 0x3C, &irq_line);
	pdev->irq = irq_line;

	/* Read BARs */
	for (int i = 0; i < 6; i++) {
		u32_t base, size;
		int ioflag;
		int port = 0x10 + i * 4;
		if (pci_get_bar(devind, port, &base, &size, &ioflag) == OK) {
			pdev->resource[i].start = base;
			pdev->resource[i].end   = base + size - 1;
			pdev->resource[i].flags = ioflag ? IORESOURCE_IO
			                                : IORESOURCE_MEM;
		}
	}

			return pdev;
		}

		/* No match — try next device */
		if (!pci_next_dev(&devind, &vid, &did))
			return NULL;
	}
}

void pci_dev_put(struct pci_dev *dev)
{
	if (dev) {
		/* Free any cached ioremap mappings */
		if (dev->mmio_base)
			iounmap(dev->mmio_base);
		if (dev->mmio_base2)
			iounmap(dev->mmio_base2);
		kfree(dev);
	}
}

/*===========================================================================*
 *		PCI DMA mask                                              *
 *===========================================================================*/

int pci_set_dma_mask(struct pci_dev *dev, u64 mask)
{
	/* With direct DMA, the device can address any physical address.
	 * We accept most masks. */
	(void)dev;
	return (mask >= 0xFFFFULL) ? 0 : -EIO;
}

int pci_set_consistent_dma_mask(struct pci_dev *dev, u64 mask)
{
	return pci_set_dma_mask(dev, mask);
}

/*===========================================================================*
 *		PCI register/unregister driver                           *
 *===========================================================================*/

int __pci_register_driver(struct pci_driver *drv, const char *owner)
{
	if (!drv) return -EINVAL;

	printf("kernel_shim: registering PCI driver '%s' (owner=%s)\n",
	       drv->name ? drv->name : "(unnamed)", owner);

	/* If the driver has an id_table, iterate and call probe for each
	 * matching device.  This mimics Linux behaviour where probe is
	 * called for each device matching the driver's ID table. */
	if (drv->id_table && drv->probe) {
		int devind;
		u16_t vid, did;

		extern int pci_first_dev(int *, u16_t *, u16_t *);
		extern int pci_next_dev(int *, u16_t *, u16_t *);
		extern void pci_init(void);

		pci_init();
		devind = 0;

		if (pci_first_dev(&devind, &vid, &did)) {
			do {
				/* Check each entry in the ID table */
				const struct pci_device_id *id;
				for (id = drv->id_table;
				     id->vendor != PCI_ANY_ID ||
				     id->device != PCI_ANY_ID ||
				     id->subvendor != PCI_ANY_ID ||
				     id->subdevice != PCI_ANY_ID ||
				     id->class != 0;
				     id++) {
					/* Match vendor */
					if (id->vendor != PCI_ANY_ID &&
					    id->vendor != vid)
						continue;
					/* Match device */
					if (id->device != PCI_ANY_ID &&
					    id->device != did)
						continue;

					/* Create pci_dev structure */
					struct pci_dev *pdev =
					    kzalloc(sizeof(*pdev), GFP_KERNEL);
					if (!pdev) continue;

					pdev->vendor  = vid;
					pdev->device  = did;
					pdev->devind  = devind;
					pdev->driver  = drv;

					/* Read IRQ, class, resources */
					u32 class_rev, subsys;
					pci_read_config_dword(pdev, 0x08,
					                     &class_rev);
					pci_read_config_dword(pdev, 0x2C,
					                     &subsys);
					pdev->class    = class_rev >> 16;
					pdev->revision = class_rev & 0xFF;
					pdev->subsystem_vendor = subsys & 0xFFFF;
					pdev->subsystem_device  = subsys >> 16;

					u8 irq_line;
					pci_read_config_byte(pdev, 0x3C,
					                     &irq_line);
					pdev->irq = irq_line;

					/* Reserve the device */
					extern int pci_reserve_ok(int);
					pci_reserve_ok(devind);

					/* Call probe */
					int r = drv->probe(pdev, id);
					if (r != 0) {
						printf("kernel_shim: driver '%s' "
						       "probe failed for "
						       "%04x:%04x (%d)\n",
						       drv->name,
						       vid, did, r);
						kfree(pdev);
					}
					break;  /* matched — next device */
				}
			} while (pci_next_dev(&devind, &vid, &did));
		}
	}

	return 0;
}

void pci_unregister_driver(struct pci_driver *drv)
{
	printf("kernel_shim: unregistering PCI driver '%s'\n",
	       drv->name ? drv->name : "(unnamed)");

	if (drv->remove) {
		/* In a full implementation, iterate over devices and call
		 * remove() for each.  For now, no-op since we don't track
		 * the pci_dev list per driver. */
	}
}

/*===========================================================================*
 *		IRQ subsystem                                             *
 *===========================================================================*/

int request_irq(unsigned int irq, irq_handler_t handler,
                unsigned long flags, const char *name, void *dev)
{
	return request_threaded_irq(irq, handler, NULL, flags, name, dev);
}

int request_threaded_irq(unsigned int irq, irq_handler_t handler,
                         irq_handler_t thread_fn, unsigned long flags,
                         const char *name, void *dev)
{
	int i, r;

	if (!irq_initialised) {
		memset(irq_handlers, 0, sizeof(irq_handlers));
		irq_initialised = 1;
	}

	/* Find a free slot */
	for (i = 0; i < MAX_IRQ_HANDLERS; i++) {
		if (!irq_handlers[i].in_use) break;
	}
	if (i >= MAX_IRQ_HANDLERS)
		return -EBUSY;

	/* Store handler */
	irq_handlers[i].irq        = irq;
	irq_handlers[i].handler    = handler;
	irq_handlers[i].thread_fn  = thread_fn;
	irq_handlers[i].dev_id     = dev;
	irq_handlers[i].name       = name;
	irq_handlers[i].hook_id    = (int)irq;  /* initial value, modified by sys_irqsetpolicy */
	irq_handlers[i].in_use     = 1;

	/* Register with MINIX IRQ subsystem */
	int hook_id = (int)irq;
	r = sys_irqsetpolicy((int)irq, 0, &hook_id);
	if (r != OK) {
		printf("kernel_shim: request_irq(%u): sys_irqsetpolicy failed: %d\n",
		       irq, r);
		irq_handlers[i].in_use = 0;
		return -EIO;
	}
	irq_handlers[i].hook_id = hook_id;

	r = sys_irqenable(&hook_id);
	if (r != OK) {
		printf("kernel_shim: request_irq(%u): sys_irqenable failed: %d\n",
		       irq, r);
		sys_irqrmpolicy(&hook_id);
		irq_handlers[i].in_use = 0;
		return -EIO;
	}

	printf("kernel_shim: IRQ %u registered as '%s' (handler=%p)\n",
	       irq, name ? name : "(unnamed)", (void *)handler);

	return 0;
}

void free_irq(unsigned int irq, void *dev)
{
	for (int i = 0; i < MAX_IRQ_HANDLERS; i++) {
		if (!irq_handlers[i].in_use) continue;
		if (irq_handlers[i].irq == irq &&
		    irq_handlers[i].dev_id == dev) {
			int hook = irq_handlers[i].hook_id;
			sys_irqrmpolicy(&hook);
			irq_handlers[i].in_use = 0;
			printf("kernel_shim: IRQ %u freed\n", irq);
			return;
		}
	}
}

void disable_irq(unsigned int irq)
{
	/* MINIX doesn't have a per-IRQ disable.  We just mask it. */
	(void)irq;
}

void disable_irq_nosync(unsigned int irq)
{
	(void)irq;
}

void enable_irq(unsigned int irq)
{
	/* Re-enable via sys_irqenable.  Find the handler first. */
	for (int i = 0; i < MAX_IRQ_HANDLERS; i++) {
		if (irq_handlers[i].in_use &&
		    irq_handlers[i].irq == irq) {
			sys_irqenable(&irq_handlers[i].hook_id);
			return;
		}
	}
}

void synchronize_irq(unsigned int irq)
{
	(void)irq;  /* No-op in single-threaded userspace */
}

/*===========================================================================*
 *		IRQ dispatch (called from driver main loop)              *
 *===========================================================================*/

void klkm_irq_dispatch(unsigned int mask)
{
	for (int i = 0; i < MAX_IRQ_HANDLERS; i++) {
		if (!irq_handlers[i].in_use) continue;
		if (mask & (1u << irq_handlers[i].irq)) {
			if (irq_handlers[i].handler)
				irq_handlers[i].handler(
				    irq_handlers[i].irq,
				    irq_handlers[i].dev_id);
		}
	}
}

/*===========================================================================*
 *		Timer dispatch (called from driver main loop)            *
 *===========================================================================*/

/* Track active kernel-style timers for dispatch */
#define MAX_KLKM_TIMERS 64

struct klkm_timer {
	struct timer_list *timer;
	int active;
};

static struct klkm_timer klkm_timers[MAX_KLKM_TIMERS];
static int klkm_timers_count = 0;

void init_timer(struct timer_list *timer)
{
	if (timer) {
		memset(timer, 0, sizeof(*timer));
		timer->active = 0;
	}
}

void timer_setup(struct timer_list *timer,
                 void (*callback)(unsigned long), unsigned long data)
{
	if (timer) {
		timer->function = callback;
		timer->data     = data;
		timer->active   = 0;
	}
}

int mod_timer(struct timer_list *timer, unsigned long expires)
{
	if (!timer) return 0;

	timer->expires = expires;

	if (!timer->active) {
		/* Register with the dispatcher */
		if (klkm_timers_count < MAX_KLKM_TIMERS) {
			klkm_timers[klkm_timers_count].timer = timer;
			klkm_timers[klkm_timers_count].active = 1;
			klkm_timers_count++;
		}
		timer->active = 1;
	}
	return 1;
}

int del_timer(struct timer_list *timer)
{
	if (!timer || !timer->active) return 0;

	timer->active = 0;

	/* Remove from dispatcher list */
	for (int i = 0; i < klkm_timers_count; i++) {
		if (klkm_timers[i].timer == timer) {
			klkm_timers[i] = klkm_timers[--klkm_timers_count];
			break;
		}
	}
	return 1;
}

int del_timer_sync(struct timer_list *timer)
{
	return del_timer(timer);
}

int timer_pending(const struct timer_list *timer)
{
	return timer ? timer->active : 0;
}

void add_timer(struct timer_list *timer)
{
	mod_timer(timer, timer->expires);
}

void klkm_timer_dispatch(clock_t stamp)
{
	/* Increment jiffies (approximate: assume 1 tick per dispatch) */
	jiffies++;

	/* Fire expired timers */
	for (int i = 0; i < klkm_timers_count; i++) {
		struct timer_list *t = klkm_timers[i].timer;
		if (t && t->active &&
		    (unsigned long)stamp >= t->expires) {
			t->active = 0;
			if (t->function)
				t->function(t->data);
			/* Remove from list */
			klkm_timers[i] = klkm_timers[--klkm_timers_count];
			i--;
		}
	}
}

/*===========================================================================*
 *		Delays                                                    *
 *===========================================================================*/

void mdelay(unsigned long msecs)
{
	/* MINIX micro_delay takes microseconds */
	micro_delay(msecs * 1000);
}

void udelay(unsigned long usecs)
{
	micro_delay(usecs);
}

void msleep(unsigned int msecs)
{
	/* MINIX usleep takes microseconds */
	usleep(msecs * 1000);
}

void ssleep(unsigned int seconds)
{
	sleep(seconds);
}

/*===========================================================================*
 *		DMA API                                                   *
 *===========================================================================*/

void *dma_alloc_coherent(struct device *dev, size_t size,
                         dma_addr_t *dma_handle, int gfp)
{
	phys_bytes phys;
	void *virt;

	(void)dev; (void)gfp;

	virt = alloc_contig(size, AC_ALIGN4K, &phys);
	if (!virt)
		return NULL;

	memset(virt, 0, size);

	if (dma_handle)
		*dma_handle = (dma_addr_t)phys;

	return virt;
}

void dma_free_coherent(struct device *dev, size_t size,
                       void *cpu_addr, dma_addr_t dma_handle)
{
	/* MINIX alloc_contig doesn't have a matching free. */
	(void)dev; (void)size; (void)cpu_addr; (void)dma_handle;
}

dma_addr_t dma_map_single(struct device *dev, void *cpu_addr,
                          size_t size, enum dma_data_direction dir)
{
	phys_bytes phys;

	(void)dev; (void)dir;

	if (sys_umap_remote(SELF, SELF, VM_D, (vir_bytes)cpu_addr,
	    (vir_bytes)size, &phys) != OK)
		return 0;

	vm_adddma(SELF, phys, (vir_bytes)size);

	return (dma_addr_t)phys;
}

void dma_unmap_single(struct device *dev, dma_addr_t dma_addr,
                      size_t size, enum dma_data_direction dir)
{
	(void)dev; (void)dir;
	vm_deldma(SELF, (phys_bytes)dma_addr, (vir_bytes)size);
}

int dma_set_mask(struct device *dev, u64 mask)
{
	(void)dev;
	return (mask >= 0xFFFFULL) ? 0 : -EIO;
}

int dma_set_coherent_mask(struct device *dev, u64 mask)
{
	return dma_set_mask(dev, mask);
}

void dma_sync_single_for_cpu(struct device *dev, dma_addr_t dma_addr,
                             size_t size, enum dma_data_direction dir)
{
	(void)dev; (void)dma_addr; (void)size; (void)dir;
}

void dma_sync_single_for_device(struct device *dev, dma_addr_t dma_addr,
                                size_t size, enum dma_data_direction dir)
{
	(void)dev; (void)dma_addr; (void)size; (void)dir;
}

/*===========================================================================*
 *		Printk / logging                                          *
 *===========================================================================*/

int printk(const char *fmt, ...)
{
	va_list args;
	int r;

	/* Strip KERN_* level prefix if present */
	if (fmt && fmt[0] == KERN_SOH_ASCII && fmt[1] >= '0' && fmt[1] <= '7')
		fmt += 2;

	va_start(args, fmt);
	r = vprintf(fmt, args);
	va_end(args);
	return r;
}

int dev_printk(const char *level, const struct device *dev, const char *fmt, ...)
{
	va_list args;
	int r;

	(void)level;

	if (dev)
		printf("%s: ", dev->driver_data ? (char *)dev->driver_data : "dev");

	va_start(args, fmt);
	r = vprintf(fmt, args);
	va_end(args);
	return r;
}

/*===========================================================================*
 *		Workqueues                                                *
 *===========================================================================*/

/* Workqueue items are executed synchronously when schedule_work is called.
 * In a single-threaded driver, there's no separate workqueue thread. */

static int work_execute(struct work_struct *work)
{
	if (!work || !work->func) return 0;
	work->pending = 0;
	work->func(work->data);
	return 1;
}

int schedule_work(struct work_struct *work)
{
	if (!work) return 0;
	/* Execute immediately */
	work->pending = 0;
	work_execute(work);
	return 1;
}

int schedule_delayed_work(struct delayed_work *dwork, unsigned long delay)
{
	if (!dwork) return 0;
	/* Ignore delay — execute synchronously */
	work_execute(&dwork->work);
	return 1;
}

void flush_work(struct work_struct *work)
{
	/* In synchronous execution, work is already done. */
	(void)work;
}

int cancel_work_sync(struct work_struct *work)
{
	if (!work) return 0;
	int was_pending = work->pending;
	work->pending = 0;
	return was_pending;
}

int cancel_delayed_work(struct delayed_work *dwork)
{
	if (!dwork) return 0;
	return cancel_work_sync(&dwork->work);
}

int cancel_delayed_work_sync(struct delayed_work *dwork)
{
	return cancel_delayed_work(dwork);
}

void flush_scheduled_work(void)
{
	/* No-op — all work is synchronous */
}

/*===========================================================================*
 *		Firmware loading                                         *
 *===========================================================================*/

int request_firmware(const struct firmware **fw, const char *name,
                     struct device *device)
{
	char path[256];
	int fd;
	struct stat st;
	u8 *data;
	struct firmware *f;

	(void)device;

	if (!fw || !name) return -EINVAL;

	/* Search /lib/firmware/ */
	snprintf(path, sizeof(path), "/lib/firmware/%s", name);

	fd = open(path, O_RDONLY);
	if (fd < 0) {
		/* Try /etc/firmware/ as fallback */
		snprintf(path, sizeof(path), "/etc/firmware/%s", name);
		fd = open(path, O_RDONLY);
	}
	if (fd < 0) {
		printf("kernel_shim: firmware '%s' not found\n", name);
		return -ENOENT;
	}

	if (fstat(fd, &st) < 0) {
		close(fd);
		return -EIO;
	}

	data = malloc(st.st_size);
	if (!data) {
		close(fd);
		return -ENOMEM;
	}

	if (read(fd, data, st.st_size) != st.st_size) {
		close(fd);
		free(data);
		return -EIO;
	}
	close(fd);

	f = malloc(sizeof(*f));
	if (!f) {
		free(data);
		return -ENOMEM;
	}

	f->size = st.st_size;
	f->data = data;
	*fw = f;

	printf("kernel_shim: firmware '%s' loaded (%zu bytes)\n",
	       name, st.st_size);
	return 0;
}

int request_firmware_nowait(const char *name, struct device *device,
                            firmware_cb_t callback, void *context)
{
	const struct firmware *fw = NULL;

	if (!name || !callback) return -EINVAL;

	printf("kernel_shim: firmware_nowait '%s' (synchronous fallback)\n",
	    name);

	/* Synchronous fallback: load firmware immediately and call callback.
	 * In a multi-threaded future version, this would queue a work item. */
	int ret = request_firmware(&fw, name, device);

	if (ret == 0 && fw) {
		callback(fw, context);
		release_firmware(fw);
	}

	return ret;
}

void release_firmware(const struct firmware *fw)
{
	if (fw) {
		free((void *)fw->data);
		free((void *)fw);
	}
}

/*===========================================================================*
 *		Wait queues / completion (synchronous)                   *
 *===========================================================================*/

void wait_for_completion(struct completion *c)
{
	/* In single-threaded mode, completions are always already done,
	 * because we don't have asynchronous completion sources. */
	if (c)
		c->done = 1;
}

unsigned long wait_for_completion_timeout(struct completion *c,
                                          unsigned long timeout)
{
	if (c)
		c->done = 1;
	return timeout;  /* return remaining jiffies */
}

void complete(struct completion *c)
{
	if (c)
		c->done = 1;
}

void complete_all(struct completion *c)
{
	if (c)
		c->done = 1;
}

/*===========================================================================*
 *		Misc                                                      *
 *===========================================================================*/

void get_random_bytes(void *buf, int len)
{
	/* Use rand() seeded from system clock if not yet done.
	 * Real impl would use random() or sys_rand32(). */
	static int seeded = 0;
	if (!seeded) {
		srand((unsigned)time(NULL) ^ (unsigned)getticks());
		seeded = 1;
	}
	for (int i = 0; i < len; i++)
		((unsigned char *)buf)[i] = (unsigned char)(rand() & 0xFF);
}

void dump_stack(void)
{
	/* MINIX doesn't have a programmatic stack tracer.
	 * Print an informational message. */
	printf("kernel_shim: dump_stack() called\n");
}

/*===========================================================================*
 *		sk_buff API (simplified)                                 *
 *===========================================================================*/

struct sk_buff *dev_alloc_skb(unsigned int length)
{
	struct sk_buff *skb = kmalloc(sizeof(*skb), GFP_KERNEL);
	if (!skb) return NULL;

	/* Allocate data buffer with some headroom */
	unsigned int alloc_len = length + 64;  /* extra for headroom */
	skb->data = kmalloc(alloc_len, GFP_KERNEL);
	if (!skb->data) {
		kfree(skb);
		return NULL;
	}

	skb->head    = skb->data;
	skb->tail    = skb->data;
	skb->end     = skb->data + alloc_len;
	skb->len     = 0;
	skb->truesize = sizeof(*skb) + alloc_len;
	skb->users   = 1;

	return skb;
}

void dev_kfree_skb(struct sk_buff *skb)
{
	kfree_skb(skb);
}

void kfree_skb(struct sk_buff *skb)
{
	if (!skb) return;
	if (skb->users > 1) {
		skb->users--;
		return;
	}
	kfree(skb->head);
	kfree(skb);
}

unsigned char *skb_put(struct sk_buff *skb, unsigned int len)
{
	unsigned char *tmp = skb->tail;
	skb->tail += len;
	skb->len  += len;
	return tmp;
}

unsigned char *skb_push(struct sk_buff *skb, unsigned int len)
{
	skb->data -= len;
	skb->len  += len;
	return skb->data;
}

unsigned char *skb_pull(struct sk_buff *skb, unsigned int len)
{
	skb->data += len;
	skb->len  -= len;
	return skb->data;
}

void skb_reserve(struct sk_buff *skb, unsigned int len)
{
	skb->data += len;
	skb->tail += len;
}

void skb_trim(struct sk_buff *skb, unsigned int len)
{
	if (skb->len > len) {
		skb->len = len;
		skb->tail = skb->data + len;
	}
}

struct sk_buff *skb_clone(struct sk_buff *skb, int gfp_mask)
{
	struct sk_buff *clone = kmalloc(sizeof(*clone), gfp_mask);
	if (!clone) return NULL;

	memcpy(clone, skb, sizeof(*clone));
	clone->users = 1;  /* we own this copy */
	skb->users++;      /* original still exists */
	return clone;
}

struct sk_buff *skb_copy(struct sk_buff *skb, int gfp_mask)
{
	struct sk_buff *new_skb = dev_alloc_skb(skb->len);
	if (!new_skb) return NULL;

	memcpy(new_skb->data, skb->data, skb->len);
	new_skb->tail = new_skb->data + skb->len;
	new_skb->len  = skb->len;
	return new_skb;
}

/*===========================================================================*
 *		Initialisation and cleanup                               *
 *===========================================================================*/

void klkm_init(void)
{
	memset(irq_handlers, 0, sizeof(irq_handlers));
	irq_initialised = 1;
	memset(klkm_timers, 0, sizeof(klkm_timers));
	klkm_timers_count = 0;
	jiffies = 0;

	printf("kernel_shim: initialised (MINIX userspace mode)\n");
}

void klkm_exit(void)
{
	/* Free all IRQ handlers */
	for (int i = 0; i < MAX_IRQ_HANDLERS; i++) {
		if (irq_handlers[i].in_use) {
			int hook = irq_handlers[i].hook_id;
			sys_irqrmpolicy(&hook);
			irq_handlers[i].in_use = 0;
		}
	}

	klkm_timers_count = 0;
	printf("kernel_shim: shut down\n");
}
