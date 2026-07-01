/* core.c — GergiOS Driver Core: registration, lifecycle, message dispatch
 *
 * This is the heart of the unified driver model.  It provides:
 *   - gergios_driver_register() — register a driver with the core
 *   - gergios_driver_task() — main message loop (replaces blockdriver_task,
 *     chardriver_task, netdriver_task)
 *   - gergios_driver_process() — dispatch a single message to the correct
 *     type-specific handler
 *   - gergios_driver_announce() — publish DS_DRIVER_UP event
 *   - gergios_driver_terminate() — break out of the message loop
 *
 * Replies are built inline rather than calling internal libblockdriver or
 * libchardriver functions, so this core has no link dependency on those
 * libraries beyond what is needed for message type constants.
 */

#include <minix/drivers.h>
#include <minix/ds.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <minix/sysutil.h>
#include <minix/com.h>

#include "gergios_driver.h"
#include "gergios_device.h"

/*===========================================================================*
 *			Global state					     *
 *===========================================================================*/

/* Linked list of all registered drivers */
static struct gergios_driver *driver_list = NULL;

/* Whether the main loop should keep running */
static int running;

/*===========================================================================*
 *			Reply helpers (inline, no external deps)	     *
 *===========================================================================*/

/* Send a BDEV_REPLY message */
static void
block_reply(endpoint_t endpt, int ipc_status, int status, int id)
{
	message m;

	memset(&m, 0, sizeof(m));
	m.m_type = BDEV_REPLY;
	m.m_lblockdriver_lbdev_reply.status = status;
	m.m_lblockdriver_lbdev_reply.id = id;

	if (IPC_STATUS_CALL(ipc_status) == SENDREC)
		ipc_sendnb(endpt, &m);
	else
		asynsend3(endpt, &m, AMF_NOREPLY);
}

/*===========================================================================*
 *			gergios_driver_register				     *
 *===========================================================================*/
void
gergios_driver_register(struct gergios_driver *drv)
{
	if (drv == NULL)
		panic("gergios_driver_register: NULL driver");

	if (drv->name == NULL)
		panic("gergios_driver_register: driver without name");

	/* Insert at head of list */
	drv->next = driver_list;
	driver_list = drv;

	/* Clear private field */
	drv->private = NULL;

	printf("gergios: registered driver '%s' (class %d)\n",
	    drv->name, (int)drv->class);
}

/*===========================================================================*
 *			gergios_driver_announce				     *
 *===========================================================================*/
void
gergios_driver_announce(void)
{
	int r;
	char key[DS_MAX_KEYLEN];
	char label[DS_MAX_KEYLEN];
	const char *prefix;

	/* Determine prefix based on driver class of the first registered
	 * driver.  If no driver is registered, use a generic prefix.
	 */
	if (driver_list != NULL) {
		switch (driver_list->class) {
		case GERGIOS_DRIVER_BLOCK:
			prefix = "drv.blk.";
			break;
		case GERGIOS_DRIVER_CHAR:
			prefix = "drv.chr.";
			break;
		case GERGIOS_DRIVER_NET:
			prefix = "drv.net.";
			break;
		default:
			prefix = "drv.oth.";
			break;
		}
	} else {
		prefix = "drv.oth.";
	}

	/* Callers are allowed to use ipc_sendrec to communicate with drivers.
	 * For this reason, there may be blocked callers when a driver
	 * restarts.  Ask the kernel to unblock them (if any).
	 */
	if ((r = sys_statectl(SYS_STATE_CLEAR_IPC_REFS, 0, 0)) != OK)
		panic("gergios_driver_announce: sys_statectl failed: %d", r);

	/* Publish a driver up event. */
	if ((r = ds_retrieve_label_name(label, sef_self())) != OK)
		panic("gergios_driver_announce: unable to get own label: %d",
		    r);

	snprintf(key, sizeof(key), "%s%s", prefix, label);
	if ((r = ds_publish_u32(key, DS_DRIVER_UP, DSF_OVERWRITE)) != OK)
		panic("gergios_driver_announce: unable to publish up event: %d",
		    r);
}

