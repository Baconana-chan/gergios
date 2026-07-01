/* device.c — GergiOS Device Abstraction Implementation
 *
 * Provides the device lifecycle API: create, destroy, reference counting,
 * state transitions, resource management, and lookup.
 *
 * The device tree root is embedded in this file.  All devices form a
 * tree via parent/children pointers.
 */

#include <minix/drivers.h>
#include <minix/sysutil.h>
#include <sys/queue.h>

#include "gergios_driver.h"
#include "gergios_device.h"

/*===========================================================================*
 *			Global device tree root				     *
 *===========================================================================*/

/* Root device (has dev_id = 0, no parent) */
static struct gergios_device root_device = {
	.dev_id    = 0,
	.driver    = NULL,
	.state     = GERGIOS_DEV_ACTIVE,
	.parent    = NULL,
	.ref_count = 1,
};

/* Next available device ID */
static int next_device_id = 1;

/*===========================================================================*
 *			Internal helpers				     *
 *===========================================================================*/

/* Recursive find by ID starting from a given device */
static struct gergios_device *
find_dev_by_id(struct gergios_device *dev, int dev_id)
{
	struct gergios_device *child, *found;

	if (dev == NULL)
		return NULL;

	if (dev->dev_id == dev_id)
		return dev;

	TAILQ_FOREACH(child, &dev->children, siblings) {
		found = find_dev_by_id(child, dev_id);
		if (found != NULL)
			return found;
	}

	return NULL;
}

/*===========================================================================*
 *			gergios_device_create				     *
 *===========================================================================*/
struct gergios_device *
gergios_device_create(struct gergios_device *parent,
    uint16_t vendor_id, uint16_t device_id,
    uint16_t subvendor_id, uint16_t subdevice_id,
    uint32_t class_code, uint32_t bus_address)
{
	struct gergios_device *dev;

	/* Allocate */
	dev = malloc(sizeof(*dev));
	if (dev == NULL)
		panic("gergios_device_create: out of memory");

	memset(dev, 0, sizeof(*dev));

	/* Initialize */
	dev->dev_id       = next_device_id++;
	dev->state        = GERGIOS_DEV_UNBOUND;
	dev->ref_count    = 1;
	dev->parent       = parent;
	dev->vendor_id    = vendor_id;
	dev->device_id    = device_id;
	dev->subvendor_id = subvendor_id;
	dev->subdevice_id = subdevice_id;
	dev->class_code   = class_code;
	dev->bus_address  = bus_address;
	dev->owner        = NONE;
	dev->major        = -1;
	dev->minor        = -1;

	TAILQ_INIT(&dev->children);

	/* Link to parent */
	if (parent != NULL) {
		TAILQ_INSERT_HEAD(&parent->children, dev, siblings);
		gergios_device_get(parent);
	} else {
		/* Root device */
		TAILQ_INSERT_HEAD(&root_device.children, dev, siblings);
	}

	return dev;
}

/*===========================================================================*
 *			gergios_device_destroy				     *
 *===========================================================================*/
void
gergios_device_destroy(struct gergios_device *dev)
{
	struct gergios_device *child;

	if (dev == NULL || dev == &root_device)
		return;

	/* Remove all children first */
	while (!TAILQ_EMPTY(&dev->children)) {
		child = TAILQ_FIRST(&dev->children);
		gergios_device_destroy(child);
	}

	/* Remove from parent */
	if (dev->parent != NULL)
		TAILQ_REMOVE(&dev->parent->children, dev, siblings);
	else
		TAILQ_REMOVE(&root_device.children, dev, siblings);

	/* Release parent reference */
	if (dev->parent != NULL)
		gergios_device_put(dev->parent);

	/* Free private data */
	if (dev->private)
		free(dev->private);

	/* Free self */
	memset(dev, 0, sizeof(*dev));
	free(dev);
}

/*===========================================================================*
 *			gergios_device_get / put			     *
 *===========================================================================*/
void
gergios_device_get(struct gergios_device *dev)
{
	if (dev == NULL || dev == &root_device)
		return;
	dev->ref_count++;
}

void
gergios_device_put(struct gergios_device *dev)
{
	if (dev == NULL || dev == &root_device)
		return;
	dev->ref_count--;
	if (dev->ref_count <= 0)
		gergios_device_destroy(dev);
}

/*===========================================================================*
 *			gergios_device_set_state			     *
 *===========================================================================*/
int
gergios_device_set_state(struct gergios_device *dev, gergios_dev_state_t state)
{
	if (dev == NULL)
		return EINVAL;

	/* Basic state machine validation */
	switch (state) {
	case GERGIOS_DEV_UNBOUND:
		/* Can only transition from DEAD or initial */
		if (dev->state != GERGIOS_DEV_DEAD && dev->state != GERGIOS_DEV_UNBOUND)
			return EBUSY;
		break;

	case GERGIOS_DEV_ATTACHED:
		if (dev->state != GERGIOS_DEV_UNBOUND)
			return EBUSY;
		break;

	case GERGIOS_DEV_ACTIVE:
		if (dev->state != GERGIOS_DEV_ATTACHED && dev->state != GERGIOS_DEV_SLEEPING)
			return EBUSY;
		break;

	case GERGIOS_DEV_SLEEPING:
		if (dev->state != GERGIOS_DEV_ACTIVE)
			return EBUSY;
		break;

	case GERGIOS_DEV_ZOMBIE:
		/* Can transition from any bound state */
		break;

	case GERGIOS_DEV_DEAD:
		/* Can always be forced to DEAD for cleanup */
		break;
	}

	dev->state = state;
	return OK;
}

/*===========================================================================*
 *			gergios_device_add_resource			     *
 *===========================================================================*/
int
gergios_device_add_resource(struct gergios_device *dev,
    const struct gergios_resource *res)
{
	if (dev == NULL || res == NULL)
		return EINVAL;

	if (dev->num_resources >= GERGIOS_DEVICE_MAX_RESOURCES)
		return ENOMEM;

	memcpy(&dev->resources[dev->num_resources], res,
	    sizeof(*res));
	dev->num_resources++;

	return OK;
}

/*===========================================================================*
 *			gergios_device_find				     *
 *===========================================================================*/
struct gergios_device *
gergios_device_find(int dev_id)
{
	return find_dev_by_id(&root_device, dev_id);
}
