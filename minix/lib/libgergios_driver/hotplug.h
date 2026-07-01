/* hotplug.h — ACPI Hot-Plug & Device Autoloading for GergiOS
 *
 * Completes the deferred Phase 2 functionality by integrating:
 * 1. ACPI Notify handler for PCIe hot-plug events (DEVICE_CHECK/BUS_CHECK)
 * 2. devman device registration on hot-plug ADD events
 * 3. RS (Reincarnation Server) driver autoloading via RS_UP/RS_DOWN
 * 4. Automatic PCI bus rescans triggered by ACPI events
 *
 * Flow:
 *   ACPI Notify → gergios_hotplug_acpi_handler()
 *     ├── DEVICE_CHECK → gergios_pci_rescan_slot(bus, dev, func)
 *     │     ├── gergios_device_create() + gergios_device_match()
 *     │     ├── devman_add_device() — register in VTreeFS
 *     │     └── gergios_driver_autoload() — RS lookup/start driver
 *     └── BUS_CHECK → gergios_pci_rescan_bus()
 *           └── pci_first_dev/pci_next_dev full enumeration
 */

#ifndef _GERGIOS_HOTPLUG_H
#define _GERGIOS_HOTPLUG_H

#include <minix/config.h>
#include <minix/type.h>
#include <minix/endpoint.h>
#include <minix/ipc.h>

#include "gergios_driver.h"
#include "gergios_device.h"
#include "pci_scan.h"

/*===========================================================================*
 *		ACPI Notify handler					     *
 *===========================================================================*/

/* Initialise the ACPI hot-plug subsystem.
 * Registers an ACPI Notify handler for the PCI root bridge (\_SB.PCI0).
 * Returns 0 on success, negative errno if ACPI is not available.
 */
int gergios_hotplug_acpi_init(void);

/* ACPI Notify handler callback.
 * Called by ACPICA when a PCI device sends a Notify event.
 * Notify values:
 *   0x00 (BUS_CHECK) — re-enumerate the entire bus
 *   0x01 (DEVICE_CHECK) — check a specific device
 *   0x03 (EJECT_REQUEST) — device is about to be removed
 *   0x04 (DEVICE_CHECK_LIGHT) — light-weight device check
 */
void gergios_hotplug_acpi_handler(void *context, uint32_t notify_value);

/*===========================================================================*
 *		PCI bus rescans (triggered by ACPI)			     *
 *===========================================================================*/

/* Rescan a specific PCI slot (bus:dev:func) for a new device.
 * Called when ACPI Notify DEVICE_CHECK is received for a slot.
 * Returns 0 on success (device found and handled), negative on error.
 */
int gergios_pci_rescan_slot(uint8_t bus, uint8_t dev, uint8_t func);

/* Rescan the entire PCI bus for new/removed devices.
 * Called when ACPI Notify BUS_CHECK is received.
 * Returns the number of new devices found, or negative on error.
 */
int gergios_pci_rescan_bus(void);

/*===========================================================================*
 *		devman device registration				     *
 *===========================================================================*/

/* Register a new device with devman (VTreeFS device tree).
 * Creates the device node with vendor/device/class info in /devices/.
 * Returns the devman device ID (positive), or negative errno on error.
 */
int gergios_hotplug_devman_add(struct gergios_device *dev);

/* Remove a device from devman.
 * Called when a device is hot-removed or driver unloaded.
 */
int gergios_hotplug_devman_remove(struct gergios_device *dev);

/*===========================================================================*
 *		RS driver autoloading					     *
 *===========================================================================*/

/* Automatically start a driver for a newly appeared device.
 * Looks up the driver name from the device's ID table match,
 * then sends RS_UP to start the driver service.
 * Returns 0 on success, negative errno if driver not found or start fails.
 */
int gergios_hotplug_autoload_driver(struct gergios_device *dev);

/* Stop a driver when its last device is removed.
 * Sends RS_DOWN to stop the driver service.
 */
int gergios_hotplug_unload_driver(struct gergios_device *dev);

/* Register a driver name → PCI ID mapping for autoloading.
 * Format: "driver_name" → { vendor, device, subvendor, subdevice, class_mask }
 * Called by drivers during their init to advertise autoload capability.
 */