/*===========================================================================*
 *			Message dispatch				     *
 *===========================================================================*/

/* Block driver message dispatch */
static void
dispatch_block(struct gergios_driver *drv, message *m_ptr, int ipc_status)
{
	int id;

	/* Notifications (interrupts, alarms, etc.) */
	if (is_ipc_notify(ipc_status)) {
		switch (_ENDPOINT_P(m_ptr->m_source)) {
		case HARDWARE:
			if (drv->ops.irq)
				drv->ops.irq(NULL,
				    m_ptr->m_notify.interrupts);
			break;

		case CLOCK:
			if (drv->ops.alarm)
				drv->ops.alarm(NULL,
				    m_ptr->m_notify.timestamp);
			break;

		default:
			if (drv->ops.other)
				drv->ops.other(m_ptr, ipc_status);
		}
		return;
	}

	/* Reply to char open with ENXIO (block driver doesn't handle char) */
	if (m_ptr->m_type == CDEV_OPEN) {
		message reply;

		memset(&reply, 0, sizeof(reply));
		reply.m_type = CDEV_REPLY;
		reply.m_lchardriver_vfs_reply.status = ENXIO;
		reply.m_lchardriver_vfs_reply.id =
		    m_ptr->m_vfs_lchardriver_openclose.id;
		ipc_sendnb(m_ptr->m_source, &reply);
		return;
	}

	/* Block driver dispatch */
	switch (m_ptr->m_type) {
	case BDEV_OPEN:
		id = m_ptr->m_lbdev_lblockdriver_msg.id;
		if (drv->u.block.open) {
			int r = drv->u.block.open(
			    m_ptr->m_lbdev_lblockdriver_msg.minor,
			    m_ptr->m_lbdev_lblockdriver_msg.access);
			block_reply(m_ptr->m_source, ipc_status, r, id);
		} else
			block_reply(m_ptr->m_source, ipc_status, ENODEV, id);
		break;

	case BDEV_CLOSE:
		id = m_ptr->m_lbdev_lblockdriver_msg.id;
		if (drv->u.block.close) {
			int r = drv->u.block.close(
			    m_ptr->m_lbdev_lblockdriver_msg.minor);
			block_reply(m_ptr->m_source, ipc_status, r, id);
		} else
			block_reply(m_ptr->m_source, ipc_status, OK, id);
		break;

	case BDEV_READ:
	case BDEV_WRITE: {
		iovec_t iovec1;

		id = m_ptr->m_lbdev_lblockdriver_msg.id;
		if (!drv->u.block.transfer) {
			block_reply(m_ptr->m_source, ipc_status, ENODEV, id);
			break;
		}

		iovec1.iov_addr = m_ptr->m_lbdev_lblockdriver_msg.grant;
		iovec1.iov_size = m_ptr->m_lbdev_lblockdriver_msg.count;
		ssize_t r = drv->u.block.transfer(
		    m_ptr->m_lbdev_lblockdriver_msg.minor,
		    (m_ptr->m_type == BDEV_WRITE),
		    m_ptr->m_lbdev_lblockdriver_msg.pos,
		    m_ptr->m_source, &iovec1, 1,
		    m_ptr->m_lbdev_lblockdriver_msg.flags);
		block_reply(m_ptr->m_source, ipc_status, r, id);
		break;
	}

	case BDEV_GATHER:
	case BDEV_SCATTER: {
		iovec_t iovec[NR_IOREQS];
		unsigned int i, nr_req;
		ssize_t size;

		id = m_ptr->m_lbdev_lblockdriver_msg.id;
		if (!drv->u.block.transfer) {
			block_reply(m_ptr->m_source, ipc_status, ENODEV, id);
			break;
		}

		nr_req = m_ptr->m_lbdev_lblockdriver_msg.count;
		if (nr_req > NR_IOREQS)
			nr_req = NR_IOREQS;

		if (OK != sys_safecopyfrom(m_ptr->m_source,
		    m_ptr->m_lbdev_lblockdriver_msg.grant, 0,
		    (vir_bytes)iovec, nr_req * sizeof(iovec[0]))) {
			block_reply(m_ptr->m_source, ipc_status, EINVAL, id);
			break;
		}

		/* Check for overflow (matching original libblockdriver) */
		for (i = 0, size = 0; i < nr_req; i++) {
			if ((ssize_t)(size + iovec[i].iov_size) < size) {
				block_reply(m_ptr->m_source, ipc_status,
				    EINVAL, id);
				return;
			}
			size += iovec[i].iov_size;
		}

		ssize_t r = drv->u.block.transfer(
		    m_ptr->m_lbdev_lblockdriver_msg.minor,
		    (m_ptr->m_type == BDEV_SCATTER),
		    m_ptr->m_lbdev_lblockdriver_msg.pos,
		    m_ptr->m_source, iovec, nr_req,
		    m_ptr->m_lbdev_lblockdriver_msg.flags);
		block_reply(m_ptr->m_source, ipc_status, r, id);
		break;
	}

	case BDEV_IOCTL: {
		int r;
		struct device *dv;
		struct part_geom entry;

		id = m_ptr->m_lbdev_lblockdriver_msg.id;

		/* Handle disk-specific partition requests (DIOCSETP/GETP) */
		if (m_ptr->m_lbdev_lblockdriver_msg.request == DIOCSETP &&
		    drv->u.block.part) {
			r = sys_safecopyfrom(m_ptr->m_source,
			    m_ptr->m_lbdev_lblockdriver_msg.grant, 0,
			    (vir_bytes)&entry, sizeof(entry));
			if (r == OK) {
				dv = drv->u.block.part(
				    m_ptr->m_lbdev_lblockdriver_msg.minor);
				if (dv != NULL) {
					dv->dv_base = entry.base;
					dv->dv_size = entry.size;
				} else
					r = ENXIO;
			}
		} else if (m_ptr->m_lbdev_lblockdriver_msg.request ==
		    DIOCGETP && drv->u.block.part) {
			dv = drv->u.block.part(
			    m_ptr->m_lbdev_lblockdriver_msg.minor);
			if (dv != NULL) {
				entry.base = dv->dv_base;
				entry.size = dv->dv_size;
				if (drv->u.block.geometry)
					drv->u.block.geometry(
					    m_ptr->m_lbdev_lblockdriver_msg.
					    minor, &entry);
				else {
					entry.cylinders = (unsigned long)
					    (entry.size / SECTOR_SIZE) /
					    (64 * 32);
					entry.heads = 64;
					entry.sectors = 32;
				}
				r = sys_safecopyto(m_ptr->m_source,
				    m_ptr->m_lbdev_lblockdriver_msg.grant, 0,
				    (vir_bytes)&entry, sizeof(entry));
			} else
				r = ENXIO;
		} else if (drv->u.block.ioctl) {
			r = drv->u.block.ioctl(
			    m_ptr->m_lbdev_lblockdriver_msg.minor,
			    m_ptr->m_lbdev_lblockdriver_msg.request,
			    m_ptr->m_source,
			    m_ptr->m_lbdev_lblockdriver_msg.grant,
			    m_ptr->m_lbdev_lblockdriver_msg.user);
		} else
			r = ENOTTY;

		block_reply(m_ptr->m_source, ipc_status, r, id);
		break;
	}

	default:
		if (drv->ops.other)
			drv->ops.other(m_ptr, ipc_status);
		break;
	}

	/* Post-call cleanup */
	if (drv->u.block.cleanup)
		drv->u.block.cleanup();
}

