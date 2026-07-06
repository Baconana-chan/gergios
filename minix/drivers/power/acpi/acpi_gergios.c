/* acpi_gergios.c — ACPI → GergiOS Driver Model Integration
 *
 * Bridges the ACPI enumeration & hot-plug subsystem with the
 * gergios unified driver model (libgergios_driver).
 *
 * After ACPI ACPICA init and namespace walk, this module:
 *   1. Registers built-in driver PCI ID mappings for autoload
 *   2. Triggers a gergios PCI bus rescan — which creates
 *      gergios_device instances and autoloads matching drivers via RS_UP
 *   3. Registers as an ACPI hotplug listener so future ACPI Notify
 *      events (BUS_CHECK, DEVICE_CHECK) trigger the same rescan
 */

#include <stdio.h>
#include <string.h>
#include <minix/drivers.h>
#include <minix/sysutil.h>

#include <acpi.h>

/* ACPI driver's own headers (same directory, use quotes) */
#include "enumerate.h"
#include "hotplug.h"

/* GergiOS driver model headers (from libgergios_driver via -I path).
 * Use angle brackets to avoid filename collision with ACPI's hotplug.h. */
#include <gergios_device.h>
#include <gergios_driver.h>
#include <hotplug.h>		/* libgergios_driver's hotplug.h (NOT ACPI's) */
#include <pci_scan.h>

/*===========================================================================*
 *      Hotplug listener callback                                         *
 *===========================================================================*/

/*
 * Called from ACPI's notify handler when a PCI hot-plug event occurs.
 * Triggers a full PCI bus rescan through gergios — which creates
 * gergios_device instances for any newly appeared devices and
 * autoloads their drivers via RS_UP.
 */
static void
acpi_gergios_hotplug_cb(ACPI_HANDLE Device, UINT32 Event, void *Context)
{
	(void)Device;
	(void)Context;

	printf("ACPI-GERGIOS: hotplug event 0x%x -> PCI rescan\n", Event);

	/* Trigger gergios PCI bus rescan — creates gergios_device
	 * instances for new devices and autoloads matching drivers.
	 * gergios_pci_rescan_bus() handles dedup via known_devinds. */
	gergios_pci_rescan_bus();
}

/*===========================================================================*
 *      Initialisation                                                       *
 *===========================================================================*/

int
acpi_gergios_init(void)
{
	int r;

	printf("ACPI-GERGIOS: initialising gergios device integration\n");

	/*------------------------------------------------------------------------*
	 *  1. Register built-in driver PCI ID maps                               *
	 *     These are used by gergios_hotplug_autoload_driver() to match        *
	 *     a discovered PCI device to its driver binary.                       *
	 *------------------------------------------------------------------------*/
	{
		static const struct gergios_device_id ahci_ids[] = {
			{ 0x8086, 0x2922, 0xFFFF, 0xFFFF, 0x010601, 0 },
			{ 0x8086, 0x1E02, 0xFFFF, 0xFFFF, 0x010601, 0 },
			{ 0x1002, 0x4391, 0xFFFF, 0xFFFF, 0x010601, 0 },
			GERGIOS_DEVICE_ID_END
		};
		gergios_hotplug_register_driver_map("ahci", ahci_ids);

		static const struct gergios_device_id e1000_ids[] = {
			{ 0x8086, 0x100E, 0xFFFF, 0xFFFF, 0x020000, 0 },
			{ 0x8086, 0x100F, 0xFFFF, 0xFFFF, 0x020000, 0 },
			{ 0x8086, 0x10D3, 0xFFFF, 0xFFFF, 0x020000, 0 },
			GERGIOS_DEVICE_ID_END
		};
		gergios_hotplug_register_driver_map("e1000", e1000_ids);

		static const struct gergios_device_id virtio_blk_ids[] = {
			{ 0x1AF4, 0x1001, 0xFFFF, 0xFFFF, 0x010000, 0 },
			GERGIOS_DEVICE_ID_END
		};
		gergios_hotplug_register_driver_map("virtio_blk",
		    virtio_blk_ids);

		static const struct gergios_device_id virtio_net_ids[] = {
			{ 0x1AF4, 0x1000, 0xFFFF, 0xFFFF, 0x020000, 0 },
			GERGIOS_DEVICE_ID_END
		};
		gergios_hotplug_register_driver_map("virtio_net",
		    virtio_net_ids);
	}

	/*------------------------------------------------------------------------*
	 *  2. Scan existing PCI devices and create gergios_device instances.     *
	 *     gergios_pci_rescan_bus() iterates all PCI devices via              *
	 *     pci_first_dev/pci_next_dev, creates gergios_device objects,        *
	 *     and autoloads matching drivers via RS_UP.                          *
	 *------------------------------------------------------------------------*/
	r = gergios_pci_rescan_bus();
	printf("ACPI-GERGIOS: gergios_pci_rescan_bus returned %d new devices\n",
	    r);

	/*------------------------------------------------------------------------*
	 *  3. Register as ACPI hotplug listener so future                       *
	 *     Notify events (BUS_CHECK, DEVICE_CHECK, EJECT_REQUEST)            *
	 *     trigger gergios device creation + autoload.                       *
	 *------------------------------------------------------------------------*/
	r = acpi_hotplug_register(acpi_gergios_hotplug_cb, NULL);
	if (r != 0) {
		printf("ACPI-GERGIOS: acpi_hotplug_register failed: %d\n", r);
	} else {
		printf("ACPI-GERGIOS: hotplug listener registered\n");
	}

	return 0;
}
