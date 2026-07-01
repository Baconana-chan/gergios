/* gergios_device.h — GergiOS Device Abstraction
 *
 * Represents a single hardware device instance.  Each device is associated
 * with a driver (via gergios_driver), has a unique ID, a parent device
 * (for bus hierarchy), MMIO/IRQ resources, and a state machine.
 *
 * State machine:
 *
 *   UNBOUND ──probe()──> ATTACHED ──init()──> ACTIVE
 *       │                    │                   │
 *       │                    │                   ├──suspend()──> SLEEPING
 *       │                    │                   │         resume()│
 *       │                    │                   │                │
 *       │                    │                   ├──remove()──> DEAD
 *       │                    │                   │
 *       │                    │                   └──[hw removed]──> DEAD
 *       │                    │
 *       │                    └──remove()──> DEAD
 *       │
 *       └──[driver not found]──> (stays UNBOUND)
 */

#ifndef _GERGIOS_DEVICE_H
#define _GERGIOS_DEVICE_H

#include <minix/config.h>
#include <minix/type.h>		/* endpoint_t */
#include <minix/endpoint.h>	/* _ENDPOINT_P */
#include <sys/queue.h>		/* TAILQ_* */

/* Forward declarations */
struct gergios_driver;
struct gergios_device;

/*===========================================================================*
 *		Device state machine					     *
 *===========================================================================*/
typedef enum {
	GERGIOS_DEV_UNBOUND = 0,	/* Not yet matched to a driver */
	GERGIOS_DEV_ATTACHED,		/* Driver found, probe succeeded */
	GERGIOS_DEV_ACTIVE,		/* Driver init succeeded, fully on */
	GERGIOS_DEV_SLEEPING,		/* Suspended (D1/D2/D3hot) */
	GERGIOS_DEV_ZOMBIE,		/* Hardware removed, driver still bound */
	GERGIOS_DEV_DEAD,		/* Removed, ready for free */
} gergios_dev_state_t;

/*===========================================================================*
 *		MMIO / IRQ resource descriptor				     *
 *===========================================================================*/
struct gergios_resource {
	unsigned int type;		/* 0 = MMIO, 1 = port I/O, 2 = IRQ */
	union {
		struct {
			uint64_t base;	/* Physical base address (MMIO) */
			uint64_t size;	/* Region size */
		} mmio;
		struct {
			uint16_t base;	/* I/O port base */
			uint16_t size;	/* Number of ports */
		} port;
		struct {
			int line;	/* IRQ line number */
			int vector;	/* MSI/MSI-X vector */
		} irq;
	} u;
};

/* Maximum number of resources per device */
#define GERGIOS_DEVICE_MAX_RESOURCES 16

/*===========================================================================*
 *		Device structure					     *
 *===========================================================================*/
struct gergios_device {
	/* Unique device ID (assigned by driver core) */
	int dev_id;

	/* Driver that owns this device */
	struct gergios_driver *driver;

	/* Device state */
	gergios_dev_state_t state;

	/* Parent device (for bus hierarchy, e.g. PCI→AHCI) */
	struct gergios_device *parent;

	/* Children list */
	TAILQ_HEAD(, gergios_device) children;
	TAILQ_ENTRY(gergios_device) siblings;

	/* Reference count (for safe removal) */
	int ref_count;

	/* PCI identification (0 if not a PCI device) */
	uint16_t vendor_id;
	uint16_t device_id;
	uint16_t subvendor_id;
	uint16_t subdevice_id;
	uint32_t class_code;

	/* Bus address (PCI BDF encoded: (bus<<16) | (dev<<8) | func) */
	uint32_t bus_address;

	/* Resources (MMIO ranges, IRQs, I/O ports) */
	unsigned int num_resources;
	struct gergios_resource resources[GERGIOS_DEVICE_MAX_RESOURCES];

	/* Driver-specific private data */
	void *private;

	/* Endpoint of the driver process owning this device */
	endpoint_t owner;

	/* Major/minor device numbers (for block/char devices) */
	int major;
	int minor;

	/* Link to devman device tree */
	void *devman_node;
};

/*===========================================================================*
 *		Device API						     *
 *===========================================================================*/

/* Create and register a new device. */
struct gergios_device *gergios_device_create(struct gergios_device *parent,
    uint16_t vendor_id, uint16_t device_id,
    uint16_t subvendor_id, uint16_t subdevice_id,
    uint32_t class_code, uint32_t bus_address);

/* Destroy a device (frees memory, removes from parent, notifies devman). */
void gergios_device_destroy(struct gergios_device *dev);

/* Increment / decrement reference count. */
void gergios_device_get(struct gergios_device *dev);
void gergios_device_put(struct gergios_device *dev);

/* Transition device to a new state.  Returns 0 on success. */
int gergios_device_set_state(struct gergios_device *dev,
    gergios_dev_state_t state);

/* Add a resource to a device.  Returns 0 on success. */
int gergios_device_add_resource(struct gergios_device *dev,
    const struct gergios_resource *res);

/* Find a device by its ID. */
struct gergios_device *gergios_device_find(int dev_id);

/* Return the bus address (PCI BDF encoded) for a device. */
static inline uint64_t
gergios_device_get_bus_address(const struct gergios_device *dev)
{
	return dev->bus_address;
}

#endif /* _GERGIOS_DEVICE_H */
