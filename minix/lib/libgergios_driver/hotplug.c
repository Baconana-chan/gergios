/* hotplug.c — ACPI Hot-Plug & Device Autoloading Implementation
 *
 * Implements the Phase 2 functionality:
 *   - ACPI Notify -> PCIe hot-plug event handling
 *   - PCIe Native Hot-Plug (slot polling via Presence Detect)
 *   - devman device tree registration
 *   - RS driver autoloading via fork+exec + PCI ACL
 */

#include <minix/drivers.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <minix/sysutil.h>
#include <minix/type.h>
#include <minix/com.h>
#include <minix/ipc.h>
#include <minix/rs.h>
#include <minix/ds.h>
#include <sys/wait.h>
#include <unistd.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <dev/pci/pcireg.h>

#include "gergios_device.h"
#include "gergios_driver.h"
#include "pci_scan.h"
#include "hotplug.h"

/*===========================================================================*
 *		External ACPI functions (weak stubs)			     *
 *===========================================================================*/
extern int AcpiInstallNotifyHandler(void *device, uint32_t handler_type,
    void (*handler)(void *, uint32_t), void *context);
extern void *AcpiGetHandle(void *, char *);

__attribute__((weak))
int AcpiInstallNotifyHandler(void *device, uint32_t handler_type,
    void (*handler)(void *, uint32_t), void *context)
{
	(void)device; (void)handler_type; (void)handler; (void)context;
	return 1;
}

__attribute__((weak))
void *AcpiGetHandle(void *parent, char *path)
{
	(void)parent; (void)path;
	return NULL;
}

/*===========================================================================*
 *		External PCI config access functions			     *
 *===========================================================================*/
/* PCI config space access — provided by libpci via libsys */
extern u8_t  pci_attr_r8(int devind, int port);
extern u16_t pci_attr_r16(int devind, int port);
extern u32_t pci_attr_r32(int devind, int port);
extern void  pci_attr_w32(int devind, int port, u32_t val);

/* PCI device enumeration — provided by libpci */
extern int   pci_first_dev(int *devindp, u16_t *vidp, u16_t *didp);
extern int   pci_next_dev(int *devindp, u16_t *vidp, u16_t *didp);

/*===========================================================================*
 *		Internal state						*
 *===========================================================================*/

#define GERGIOS_HOTPLUG_MAX_MAPS	64
#define GERGIOS_MAX_PCI_DEVICES		256

struct driver_map {
	char		name[48];
	struct gergios_device_id id;
	unsigned int	in_use : 1;
};

static struct driver_map driver_maps[GERGIOS_HOTPLUG_MAX_MAPS];
static unsigned int driver_map_count = 0;
static int hotplug_initialised = 0;
static int acpi_available = 0;

/* devind dedup */
static int known_devinds[GERGIOS_MAX_PCI_DEVICES];
static unsigned int known_devind_count = 0;

/* ACPI root bridge handle */
static void *pci_root_handle = NULL;

/* PCIe Native Hot-Plug slot descriptors */
static struct gergios_hp_slot hp_slots[GERGIOS_HP_MAX_PORTS];
static unsigned int hp_slot_count = 0;

/*===========================================================================*
 *		Helper: devind dedup					*
 *===========================================================================*/

static int devind_is_known(int devind)
{
	for (unsigned int i = 0; i < known_devind_count; i++)
		if (known_devinds[i] == devind) return 1;
	return 0;
}

static int devind_mark_known(int devind)
{
	if (known_devind_count >= GERGIOS_MAX_PCI_DEVICES) return -1;
	known_devinds[known_devind_count++] = devind;
	return 0;
}

/*===========================================================================*
 *		Helper: walk PCI capabilities				*
 *===========================================================================*/

/* Walk the PCI capabilities list and return the offset (in config space)
 * of the first capability matching @cap_id, or 0 if not found.
 */
