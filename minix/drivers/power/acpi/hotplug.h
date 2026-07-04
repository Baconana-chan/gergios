#ifndef __ACPI_HOTPLUG_H__
#define __ACPI_HOTPLUG_H__

#include <acpi.h>

/*
 * Maximum number of PCI root bridges (usually 1 on single-root systems).
 */
#define ACPI_HOTPLUG_MAX_ROOT_BRIDGES	8

/*
 * Maximum number of hot-plug event listeners.
 */
#define ACPI_HOTPLUG_MAX_LISTENERS	8

/*
 * Hot-plug event types (subset of ACPI_NOTIFY_* values).
 */
#define ACPI_HOTPLUG_EVENT_DEVICE_ARRIVED	0x01  /* DEVICE_CHECK */
#define ACPI_HOTPLUG_EVENT_DEVICE_REMOVED	0x03  /* EJECT_REQUEST */
#define ACPI_HOTPLUG_EVENT_BUS_RESCAN		0x00  /* BUS_CHECK */

/*
 * Callback for hot-plug event notification.
 * Called from main loop context when an ACPI Notify event arrives.
 */
typedef void (*acpi_hotplug_callback_t)(ACPI_HANDLE Device,
    UINT32 Event, void *Context);

/*
 * Initialize ACPI hot-plug subsystem:
 * 1. Find all PCI root bridge devices (PNP0A03/PNP0A08)
 * 2. Install ACPI_NOTIFY_SYSTEM handlers on each root bridge
 * 3. Register a default handler that logs events
 */
void acpi_hotplug_init(void);

/*
 * Register a callback for hot-plug events.
 * Returns 0 on success, -1 if listener table is full.
 */
int acpi_hotplug_register(acpi_hotplug_callback_t callback, void *context);

/*
 * Unregister a previously registered callback.
 */
void acpi_hotplug_unregister(acpi_hotplug_callback_t callback);

#endif /* __ACPI_HOTPLUG_H__ */