/* Character driver message dispatch */
static void
dispatch_char(struct gergios_driver *drv, message *m_ptr, int ipc_status)
{
	/* Notifications */
	if (is_ipc_notify(ipc_status)) {
		switch (_ENDPOINT_P(m_ptr->m_source)) {
		case HARDWARE:
			if (drv->ops.irq)
				drv->ops.irq(NULL,
				    m_ptr->m_notify.interrupts);
			break;

		case CLOCK:
			if (drv->ops.alarm)
				drv->ops.alarm(NULL,
				    m_ptr->m_notify.timestamp);
			break;

		default:
			if (drv->ops.other)
				drv->ops.other(m_ptr, ipc_status);
		}
		return;
	}

	/* Reply to block open with ENXIO (char driver doesn't handle block) */
	if (m_ptr->m_type == BDEV_OPEN) {
		message reply;

		memset(&reply, 0, sizeof(reply));
		reply.m_type = BDEV_REPLY;
		reply.m_lblockdriver_lbdev_reply.status = ENXIO;
		reply.m_lblockdriver_lbdev_reply.id =
		    m_ptr->m_lbdev_lblockdriver_msg.id;
		ipc_sendnb(m_ptr->m_source, &reply);
		return;
	}

	/* Character driver dispatch */
	switch (m_ptr->m_type) {
	case CDEV_OPEN:
		if (drv->u.chr.open) {
			int r = drv->u.chr.open(
			    m_ptr->m_vfs_lchardriver_openclose.minor,
			    m_ptr->m_vfs_lchardriver_openclose.access,
			    m_ptr->m_vfs_lchardriver_openclose.user);
			chardriver_reply_task(m_ptr->m_source,
			    m_ptr->m_vfs_lchardriver_openclose.id, r);
		}
		break;

	case CDEV_CLOSE:
		if (drv->u.chr.close) {
			int r = drv->u.chr.close(
			    m_ptr->m_vfs_lchardriver_openclose.minor);
			chardriver_reply_task(m_ptr->m_source,
			    m_ptr->m_vfs_lchardriver_openclose.id, r);
		}
		break;

	case CDEV_READ:
		if (drv->u.chr.read) {
			ssize_t r = drv->u.chr.read(
			    m_ptr->m_vfs_lchardriver_readwrite.minor,
			    m_ptr->m_vfs_lchardriver_readwrite.pos,
			    m_ptr->m_source,
			    m_ptr->m_vfs_lchardriver_readwrite.grant,
			    m_ptr->m_vfs_lchardriver_readwrite.count,
			    m_ptr->m_vfs_lchardriver_readwrite.flags,
			    m_ptr->m_vfs_lchardriver_readwrite.id);
			chardriver_reply_task(m_ptr->m_source,
			    m_ptr->m_vfs_lchardriver_readwrite.id, r);
		}
		break;

	case CDEV_WRITE:
		if (drv->u.chr.write) {
			ssize_t r = drv->u.chr.write(
			    m_ptr->m_vfs_lchardriver_readwrite.minor,
			    m_ptr->m_vfs_lchardriver_readwrite.pos,
			    m_ptr->m_source,
			    m_ptr->m_vfs_lchardriver_readwrite.grant,
			    m_ptr->m_vfs_lchardriver_readwrite.count,
			    m_ptr->m_vfs_lchardriver_readwrite.flags,
			    m_ptr->m_vfs_lchardriver_readwrite.id);
			chardriver_reply_task(m_ptr->m_source,
			    m_ptr->m_vfs_lchardriver_readwrite.id, r);
		}
		break;

	case CDEV_IOCTL:
		if (drv->u.chr.ioctl) {
			int r = drv->u.chr.ioctl(
			    m_ptr->m_vfs_lchardriver_readwrite.minor,
			    m_ptr->m_vfs_lchardriver_readwrite.request,
			    m_ptr->m_source,
			    m_ptr->m_vfs_lchardriver_readwrite.grant,
			    m_ptr->m_vfs_lchardriver_readwrite.flags,
			    m_ptr->m_vfs_lchardriver_readwrite.user,
			    m_ptr->m_vfs_lchardriver_readwrite.id);
			chardriver_reply_task(m_ptr->m_source,
			    m_ptr->m_vfs_lchardriver_readwrite.id, r);
		}
		break;

	case CDEV_CANCEL:
		if (drv->u.chr.cancel) {
			int r = drv->u.chr.cancel(
			    m_ptr->m_vfs_lchardriver_cancel.minor,
			    m_ptr->m_source,
			    m_ptr->m_vfs_lchardriver_cancel.id);
			chardriver_reply_task(m_ptr->m_source,
			    m_ptr->m_vfs_lchardriver_cancel.id, r);
		}
		break;

	case CDEV_SELECT:
		if (drv->u.chr.select) {
			int r = drv->u.chr.select(
			    m_ptr->m_vfs_lchardriver_select.minor,
			    m_ptr->m_vfs_lchardriver_select.ops,
			    m_ptr->m_source);
			message reply;

			memset(&reply, 0, sizeof(reply));
			reply.m_type = CDEV_SEL1_REPLY;
			reply.m_lchardriver_vfs_sel1.status = r;
			reply.m_lchardriver_vfs_sel1.minor =
			    m_ptr->m_vfs_lchardriver_select.minor;
			ipc_sendnb(m_ptr->m_source, &reply);
		}
		break;

	default:
		if (drv->ops.other)
			drv->ops.other(m_ptr, ipc_status);
		break;
	}
}

