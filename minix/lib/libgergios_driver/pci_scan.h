/* pci_scan.h — GergiOS PCI Device Discovery & Hot-Plug API
 *
 * Provides automatic PCI bus scanning that replaces the duplicated
 * pci_init() + pci_first_dev() + pci_get_bar() pattern currently
 * repeated in ~30 drivers.
 *
 * Usage:
 *   #include "pci_scan.h"
 *
 *   int dev_count = gergios_pci_probe(NULL);
 *   printf("Found %d PCI devices\n", dev_count);
 *
 *   // With driver matching:
 *   struct gergios_driver *ahci = ...;
 *   int matched = gergios_pci_probe(ahci);
 *   // matched = number of devices that matched ahci's id_table
 *
 * Hot-plug (Phase 2b, deferred):
 *   gergios_pci_hotplug_register(handler);
 *   // handler is called when new PCI devices appear / disappear
 */

#ifndef _GERGIOS_PCI_SCAN_H
#define _GERGIOS_PCI_SCAN_H

#include <minix/config.h>
#include <minix/type.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>	/* pci_* API */

#include "gergios_driver.h"
#include "gergios_device.h"

/*===========================================================================*
 *		PCI scanning						     *
 *===========================================================================*/

/* Scan the PCI bus and create gergios_device entries for all found devices.
 *
 * If drv is non-NULL, only devices matching drv->id_table are created
 * and registered with the driver (drv->ops.probe is called for each).
 * If drv is NULL, all devices are created as UNBOUND devices.
 *
 * Returns the number of devices found (>= 0), or a negative errno on error.
 */
int gergios_pci_probe(struct gergios_driver *drv);

/* Read PCI configuration space registers for a device.
 * These wrap pci_attr_r16/r32 with proper knowledge of config space layout.
 */
uint16_t gergios_pci_read16(int devind, int offset);
uint32_t gergios_pci_read32(int devind, int offset);

/* Get the 24-bit class code (base class << 16 | subclass << 8 | interface). */
uint32_t gergios_pci_get_class(int devind);

/* Reserve a PCI device for exclusive use. */
int gergios_pci_reserve(int devind);

/*===========================================================================*
 *		PCI hot-plug notification (Phase 2b)			     *
 *===========================================================================*/

/* Types of hot-plug events */
typedef enum {
	GERGIOS_PCI_EVENT_DEVICE_ADDED,		/* New device appeared */
	GERGIOS_PCI_EVENT_DEVICE_REMOVED,	/* Device removed */
	GERGIOS_PCI_EVENT_BUS_RESCAN,		/* Full bus rescan needed */
} gergios_pci_event_t;

/* Hot-plug event */
struct gergios_pci_event {
	gergios_pci_event_t type;
	int devind;			/* PCI device index */
	uint16_t vendor_id;		/* PCI vendor ID */
	uint16_t device_id;		/* PCI device ID */
	uint8_t bus;			/* Bus number */
	uint8_t dev;			/* Device number */
	uint8_t func;			/* Function number */
};

/* Hot-plug callback */
typedef void (*gergios_pci_hotplug_cb_t)(const struct gergios_pci_event *ev);

/* Register a callback for PCI hot-plug events.
 * Returns 0 on success, negative errno on error.
 *
 * NOTE: ACPI hot-plug support is not yet implemented. This function
 * currently returns ENOSYS.  It will be implemented in Phase 2b when
 * the ACPI driver integration for PCIe Native Hot-Plug is ready.
 */
int gergios_pci_hotplug_register(gergios_pci_hotplug_cb_t cb);

/* Unregister a hot-plug callback. */
void gergios_pci_hotplug_unregister(gergios_pci_hotplug_cb_t cb);

#endif /* _GERGIOS_PCI_SCAN_H */
