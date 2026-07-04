#include <stdio.h>
#include <string.h>
#include <acpi.h>
#include <minix/driver.h>

#include "hotplug.h"
#include "pci.h"	/* for pci_scan_devices() */

/*
 * Registered hot-plug event listeners.
 */
static struct {
	acpi_hotplug_callback_t	callback;
	void			*context;
} acpi_hotplug_listeners[ACPI_HOTPLUG_MAX_LISTENERS];
static int acpi_hotplug_listener_count = 0;

/*
 * ACPI Notify handler — called by ACPICA when a device sends a
 * system-level notification (ACPI_SYSTEM_NOTIFY).
 *
 * This handler is installed on PCI root bridge devices (PNP0A03/PNP0A08)
 * and receives notifications for:
 *   BUS_CHECK (0x00)  — PCI bus topology may have changed
 *   DEVICE_CHECK (0x01) — device arrived on this bus
 *   DEVICE_WAKE (0x02) — device woke the system
 *   EJECT_REQUEST (0x03) — device is being ejected
 */
static void acpi_hotplug_notify_handler(
    ACPI_HANDLE Device,
    UINT32 Value,
    void *Context)
{
	int i;
	(void)Context;

	printf("ACPI: hot-plug event on %p: notify=0x%x\n",
	    (void *)Device, Value);

	switch (Value) {
	case ACPI_NOTIFY_BUS_CHECK:
		printf("ACPI: BUS_CHECK — PCI bus rescan needed\n");
		pci_scan_devices();
		break;

	case ACPI_NOTIFY_DEVICE_CHECK:
		printf("ACPI: DEVICE_CHECK — device arrived/removed\n");
		pci_scan_devices();
		break;

	case ACPI_NOTIFY_EJECT_REQUEST:
		printf("ACPI: EJECT_REQUEST — device being removed\n");
		pci_scan_devices();
		break;

	case ACPI_NOTIFY_DEVICE_WAKE:
		printf("ACPI: DEVICE_WAKE — device woke the system\n");
		break;

	default:
		printf("ACPI: unknown notify event 0x%x\n", Value);
		break;
	}

	/* Notify all registered listeners */
	for (i = 0; i < acpi_hotplug_listener_count; i++) {
		if (acpi_hotplug_listeners[i].callback) {
			acpi_hotplug_listeners[i].callback(
			    Device, Value,
			    acpi_hotplug_listeners[i].context);
		}
	}
}

/*
 * ACPI namespace walk callback — find PCI root bridge devices
 * and install Notify handlers.
 */
static ACPI_STATUS acpi_hotplug_walk_cb(
    ACPI_HANDLE handle, UINT32 level,
    void *context, void **retval)
{
	ACPI_STATUS status;
	int *count = (int *)context;
	(void)level;
	(void)retval;

	status = AcpiInstallNotifyHandler(handle, ACPI_SYSTEM_NOTIFY,
	    acpi_hotplug_notify_handler, NULL);
	if (ACPI_SUCCESS(status)) {
		printf("ACPI: hot-plug handler installed on %p\n",
		    (void *)handle);
		if (count)
			(*count)++;
	} else {
		printf("ACPI: failed to install hot-plug handler "
		    "on %p: %d\n", (void *)handle, status);
	}

	return AE_OK;
}

/*
 * Initialize ACPI hot-plug subsystem.
 *
 * Walk the ACPI namespace for PCI Express root bridges (PNP0A08)
 * and legacy PCI root bridges (PNP0A03), and install ACPI_SYSTEM_NOTIFY
 * handlers on each.
 */
void acpi_hotplug_init(void)
{
	ACPI_STATUS status;
	int installed = 0;

	/*
	 * Find PCI Express root bridges (PNP0A08) first.
	 */
	status = AcpiGetDevices(
	    (char *)PCI_EXPRESS_ROOT_HID_STRING,
	    acpi_hotplug_walk_cb, &installed, NULL);
	if (ACPI_FAILURE(status)) {
		printf("ACPI: hot-plug walk for PNP0A08 failed: %d\n",
		    status);
	}

	/*
	 * Also scan for legacy PCI root bridges (PNP0A03).
	 */
	status = AcpiGetDevices(
	    (char *)PCI_ROOT_HID_STRING,
	    acpi_hotplug_walk_cb, &installed, NULL);
	if (ACPI_FAILURE(status)) {
		printf("ACPI: hot-plug walk for PNP0A03 failed: %d\n",
		    status);
	}

	/* Clear listener table */
	acpi_hotplug_listener_count = 0;
	memset(acpi_hotplug_listeners, 0, sizeof(acpi_hotplug_listeners));

	printf("ACPI: hot-plug subsystem initialized (%d bridges)\n",
	    installed);
}

int acpi_hotplug_register(acpi_hotplug_callback_t callback, void *context)
{
	int i;

	if (!callback)
		return -1;

	for (i = 0; i < ACPI_HOTPLUG_MAX_LISTENERS; i++) {
		if (acpi_hotplug_listeners[i].callback == NULL) {
			acpi_hotplug_listeners[i].callback = callback;
			acpi_hotplug_listeners[i].context = context;
			if (i >= acpi_hotplug_listener_count)
				acpi_hotplug_listener_count = i + 1;
			return 0;
		}
	}

	printf("ACPI: hot-plug listener table full\n");
	return -1;
}

void acpi_hotplug_unregister(acpi_hotplug_callback_t callback)
{
	int i;

	for (i = 0; i < ACPI_HOTPLUG_MAX_LISTENERS; i++) {
		if (acpi_hotplug_listeners[i].callback == callback) {
			acpi_hotplug_listeners[i].callback = NULL;
			acpi_hotplug_listeners[i].context = NULL;
			while (acpi_hotplug_listener_count > 0 &&
			    acpi_hotplug_listeners[
			    acpi_hotplug_listener_count - 1].callback == NULL)
				acpi_hotplug_listener_count--;
			return;
		}
	}
}
