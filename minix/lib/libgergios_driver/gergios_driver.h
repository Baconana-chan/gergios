/* gergios_driver.h — Unified GergiOS Driver Model
 *
 * This is the core of the new driver model for GergiOS. It provides:
 *   - A unified driver descriptor (gergios_driver) that replaces the three
 *     separate structs (blockdriver, chardriver, netdriver)
 *   - A device matching framework (gergios_device_id)
 *   - Type-specific operations via an anonymous union
 *   - Registration, lifecycle, and dispatch APIs
 *
 * Backward compatibility:
 *   Existing MINIX drivers can continue using blockdriver, chardriver,
 *   and netdriver structs.  The compat layer in compat.c provides wrapper
 *   functions that convert between old and new interfaces.
 */

#ifndef _GERGIOS_DRIVER_H
#define _GERGIOS_DRIVER_H

#include <minix/config.h>
#include <minix/driver.h>		/* device_t, devminor_t, endpoint_t */
#include <minix/blockdriver.h>		/* devminor_t, iovec_t, part_geom, device_id_t */
#include <minix/chardriver.h>		/* cdev_id_t */
#include <minix/netdriver.h>		/* netdriver_data, netdriver_addr_t */
#include "dma.h"			/* struct gergios_dma_ops (expanded) */

/* Forward declarations */
struct gergios_device;
struct gergios_driver;

/*===========================================================================*
 *		Driver class (type of device)				     *
 *===========================================================================*/
typedef enum {
	GERGIOS_DRIVER_BLOCK,		/* Block device (disk, partition) */
	GERGIOS_DRIVER_CHAR,		/* Character device (TTY, PCI, etc.) */
	GERGIOS_DRIVER_NET,		/* Network device (ethernet) */
	GERGIOS_DRIVER_BUS,		/* Bus controller (PCI, I2C) */
	GERGIOS_DRIVER_AUDIO,		/* Audio device */
	GERGIOS_DRIVER_VIDEO,		/* Video / framebuffer */
	GERGIOS_DRIVER_SENSOR,		/* Sensor device */
	GERGIOS_DRIVER_INPUT,		/* Input device (keyboard, mouse) */
	GERGIOS_DRIVER_OTHER		/* Everything else */
} gergios_driver_class_t;

/*===========================================================================*
 *		Device identification table entry			     *
 *===========================================================================*/
struct gergios_device_id {
	uint16_t vendor;		/* PCI vendor ID, or 0xFFFF = any */
	uint16_t device;		/* PCI device ID, or 0xFFFF = any */
	uint16_t subvendor;		/* PCI subvendor, or 0xFFFF = any */
	uint16_t subdevice;		/* PCI subdevice, or 0xFFFF = any */
	uint32_t class;			/* PCI class code, or 0xFFFFFFFF = any */
	uintptr_t driver_data;		/* Opaque data for the driver */
};

/* Sentinel for end of ID table */
#define GERGIOS_DEVICE_ID_END	{ 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, \
				  0xFFFFFFFF, 0 }

/* PCI convenience macro */
#define GERGIOS_PCI_DEVICE(vid, did) \
	{ vid, did, 0xFFFF, 0xFFFF, 0xFFFFFFFF, 0 }

/*===========================================================================*
 *		Power management states				     *
 *===========================================================================*/
typedef enum {
	GERGIOS_PM_ON,			/* S0  — fully on */
	GERGIOS_PM_SLEEP,		/* S1  — light sleep */
	GERGIOS_PM_DEEP_SLEEP,		/* S2  — deep sleep */
	GERGIOS_PM_OFF,			/* S3+ — off / suspend */
} gergios_pm_state_t;

/*===========================================================================*
 *		Power management operations				     *
 *===========================================================================*/
struct gergios_pm_ops {
	int (*suspend)(struct gergios_device *dev, gergios_pm_state_t state);
	int (*resume)(struct gergios_device *dev);
	int (*runtime_suspend)(struct gergios_device *dev);
	int (*runtime_resume)(struct gergios_device *dev);
};

/*===========================================================================*
 *		Device operations (the actual driver callbacks)		     *
 *===========================================================================*/
struct gergios_driver_ops {
	/* Lifecycle */
	int (*probe)(struct gergios_device *dev);
	int (*init)(struct gergios_device *dev);
	void (*remove)(struct gergios_device *dev);

	/* Power management (optional) */
	const struct gergios_pm_ops *pm;

	/* DMA operations (optional) */
	const struct gergios_dma_ops *dma;

	/* Interrupt handler */
	void (*irq)(struct gergios_device *dev, unsigned int mask);

	/* Timer / alarm handler */
	void (*alarm)(struct gergios_device *dev, clock_t stamp);

	/* Catch-all for unexpected messages (non-const m_ptr for compat) */
	void (*other)(message *m_ptr, int ipc_status);
};

/*===========================================================================*
 *		Unified driver descriptor				     *
 *===========================================================================*/
struct gergios_driver {
	/*** Public fields (set by the driver) ***/

	const char *name;		/* Human-readable driver name */
	gergios_driver_class_t class;	/* Driver class (block/char/net/...) */