static int find_pci_cap(int devind, uint8_t cap_id)
{
	uint8_t cap_ptr;
	uint16_t cap_entry;

	/* Check if the device supports capabilities list */
	uint16_t status = pci_attr_r16(devind, PCI_COMMAND_STATUS_REG);
	if (!(status & PCI_STATUS_CAPLIST_SUPPORT))
		return 0;

	/* Read the capabilities pointer (offset depends on header type) */
	uint32_t bhlc = pci_attr_r32(devind, PCI_BHLC_REG);
	uint8_t hdr_type = PCI_HDRTYPE(bhlc) & 0x7f;

	uint8_t cap_ptr_reg;
	if (hdr_type == PCI_HDRTYPE_DEVICE || hdr_type == PCI_HDRTYPE_PPB)
		cap_ptr_reg = PCI_CAPLISTPTR_REG;	/* offset 0x34 */
	else if (hdr_type == PCI_HDRTYPE_PCB)
		cap_ptr_reg = PCI_CARDBUS_CAPLISTPTR_REG; /* offset 0x14 */
	else
		return 0;

	cap_ptr = pci_attr_r8(devind, cap_ptr_reg);

	while (cap_ptr != 0) {
		cap_entry = pci_attr_r16(devind, cap_ptr);
		if ((cap_entry & 0xff) == cap_id)
			return cap_ptr;
		cap_ptr = (cap_entry >> 8) & 0xff;
	}
	return 0;
}

/*===========================================================================*
 *		PCI bus rescans						*
 *===========================================================================*/

int gergios_pci_rescan_slot(uint8_t bus, uint8_t dev, uint8_t func)
{
	int devind;

	/* Use raw BUSC_PCI_FIND_DEV IPC to the PCI server for BDF lookup.
	 * The libpci wrapper (pci_find_dev) searches by vendor/device ID,
	 * not by BDF, so we must send the raw message ourselves. */
	{
		message m;
		endpoint_t pci_ep;
		int r = minix_rs_lookup("pci", &pci_ep);
		if (r != OK) {
			printf("gergios_hotplug: cannot find PCI server\n");
			return -ENODEV;
		}
		memset(&m, 0, sizeof(m));
		m.m_type = BUSC_PCI_FIND_DEV;
		m.m1_i1 = bus;
		m.m1_i2 = dev;
		m.m1_i3 = func;
		r = ipc_sendrec(pci_ep, &m);
		if (r != OK || m.m_type != 1) {
			printf("gergios_hotplug: slot %02x:%02x.%x empty\n",
			    bus, dev, func);
			return -ENODEV;
		}
		devind = m.m1_i1;
	}

	if (devind_is_known(devind))
		return 0;

	uint32_t id_reg = pci_attr_r32(devind, 0);
	uint16_t vid = id_reg & 0xffff;
	uint16_t did = (id_reg >> 16) & 0xffff;

	uint16_t subvid = pci_attr_r16(devind, PCI_SUBSYS_ID_REG);
	uint16_t subdid = pci_attr_r16(devind, PCI_SUBSYS_ID_REG + 2) >> 16;
	uint32_t class = gergios_pci_get_class(devind);

	printf("gergios_hotplug: new device at %02x:%02x.%x "
	    "devind=%d %04x:%04x class=%06x\n",
	    bus, dev, func, devind, vid, did, class);

	struct gergios_device *gdev = gergios_device_create(NULL,
	    vid, did, subvid, subdid, class, (uint32_t)devind);
	if (gdev == NULL)
		return -ENOMEM;

	pci_reserve(devind);
	devind_mark_known(devind);
	gergios_hotplug_autoload_driver(gdev);
	return 0;
}

int gergios_pci_rescan_bus(void)
{
	int devind;
	u16_t vid, did;
	int count = 0;

	printf("gergios_hotplug: rescanning PCI bus...\n");

	pci_init();
	devind = 0;
	if (!pci_first_dev(&devind, &vid, &did))
		return 0;

	while (1) {
		if (devind_is_known(devind)) {
			if (!pci_next_dev(&devind, &vid, &did)) break;
			continue;
		}

		uint16_t subvid = pci_attr_r16(devind, 0x2C);
		uint16_t subdid = pci_attr_r16(devind, 0x2E);
		uint32_t class = gergios_pci_get_class(devind);

		printf("gergios_hotplug: new device devind=%d "
		    "%04x:%04x class=%06x\n",
		    devind, vid, did, class);

		struct gergios_device *gdev = gergios_device_create(NULL,
		    vid, did, subvid, subdid, class, (uint32_t)devind);
		if (gdev != NULL) {
			pci_reserve(devind);
			devind_mark_known(devind);
			gergios_hotplug_autoload_driver(gdev);
			count++;
		}

		if (!pci_next_dev(&devind, &vid, &did)) break;
	}

	printf("gergios_hotplug: bus rescan found %d new devices\n", count);
	return count;
}