int gergios_hotplug_register_driver_map(const char *driver_name,
    const struct gergios_device_id *id_table);

/*===========================================================================*
 *		PCIe Native Hot-Plug					     *
 *===========================================================================*/

/* Maximum number of PCIe hot-plug capable root ports to monitor. */
#define GERGIOS_HP_MAX_PORTS	16

/* Descriptor for a PCIe slot with hot-plug capability. */
struct gergios_hp_slot {
	int	 devind;	/* PCI devind for this port */
	uint8_t  cap_offset;	/* PCIe capability offset in config space */
	uint8_t  bus;		/* bus number */
	uint8_t  dev;		/* device number */
	uint8_t  func;		/* function number */
	uint16_t pds_last;	/* last known Presence Detect State (bit 6+16) */
	uint16_t slot_num;	/* Physical Slot Number from SLCAP */
	unsigned int valid : 1;
};

/* Initialise PCIe Native Hot-Plug subsystem.
 * Scans all PCI devices for Downstream Ports (type 0x4/0x6)
 * with Slot Implemented and Hot-Plug Capable bits set.
 * Returns number of HP-capable ports found, or negative on error.
 */
int gergios_hotplug_pcie_nh_init(void);

/* Poll all known hot-plug capable slots for presence changes.
 * If a Presence Detect Changed event is detected, reads the
 * current Presence Detect State, logs the change, and triggers
 * a PCI bus rescan.
 * Returns number of events detected, or 0 if none.
 */
int gergios_hotplug_pcie_nh_poll(void);

/* Return the array of hot-plug slot descriptors.
 * Fills *count with the number of valid slots.
 */
const struct gergios_hp_slot *gergios_hotplug_pcie_nh_slots(
    unsigned int *count);

/*===========================================================================*
 *		Real RS_UP driver autoloading				     *
 *===========================================================================*/

/* Actually start a driver via RS_UP.
 * Uses fork() + exec() to call /sbin/service for the driver,
 * then sets up PCI ACL via BUSC_PCI_SET_ACL to the PCI server.
 *
 * @param driver_name  Name of the driver (e.g. "ahci", "e1000")
 * @param driver_path  Full path to the driver binary (e.g. "/sbin/ahci")
 * @param devind       PCI devind of the device this driver should control
 * @param vid, did     PCI vendor/device ID for ACL setup
 * Returns 0 on success, negative errno on failure.
 */
int gergios_hotplug_rs_up(const char *driver_name,
    const char *driver_path, int devind,
    uint16_t vid, uint16_t did);

/* Stop a driver via RS_DOWN.
 * Uses system() to call /sbin/service down <driver_name>.
 */
int gergios_hotplug_rs_down(const char *driver_name);


/*===========================================================================*
 *		Top-level hot-plug initialisation			     *
 *===========================================================================*/

/* Initialise the complete hot-plug subsystem.
 * This function:
 *   1. Records existing PCI device indices (devind) for dedup
 *   2. Registers built-in driver mappings (PCI ID -> driver name)
 *   3. Initialises ACPI Notify handler (if ACPI available)
 *   4. Sets up devman IPC channel
 *   5. Initialises the driver mapping table for autoloading
 *   6. Scans for PCIe Native Hot-Plug capable ports
 *
 * IMPORTANT ORDERING: This function MUST be called AFTER
 * gergios_pci_probe() and gergios_driver_init(), because it
 * pre-populates the known-devind list by scanning devices that
 * were already found during the initial PCI probe.  Calling
 * before the probe would miss existing devices, causing
 * duplicate gergios_device instances on the first rescan.
 *
 * Should be called once during system startup.
 */
int gergios_hotplug_init(void);

/* Periodic hot-plug poll — should be called periodically (e.g. every
 * 2 seconds from a timer or idle loop).  Checks both ACPI events
 * (if available) and PCIe Native Hot-Plug slot status.
 */
int gergios_hotplug_poll(void);

#endif /* _GERGIOS_HOTPLUG_H */

#endif /* _GERGIOS_HOTPLUG_H */