/* Network driver message dispatch.
 *
 * Pure gergios network drivers (non-wrapped) handle NDEV_* messages
 * directly.  Wrapped netdrivers delegate to libnetdriver's
 * netdriver_process() via the compat layer's other handler.
 */
static void
dispatch_net(struct gergios_driver *drv, message *m_ptr, int ipc_status)
{
	/* Notifications */
	if (is_ipc_notify(ipc_status)) {
		switch (_ENDPOINT_P(m_ptr->m_source)) {
		case HARDWARE:
			if (drv->ops.irq)
				drv->ops.irq(NULL,
				    m_ptr->m_notify.interrupts);
			break;

		default:
			if (drv->ops.other)
				drv->ops.other(m_ptr, ipc_status);
		}
		return;
	}

	/* Delegate NDEV messages to ops.other.  For wrapped drivers this
	 * calls netdriver_process().  Pure gergios network drivers should
	 * handle NDEV_INIT/CONF/SEND/RECV in their own ops.other. */
	if (drv->ops.other)
		drv->ops.other(m_ptr, ipc_status);
}

/*===========================================================================*
 *			gergios_driver_process				     *
 *===========================================================================*/
void
gergios_driver_process(struct gergios_driver *drv, message *m_ptr,
    int ipc_status)
{
	if (drv == NULL || m_ptr == NULL)
		return;

	switch (drv->class) {
	case GERGIOS_DRIVER_BLOCK:
		dispatch_block(drv, m_ptr, ipc_status);
		break;

	case GERGIOS_DRIVER_CHAR:
	case GERGIOS_DRIVER_BUS:
	case GERGIOS_DRIVER_AUDIO:
	case GERGIOS_DRIVER_VIDEO:
	case GERGIOS_DRIVER_SENSOR:
	case GERGIOS_DRIVER_INPUT:
		dispatch_char(drv, m_ptr, ipc_status);
		break;

	case GERGIOS_DRIVER_NET:
		dispatch_net(drv, m_ptr, ipc_status);
		break;

	default:
		if (drv->ops.other)
			drv->ops.other(m_ptr, ipc_status);
		break;
	}
}

/*===========================================================================*
 *			gergios_driver_task				     *
 *===========================================================================*/
void
gergios_driver_task(struct gergios_driver *drv)
{
	int r, ipc_status;
	message mess;

	running = TRUE;

	/* Announce we are up */
	gergios_driver_announce();

	/* Main message loop */
	while (running) {
		if ((r = sef_receive_status(ANY, &mess, &ipc_status)) != OK) {
			if (r == EINTR && !running)
				break;
			panic("gergios_driver_task: sef_receive_status "
			    "failed: %d", r);
		}

		gergios_driver_process(drv, &mess, ipc_status);
	}
}

/*===========================================================================*
 *			gergios_driver_terminate			     *
 *===========================================================================*/
void
gergios_driver_terminate(void)
{
	running = FALSE;
	sef_cancel();
}