/*===========================================================================*
 *		ACPI Notify handler					*
 *===========================================================================*/

void gergios_hotplug_acpi_handler(void *context, uint32_t notify_value)
{
	(void)context;

	switch (notify_value) {
	case 0x00: /* BUS_CHECK */
		printf("gergios_hotplug: ACPI BUS_CHECK\n");
		gergios_pci_rescan_bus();
		break;
	case 0x01: /* DEVICE_CHECK */
	case 0x04: /* DEVICE_CHECK_LIGHT */
		printf("gergios_hotplug: ACPI DEVICE_CHECK\n");
		gergios_pci_rescan_bus();
		break;
	case 0x03: /* EJECT_REQUEST */
		printf("gergios_hotplug: ACPI EJECT_REQUEST\n");
		break;
	default:
		printf("gergios_hotplug: unknown ACPI Notify 0x%x\n",
		    notify_value);
		break;
	}
}

int gergios_hotplug_acpi_init(void)
{
	static const char *paths[] = {
		"\\_SB_.PCI0",
		"\\_SB_.PC00",
		"_SB_.PCI0",
		NULL
	};

	for (int i = 0; paths[i] != NULL; i++) {
		pci_root_handle = AcpiGetHandle(NULL, (char *)paths[i]);
		if (pci_root_handle != NULL) {
			printf("gergios_hotplug: PCI root found via '%s'\n",
			    paths[i]);
			break;
		}
	}

	if (pci_root_handle == NULL) {
		printf("gergios_hotplug: PCI root not found in ACPI\n");
		acpi_available = 0;
		return -ENODEV;
	}

	int r = AcpiInstallNotifyHandler(pci_root_handle,
	    2, gergios_hotplug_acpi_handler, NULL);
	if (r != 0) {
		printf("gergios_hotplug: AcpiInstallNotifyHandler failed: %d\n", r);
		acpi_available = 0;
		return -ENODEV;
	}

	printf("gergios_hotplug: ACPI Notify handler installed\n");
	acpi_available = 1;
	return 0;
}

/*===========================================================================*
 *		devman device registration				*
 *===========================================================================*/

int gergios_hotplug_devman_add(struct gergios_device *dev)
{
	message m;
	endpoint_t devman_ep;
	int r = minix_rs_lookup("devman", &devman_ep);
	if (r != OK) return -ENODEV;

	memset(&m, 0, sizeof(m));
	m.m_type = DEVMAN_ADD_DEV;
	m.DEVMAN_DEVICE_ID = dev->dev_id;
	r = ipc_sendrec(devman_ep, &m);
	return (r == OK) ? m.DEVMAN_RESULT : r;
}

int gergios_hotplug_devman_remove(struct gergios_device *dev)
{
	message m;
	endpoint_t devman_ep;
	int r = minix_rs_lookup("devman", &devman_ep);
	if (r != OK) return -ENODEV;

	memset(&m, 0, sizeof(m));
	m.m_type = DEVMAN_DEL_DEV;
	m.DEVMAN_DEVICE_ID = dev->dev_id;
	r = ipc_sendrec(devman_ep, &m);
	return (r == OK) ? m.DEVMAN_RESULT : r;
}

/*===========================================================================*
 *		RS driver autoloading (matching)			*
 *===========================================================================*/

int gergios_hotplug_register_driver_map(const char *driver_name,
    const struct gergios_device_id *id_table)
{
	if (driver_map_count >= GERGIOS_HOTPLUG_MAX_MAPS)
		return -ENOMEM;

	for (unsigned int i = 0;
	     id_table[i].vendor != 0xFFFF || id_table[i].device != 0xFFFF;
	     i++) {
		if (driver_map_count >= GERGIOS_HOTPLUG_MAX_MAPS) break;

		struct driver_map *map = &driver_maps[driver_map_count];
		strncpy(map->name, driver_name, sizeof(map->name) - 1);
		map->name[sizeof(map->name) - 1] = '\0';
		map->id = id_table[i];
		map->in_use = 1;
		driver_map_count++;
	}
	return 0;
}