	/* Device ID matching table (NULL = match all) */
	const struct gergios_device_id *id_table;

	/* Generic operations */
	struct gergios_driver_ops ops;

	/* Type-specific operations */
	union {
		/* Block driver operations (GERGIOS_DRIVER_BLOCK) */
		struct {
			int (*open)(devminor_t minor, int access);
			int (*close)(devminor_t minor);
			ssize_t (*transfer)(devminor_t minor, int do_write,
			    u64_t pos, endpoint_t endpt, iovec_t *iov,
			    unsigned int count, int flags);
			int (*ioctl)(devminor_t minor, unsigned long request,
			    endpoint_t endpt, cp_grant_id_t grant,
			    endpoint_t user_endpt);
			void (*cleanup)(void);
			struct device *(*part)(devminor_t minor);
			void (*geometry)(devminor_t minor,
			    struct part_geom *part);
			int (*device)(devminor_t minor, device_id_t *id);
		} block;

		/* Character driver operations (GERGIOS_DRIVER_CHAR) */
		struct {
			int (*open)(devminor_t minor, int access,
			    endpoint_t user_endpt);
			int (*close)(devminor_t minor);
			ssize_t (*read)(devminor_t minor, u64_t position,
			    endpoint_t endpt, cp_grant_id_t grant,
			    size_t size, int flags, cdev_id_t id);
			ssize_t (*write)(devminor_t minor, u64_t position,
			    endpoint_t endpt, cp_grant_id_t grant,
			    size_t size, int flags, cdev_id_t id);
			int (*ioctl)(devminor_t minor, unsigned long request,
			    endpoint_t endpt, cp_grant_id_t grant,
			    int flags, endpoint_t user_endpt, cdev_id_t id);
			int (*cancel)(devminor_t minor, endpoint_t endpt,
			    cdev_id_t id);
			int (*select)(devminor_t minor, unsigned int ops,
			    endpoint_t endpt);
		} chr;

		/* Network driver operations (GERGIOS_DRIVER_NET) */
		struct {
			int (*init)(unsigned int instance,
			    netdriver_addr_t *hwaddr, uint32_t *caps,
			    unsigned int *ticks);
			void (*stop)(void);
			void (*set_mode)(unsigned int mode,
			    const netdriver_addr_t *mcast_list,
			    unsigned int mcast_count);
			void (*set_caps)(uint32_t caps);
			void (*set_flags)(uint32_t flags);
			void (*set_media)(uint32_t media);
			void (*set_hwaddr)(const netdriver_addr_t *hwaddr);
			ssize_t (*recv)(struct netdriver_data *data,
			    size_t max);
			int (*send)(struct netdriver_data *data, size_t size);
			unsigned int (*get_link)(uint32_t *media);
			void (*tick)(void);
		} net;
	} u;

	/*** Private fields (managed by driver core) ***/

	/* Linked list of driver instances */
	struct gergios_driver *next;

	/* Pointer back to the driver's own private data */
	void *private;
};

/*===========================================================================*
 *		Driver Core API — called by drivers			     *
 *===========================================================================*/

/* Register a driver with the driver core.  Must be called during SEF init. */
void gergios_driver_register(struct gergios_driver *drv);

/* Announce the driver is up (publishes DS_DRIVER_UP event). */
void gergios_driver_announce(void);

/* Main message loop: receive + dispatch until terminate. */
void gergios_driver_task(struct gergios_driver *drv);

/* Process a single message (for custom message loops). */
void gergios_driver_process(struct gergios_driver *drv, message *m_ptr,
    int ipc_status);

/* Terminate the main loop. */
void gergios_driver_terminate(void);

/*===========================================================================*
 *		Device matching API					     *
 *===========================================================================*/

/* Match a device ID table against a PCI vendor/device/class tuple.
 * Returns the matching entry, or NULL if none matches.
 */
const struct gergios_device_id *
gergios_device_match(const struct gergios_device_id *table,
    uint16_t vendor, uint16_t device, uint16_t subvendor,
    uint16_t subdevice, uint32_t class);

/*===========================================================================*
 *		Compatibility layer — existing MINIX driver structs	     *
 *===========================================================================*/

/* Convert a blockdriver to a gergios_driver wrapper.
 * The returned driver has class = GERGIOS_DRIVER_BLOCK and delegates
 * all calls to the original blockdriver callbacks via adapter functions.
 */
struct gergios_driver *gergios_wrap_blockdriver(const struct blockdriver *bdp,
    const char *name);

/* Convert a chardriver to a gergios_driver wrapper.
 * The returned driver has class = GERGIOS_DRIVER_CHAR and delegates
 * all calls to the original chardriver callbacks via adapter functions.
 */
struct gergios_driver *gergios_wrap_chardriver(const struct chardriver *cdp,
    const char *name);

/* Convert a netdriver to a gergios_driver wrapper.
 * The returned driver has class = GERGIOS_DRIVER_NET and delegates
 * all calls to the original netdriver callbacks via adapter functions.
 */
struct gergios_driver *gergios_wrap_netdriver(const struct netdriver *ndp,
    const char *name);

#endif /* _GERGIOS_DRIVER_H */
