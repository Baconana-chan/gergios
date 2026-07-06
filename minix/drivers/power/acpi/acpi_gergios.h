/* acpi_gergios.h — ACPI → GergiOS Driver Model Integration
 *
 * Bridges the ACPI enumeration & hot-plug subsystem with the
 * gergios unified driver model (libgergios_driver).
 *
 * When called from acpi.c after ACPI init, this module:
 *   1. Registers built-in driver PCI ID mappings
 *   2. Iterates discovered ACPI devices and creates gergios_device
 *      instances for each PCI device, then triggers driver autoloading
 *   3. Registers a hot-plug listener so future ACPI Notify events
 *      also trigger gergios device creation + autoloading
 *
 * This completes the deferred Phase 7.1 integration:
 *   "ACPI enumeration → gergios_device creation, gergios_hotplug_event() call"
 */

#ifndef _ACPI_GERGIOS_H_
#define _ACPI_GERGIOS_H_

/*
 * Initialise the ACPI → GergiOS bridge.
 *
 * Must be called AFTER acpi_enumerate_devices() and
 * acpi_hotplug_init() have completed.  This function:
 *   1. Registers driver ID maps (ahci, e1000, virtio_blk, virtio_net)
 *   2. Iterates the ACPI device table and for each PCI device
 *      (identified by ACPI_VALID_ADR), creates a gergios_device
 *      instance and attempts driver autoload
 *   3. Registers a callback with acpi_hotplug_register() so that
 *      ACPI Notify events (BUS_CHECK, DEVICE_CHECK) also trigger
 *      gergios device creation for newly appeared PCI devices
 *
 * Returns 0 on success, negative errno on failure.
 */
int acpi_gergios_init(void);

#endif /* _ACPI_GERGIOS_H_ */