/*===========================================================================*
 *		Real RS_UP driver autoloading				*
 *===========================================================================*/

int gergios_hotplug_rs_up(const char *driver_name,
    const char *driver_path, UNUSED(int) devind,
    UNUSED(uint16_t) vid, UNUSED(uint16_t) did)
{
	pid_t pid;
	int status;
	endpoint_t driver_ep;

	/* Check if driver is already running */
	if (minix_rs_lookup(driver_name, &driver_ep) == OK) {
		printf("gergios_hotplug: driver '%s' already running (ep=%d)\n",
		    driver_name, driver_ep);
		return 0;
	}

	printf("gergios_hotplug: starting driver '%s' via RS_UP\n",
	    driver_name);

	/* Fork to start the driver via /sbin/service.
	 * RS handles PCI ACL setup automatically via the service's
	 * /etc/system.conf entry.  The driver itself must have the
	 * appropriate PCI device IDs configured there. */
	pid = fork();
	if (pid < 0) {
		printf("gergios_hotplug: fork failed: %d\n", errno);
		return -errno;
	}

	if (pid == 0) {
		execl("/sbin/service", "service",
		    "up", driver_path, (char *)NULL);
		_exit(1);
	}

	if (waitpid(pid, &status, 0) < 0) {
		printf("gergios_hotplug: waitpid failed: %d\n", errno);
		return -errno;
	}

	if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) {
		printf("gergios_hotplug: service up failed for '%s'\n",
		    driver_name);
		return -EIO;
	}

	/* Poll for the driver to register with DS (up to ~3 seconds) */
	printf("gergios_hotplug: waiting for driver '%s' to register...\n",
	    driver_name);
	for (int retry = 0; retry < 30; retry++) {
		if (minix_rs_lookup(driver_name, &driver_ep) == OK) {
			printf("gergios_hotplug: driver '%s' registered at ep=%d\n",
			    driver_name, driver_ep);
			return 0;
		}
		usleep(100000);
	}

	printf("gergios_hotplug: timeout waiting for driver '%s'\n",
	    driver_name);
	return -ETIMEDOUT;
}

int gergios_hotplug_rs_down(const char *driver_name)
{
	pid_t pid;
	int status;

	printf("gergios_hotplug: stopping driver '%s' via RS_DOWN\n",
	    driver_name);

	pid = fork();
	if (pid < 0) return -errno;

	if (pid == 0) {
		execl("/sbin/service", "service",
		    "down", driver_name, (char *)NULL);
		_exit(1);
	}

	if (waitpid(pid, &status, 0) < 0)
		return -errno;

	if (!WIFEXITED(status) || WEXITSTATUS(status) != 0)
		return -EIO;

	return 0;
}

/*===========================================================================*
 *		Autoload driver (with real RS_UP)			*
 *===========================================================================*/

int gergios_hotplug_autoload_driver(struct gergios_device *dev)
{
	int r;

	for (unsigned int i = 0; i < driver_map_count; i++) {
		if (!driver_maps[i].in_use) continue;

		const struct gergios_device_id *id = &driver_maps[i].id;

		if ((id->vendor != 0xFFFF && id->vendor != dev->vendor_id) ||
		    (id->device != 0xFFFF && id->device != dev->device_id) ||
		    (id->subvendor != 0xFFFF && id->subvendor != dev->subvendor_id) ||
		    (id->subdevice != 0xFFFF && id->subdevice != dev->subdevice_id))
			continue;

		if (id->class != 0xFFFFFFFF &&
		    (id->class & dev->class_code) != id->class)
			continue;

		printf("gergios_hotplug: autoloading driver '%s' for "
		    "%04x:%04x\n", driver_maps[i].name,
		    dev->vendor_id, dev->device_id);

		/* Build the driver binary path: /sbin/<driver_name> */
		char driver_path[64];
		snprintf(driver_path, sizeof(driver_path),
		    "/sbin/%s", driver_maps[i].name);

		/* Try real RS_UP */
		int devind = (int)dev->bus_address;
		r = gergios_hotplug_rs_up(driver_maps[i].name, driver_path,
		    devind, dev->vendor_id, dev->device_id);
		if (r == 0) {
			/* Also register with devman */
			gergios_hotplug_devman_add(dev);
			return 0;
		}

		printf("gergios_hotplug: RS_UP for '%s' returned %d\n",
		    driver_maps[i].name, r);
		return -EIO;
	}

	return -ENOENT;
}

