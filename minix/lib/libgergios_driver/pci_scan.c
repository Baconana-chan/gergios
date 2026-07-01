/* pci_scan.c — GergiOS PCI Device Discovery & Hot-Plug Implementation
 *
 * Replaces the duplicated pci_init() + pci_first_dev() + pci_get_bar()
 * pattern found in ~30 PCI drivers with a single function call.
 *
 * How it works:
 *   1. gergios_pci_probe() calls pci_first_dev() / pci_next_dev() to
 *      enumerate all PCI devices on the bus.
 *   2. For each device, it reads config space (vendor, device, subvendor,
 *      subdevice, class code, BARs) via pci_attr_r16/r32().
 *   3. A gergios_device is created with all resources.
 *   4. If a driver with an id_table is provided, gergios_device_match()
 *      is called for each device.  On match, drv->ops.probe() is called.
 *
 * This eliminates ~200 LOC of duplicated boilerplate per driver.
 */

#include <minix/drivers.h>
#include <minix/sysutil.h>
#include <minix/syslib.h>
#include <minix/com.h>
#include <minix/endpoint.h>
#include <minix/ds.h>

#include "gergios_driver.h"
#include "gergios_device.h"
#include "pci_scan.h"

/* pci_attr_r8/r16/r32 are defined in libsys but not declared in public headers.
 * Declare them here to avoid implicit declaration warnings. */
extern u8_t  pci_attr_r8(int devind, int port);
extern u16_t pci_attr_r16(int devind, int port);
extern u32_t pci_attr_r32(int devind, int port);

/* PCI config space register offsets (standard) */
#define PCI_CONFIG_VENDOR_ID     0x00
#define PCI_CONFIG_DEVICE_ID     0x02
#define PCI_CONFIG_COMMAND       0x04
#define PCI_CONFIG_STATUS        0x06
#define PCI_CONFIG_REVISION      0x08
#define PCI_CONFIG_CLASS_CODE    0x08
#define PCI_CONFIG_HEADER_TYPE   0x0E
#define PCI_CONFIG_BAR_0         0x10
#define PCI_CONFIG_SUBSYSTEM_VID 0x2C
#define PCI_CONFIG_SUBSYSTEM_ID  0x2E
#define PCI_CONFIG_CAPABILITIES  0x34
#define PCI_CONFIG_INTERRUPT_LINE 0x3C
#define PCI_CONFIG_INTERRUPT_PIN 0x3D
#define PCI_BAR_COUNT 6

uint16_t
gergios_pci_read16(int devind, int offset)
{
	return pci_attr_r16(devind, offset);
}

uint32_t
gergios_pci_read32(int devind, int offset)
{
	return pci_attr_r32(devind, offset);
}

uint32_t
gergios_pci_get_class(int devind)
{
	return pci_attr_r32(devind, PCI_CONFIG_CLASS_CODE) >> 8;
}

int
gergios_pci_reserve(int devind)
{
	return pci_reserve_ok(devind);
}

static int
read_bars(int devind, struct gergios_device *dev)
{
	int i, r;
	u32_t base, size;
	int ioflag;
	struct gergios_resource res;

	for (i = 0; i < PCI_BAR_COUNT; i++) {
		int port = PCI_CONFIG_BAR_0 + i * 4;

		memset(&res, 0, sizeof(res));
		r = pci_get_bar(devind, port, &base, &size, &ioflag);
		if (r != OK) continue;
		if (base == 0 || size == 0) continue;

		if (ioflag & PCI_BAR_IO) {
			res.type = 1;
			res.u.port.base = (uint16_t)(base & 0xFFFF);
			res.u.port.size = (uint16_t)size;
		} else {
			res.type = 0;
			res.u.mmio.base = base;
			res.u.mmio.size = size;
		}
		r = gergios_device_add_resource(dev, &res);
		if (r != OK) return r;
	}

	/* Read IRQ line */
	{
		uint8_t irq_line = (uint8_t)pci_attr_r8(devind,
		    PCI_CONFIG_INTERRUPT_LINE);
		uint8_t irq_pin = (uint8_t)pci_attr_r8(devind,
		    PCI_CONFIG_INTERRUPT_PIN);
		if (irq_pin != 0 && irq_line != 0 && irq_line != 0xFF) {
			struct gergios_resource irq_res;
			memset(&irq_res, 0, sizeof(irq_res));
			irq_res.type = 2;
			irq_res.u.irq.line = irq_line;
			irq_res.u.irq.vector = irq_line;
			gergios_device_add_resource(dev, &irq_res);
		}
	}
	return OK;
}

int
gergios_pci_probe(struct gergios_driver *drv)
{
	int devind;
	u16_t vid, did;
	int count = 0;

	pci_init();

	devind = 0;
	if (!pci_first_dev(&devind, &vid, &did))
		return 0;

	while (1) {
		uint16_t subvid, subdid;
		uint32_t class;
		struct gergios_device *dev;

		subvid = pci_attr_r16(devind, PCI_CONFIG_SUBSYSTEM_VID);
		subdid = pci_attr_r16(devind, PCI_CONFIG_SUBSYSTEM_ID);
		class = gergios_pci_get_class(devind);

		/* Create gergios_device.  bus_address stores the PCI
		 * server's internal device index (devind), NOT a BDF.
		 * A full implementation should derive the true BDF from
		 * PCI config space or a PCI server API extension. */
		dev = gergios_device_create(NULL,
		    vid, did, subvid, subdid, class,
		    (uint32_t)devind);
		if (dev == NULL) {
			count++;
			goto next;
		}

		read_bars(devind, dev);

		if (drv != NULL && drv->id_table != NULL) {
			const struct gergios_device_id *id;

			id = gergios_device_match(drv->id_table,
			    vid, did, subvid, subdid, class);
			if (id != NULL) {
				dev->driver = drv;
				dev->private = (void *)id->driver_data;

				pci_reserve(devind);

				if (drv->ops.probe) {
					int r = drv->ops.probe(dev);
					if (r == OK) {
						gergios_device_set_state(dev,
						    GERGIOS_DEV_ATTACHED);
					}
				}
			}
		}
		count++;

next:
		if (!pci_next_dev(&devind, &vid, &did))
			break;
	}
	return count;
}

/* Hot-plug (stub — deferred to Phase 2b) */
static gergios_pci_hotplug_cb_t hotplug_cb = NULL;

int
gergios_pci_hotplug_register(gergios_pci_hotplug_cb_t cb)
{
	if (cb == NULL) return EINVAL;
	if (hotplug_cb != NULL) return EBUSY;
	hotplug_cb = cb;
	return ENOSYS;
}

void
gergios_pci_hotplug_unregister(gergios_pci_hotplug_cb_t cb)
{
	if (hotplug_cb == cb) hotplug_cb = NULL;
}
