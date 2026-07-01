/* match.c — GergiOS Device ID Matching
 *
 * Provides gergios_device_match() which checks whether a given
 * (vendor, device, subvendor, subdevice, class) tuple matches any
 * entry in a driver's ID table.  Supports wildcards (0xFFFF for
 * vendor/device IDs, 0xFFFFFFFF for class code).
 *
 * This is used by the PCI bus driver to find the correct driver
 * for each device discovered on the bus.  The matching algorithm
 * follows PCI Express Base Specification conventions:
 *   - vendor/device match exactly if not wildcard
 *   - subvendor/subdevice only match if ALL of vendor, device,
 *     subvendor, subdevice are non-wildcard
 *   - class matches if not wildcard
 */

#include <minix/drivers.h>
#include <minix/sysutil.h>

#include "gergios_driver.h"

/*===========================================================================*
 *			gergios_device_match				     *
 *===========================================================================*/
const struct gergios_device_id *
gergios_device_match(const struct gergios_device_id *table,
    uint16_t vendor, uint16_t device,
    uint16_t subvendor, uint16_t subdevice,
    uint32_t class)
{
	const struct gergios_device_id *id;

	if (table == NULL)
		return NULL;

	/* Scan the table until we hit the sentinel (all-0xFFFF/FFFFFFFF) */
	for (id = table;
	     !(id->vendor == 0xFFFF && id->device == 0xFFFF &&
	       id->subvendor == 0xFFFF && id->subdevice == 0xFFFF &&
	       id->class == 0xFFFFFFFF);
	     id++) {

		/* Check vendor */
		if (id->vendor != 0xFFFF && id->vendor != vendor)
			continue;

		/* Check device */
		if (id->device != 0xFFFF && id->device != device)
			continue;

		/* Subsystem match (only if both subvendor and subdevice
		 * are specified — following Linux convention) */
		if (id->subvendor != 0xFFFF && id->subvendor != subvendor)
			continue;

		if (id->subdevice != 0xFFFF && id->subdevice != subdevice)
			continue;

		/* Check class */
		if (id->class != 0xFFFFFFFF && id->class != class)
			continue;

		/* All fields match! */
		return id;
	}

	return NULL;
}