int gergios_hotplug_unload_driver(struct gergios_device *dev)
{
	(void)dev;
	/* TODO: track driver refcounts; only RS_DOWN when last device removed */
	return -ENOSYS;
}

/*===========================================================================*
 *		PCIe Native Hot-Plug					*
 *===========================================================================*/

/* Scan all PCI devices for Downstream Ports with hot-plug capability.
 * A PCIe Downstream Port (type 0x4 Root Port, or 0x6 Downstream Switch Port)
 * that has the Slot Implemented (SI) bit and Hot-Plug Capable (HPC) bit
 * set in the PCIe capability registers can monitor slot presence. */
int gergios_hotplug_pcie_nh_init(void)
{
	int devind;
	u16_t vid, did;

	hp_slot_count = 0;
	memset(hp_slots, 0, sizeof(hp_slots));

	pci_init();
	devind = 0;
	if (!pci_first_dev(&devind, &vid, &did))
		return 0;

	while (1) {
		int cap_ptr = find_pci_cap(devind, PCI_CAP_PCIEXPRESS);
		if (cap_ptr != 0) {
			/* Read PCIe capability register to check type and SI */
			uint32_t xcap = pci_attr_r32(devind, cap_ptr + 0);
			uint8_t type = (xcap >> 4) & 0xf;
			int slot_impl = (xcap >> 8) & 1;

			/* Downstream Ports: Root Port (0x4) or Downstream
			 * Switch Port (0x6). Also check Upstream Port (0x5)
			 * for completeness, but typically only downstream
			 * ports have slots. */
			if ((type == 0x4 || type == 0x6) && slot_impl) {
				/* Check Slot Capabilities for Hot-Plug Capable */
				uint32_t slcap = pci_attr_r32(devind,
				    cap_ptr + PCIE_SLCAP);
				int hpc = slcap & 1;	/* bit 0 = HPC */
				int hps = (slcap >> 5) & 1; /* bit 5 = Hot-Plug Surprise */
				int slot_num = (slcap >> 19) & 0x1fff;

				if (hpc && hp_slot_count < GERGIOS_HP_MAX_PORTS) {
					/* Get the BDF for this port */
					uint32_t bhlc = pci_attr_r32(devind,
					    PCI_BHLC_REG);
					uint8_t hdr_type = PCI_HDRTYPE(bhlc) & 0x7f;

					/* Read bus/device/func from PCI config.
					 * For type 0 header, the BDF is the
					 * address we used to find this device.
					 * We can get bus/dev from the devind
					 * heuristic, or read the bridge
					 * secondary bus registers for type 1. */
					struct gergios_hp_slot *slot =
					    &hp_slots[hp_slot_count];
					slot->devind = devind;
					slot->cap_offset = cap_ptr;
					slot->slot_num = slot_num;
					slot->valid = 1;

					/* Try to get bus:dev.func via PCI server */
					uint8_t bus = 0, device = 0, func = 0;
					/* For header type 1 (bridge), read
					 * secondary bus number at offset 0x18 */
					if (hdr_type == PCI_HDRTYPE_PPB) {
						uint32_t bus_reg = pci_attr_r32(
						    devind, PCI_BRIDGE_BUS_REG);
						slot->bus = PCI_BRIDGE_BUS_SECONDARY(
						    bus_reg);
					} else {
						/* For endpoints, BDF is encoded
						 * in the PCI server's devind.
						 * We read BDF from the PCI
						 * extended config or parse
						 * the port number. */
						slot->bus = 0;
					}

					/* Read the current Presence Detect State */
					uint32_t slcsr = pci_attr_r32(devind,
					    cap_ptr + PCIE_SLCSR);
					slot->pds_last =
					    (slcsr >> 22) & 1; /* bit 6+16 = PDS */

					printf("gergios_hotplug: PCIe HP port "
					    "devind=%d slot=%d%s\n",
					    devind, slot_num,
					    hps ? " (surprise)" : "");

					/* Enable hot-plug interrupt events in
					 * Slot Control (if interrupts are used).
					 * For polling, we just enable the bits
					 * and read them. */
					uint32_t slctl = pci_attr_r32(devind,
					    cap_ptr + PCIE_SLCSR);
					/* Enable Presence Detect Changed event */
					slctl |= (1 << 3); /* PDE */
					slctl |= (1 << 5); /* HPE */
					pci_attr_w32(devind,
					    cap_ptr + PCIE_SLCSR, slctl);

					hp_slot_count++;
				}
			}
		}

		if (!pci_next_dev(&devind, &vid, &did)) break;
	}

	printf("gergios_hotplug: found %u PCIe hot-plug capable port(s)\n",
	    hp_slot_count);
	return hp_slot_count;
}

int gergios_hotplug_pcie_nh_poll(void)
{
	int events = 0;

	for (unsigned int i = 0; i < hp_slot_count; i++) {
		struct gergios_hp_slot *slot = &hp_slots[i];
		if (!slot->valid) continue;

		/* Read Slot Control/Status (32-bit). Upper 16 bits = status. */
		uint32_t slcsr = pci_attr_r32(slot->devind,
		    slot->cap_offset + PCIE_SLCSR);
		uint16_t sta = (slcsr >> 16) & 0xffff;
		uint16_t pds = (sta >> 6) & 1; /* Presence Detect State */

		/* Check if Presence Detect Changed */
		if (sta & (1 << 3)) { /* PDC = bit 3 in status half */
			uint8_t bus = slot->bus;
			printf("gergios_hotplug: PCIe NH slot %d: presence "
			    "changed (now %s) on bus %d\n",
			    slot->slot_num,
			    pds ? "present" : "empty", bus);

			/* Clear the PDC status bit (write 1 to clear) */
			pci_attr_w32(slot->devind,
			    slot->cap_offset + PCIE_SLCSR,
			    slcsr | (1 << (3 + 16)));

			slot->pds_last = pds;

			if (pds) {
				/* Device appeared — rescan */
				gergios_pci_rescan_bus();
				events++;
			} else {
				printf("gergios_hotplug: slot %d: device removed\n",
				    slot->slot_num);
				events++;
			}
		}
	}

	return events;
}

const struct gergios_hp_slot *gergios_hotplug_pcie_nh_slots(
    unsigned int *count)
{
	*count = hp_slot_count;
	return hp_slots;
}

/*===========================================================================*
 *		Periodic poll						*
 *===========================================================================*/

int gergios_hotplug_poll(void)
{
	int events = 0;

	/* Poll PCIe Native Hot-Plug slots */
	events += gergios_hotplug_pcie_nh_poll();

	/* ACPI Notify is event-driven via AcpiInstallNotifyHandler,
	 * so no polling needed.  If ACPI is not available, we could
	 * add a fallback periodic rescan here, but that's expensive. */

	return events;
}

/*===========================================================================*
 *		Top-level initialisation				*
 *===========================================================================*/

int gergios_hotplug_init(void)
{
	int r;

	if (hotplug_initialised) return 0;

	memset(driver_maps, 0, sizeof(driver_maps));
	driver_map_count = 0;
	memset(known_devinds, 0, sizeof(known_devinds));
	known_devind_count = 0;

	/* Pre-populate known_devinds */
	{
		int di;
		u16_t v, d;
		pci_init();
		di = 0;
		if (pci_first_dev(&di, &v, &d)) {
			devind_mark_known(di);
			while (pci_next_dev(&di, &v, &d))
				devind_mark_known(di);
		}
	}
	printf("gergios_hotplug: %u existing PCI devices recorded\n",
	    known_devind_count);

	/* Register built-in driver mappings */
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

	printf("gergios_hotplug: %u driver mappings registered\n",
	    driver_map_count);

	/* Try ACPI hot-plug */
	r = gergios_hotplug_acpi_init();
	if (r != 0)
		printf("gergios_hotplug: ACPI not available\n");

	/* Scan for PCIe Native Hot-Plug capable ports */
	gergios_hotplug_pcie_nh_init();

	hotplug_initialised = 1;
	return 0;
}
