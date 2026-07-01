/* compat.c — Compatibility Wrappers for Existing MINIX Driver Interfaces
 *
 * This file provides adapter functions that convert existing blockdriver,
 * chardriver, and netdriver structs into gergios_driver structs.
 *
 * Purpose:
 *   Existing MINIX drivers can be used with the new driver core without
 *   any source modifications.  Just link against libgergios_driver and
 *   call gergios_wrap_blockdriver() / gergios_wrap_chardriver() /
 *   gergios_wrap_netdriver() to get a gergios_driver* that delegates
 *   all operations to the original struct's callbacks via proper
 *   adapter functions (no undefined-behavior casts).
 *
 * Usage:
 *   struct blockdriver my_block = { ... };
 *   struct gergios_driver *gdrv = gergios_wrap_blockdriver(&my_block, "ahci");
 *   gergios_driver_register(gdrv);
 *   gergios_driver_task(gdrv);
 */

#include <minix/drivers.h>
#include <minix/blockdriver.h>
#include <minix/chardriver.h>
#include <minix/netdriver.h>
#include <minix/sysutil.h>

#include "gergios_driver.h"

/*===========================================================================*
 *			Block driver adapter functions			     *
 *===========================================================================*/

/* Each adapter stores the wrapped blockdriver pointer */
static const struct blockdriver *wrapped_bdp;

static void
adapter_bdr_irq(struct gergios_device *dev, unsigned int mask)
{
	(void)dev;
	if (wrapped_bdp && wrapped_bdp->bdr_intr)
		wrapped_bdp->bdr_intr(mask);
}

static void
adapter_bdr_alarm(struct gergios_device *dev, clock_t stamp)
{
	(void)dev;
	if (wrapped_bdp && wrapped_bdp->bdr_alarm)
		wrapped_bdp->bdr_alarm(stamp);
}

/* Catch-all for unexpected messages — delegates original bdr_other */
static void
adapter_bdr_other(message *m_ptr, int ipc_status)
{
	if (wrapped_bdp && wrapped_bdp->bdr_other)
		wrapped_bdp->bdr_other(m_ptr, ipc_status);
}

/*===========================================================================*
 *			Character driver adapter functions		     *
 *===========================================================================*/

static const struct chardriver *wrapped_cdp;

static void
adapter_cdr_irq(struct gergios_device *dev, unsigned int mask)
{
	(void)dev;
	if (wrapped_cdp && wrapped_cdp->cdr_intr)
		wrapped_cdp->cdr_intr(mask);
}

static void
adapter_cdr_alarm(struct gergios_device *dev, clock_t stamp)
{
	(void)dev;
	if (wrapped_cdp && wrapped_cdp->cdr_alarm)
		wrapped_cdp->cdr_alarm(stamp);
}

static void
adapter_cdr_other(message *m_ptr, int ipc_status)
{
	if (wrapped_cdp && wrapped_cdp->cdr_other)
		wrapped_cdp->cdr_other(m_ptr, ipc_status);
}

/*===========================================================================*
 *			Network driver adapter functions		     *
 *===========================================================================*/

static const struct netdriver *wrapped_ndp;

static void
adapter_ndr_irq(struct gergios_device *dev, unsigned int mask)
{
	(void)dev;
	if (wrapped_ndp && wrapped_ndp->ndr_intr)
		wrapped_ndp->ndr_intr(mask);
}

/* Forward: netdriver_process from libnetdriver */
extern void netdriver_process(const struct netdriver *ndp,
    const message *m_ptr, int ipc_status);

static void
adapter_ndr_other(message *m_ptr, int ipc_status)
{
	/* Delegate NDEV messages to libnetdriver's process function */
	if (wrapped_ndp)
		netdriver_process(wrapped_ndp, m_ptr, ipc_status);
}

static int
adapter_ndr_probe(struct gergios_device *dev)
{
	(void)dev;
	/* Network drivers are initialized during SEF init (ndr_init),
	 * not at probe time.  OK is the default. */
	return OK;
}

static int
adapter_ndr_init(struct gergios_device *dev)
{
	(void)dev;
	return OK;
}

/*===========================================================================*
 *			gergios_wrap_blockdriver			     *
 *===========================================================================*/
struct gergios_driver *
gergios_wrap_blockdriver(const struct blockdriver *bdp, const char *name)
{
	static struct gergios_driver wrapper;
	static char name_buf[32];

	memset(&wrapper, 0, sizeof(wrapper));

	strncpy(name_buf, name, sizeof(name_buf) - 1);
	name_buf[sizeof(name_buf) - 1] = '\0';
	wrapper.name = name_buf;
	wrapper.class = GERGIOS_DRIVER_BLOCK;

	/* Save for adapter functions */
	wrapped_bdp = bdp;

	/* Generic operations — use adapter functions, NOT direct casts */
	wrapper.ops.irq   = adapter_bdr_irq;
	wrapper.ops.alarm = adapter_bdr_alarm;
	wrapper.ops.other = adapter_bdr_other;

	/* Block-specific operations — direct assignments (signatures match) */
	wrapper.u.block.open     = bdp->bdr_open;
	wrapper.u.block.close    = bdp->bdr_close;
	wrapper.u.block.transfer = bdp->bdr_transfer;
	wrapper.u.block.ioctl    = bdp->bdr_ioctl;
	wrapper.u.block.cleanup  = bdp->bdr_cleanup;
	wrapper.u.block.part     = bdp->bdr_part;
	wrapper.u.block.geometry = bdp->bdr_geometry;
	wrapper.u.block.device   = bdp->bdr_device;

	return &wrapper;
}

/*===========================================================================*
 *			gergios_wrap_chardriver			     *
 *===========================================================================*/
struct gergios_driver *
gergios_wrap_chardriver(const struct chardriver *cdp, const char *name)
{
	static struct gergios_driver wrapper;
	static char name_buf[32];

	memset(&wrapper, 0, sizeof(wrapper));

	strncpy(name_buf, name, sizeof(name_buf) - 1);
	name_buf[sizeof(name_buf) - 1] = '\0';
	wrapper.name = name_buf;
	wrapper.class = GERGIOS_DRIVER_CHAR;

	/* Save for adapter functions */
	wrapped_cdp = cdp;

	/* Generic operations */
	wrapper.ops.irq   = adapter_cdr_irq;
	wrapper.ops.alarm = adapter_cdr_alarm;
	wrapper.ops.other = adapter_cdr_other;

	/* Char-specific operations — direct assignments (signatures match) */
	wrapper.u.chr.open   = cdp->cdr_open;
	wrapper.u.chr.close  = cdp->cdr_close;
	wrapper.u.chr.read   = cdp->cdr_read;
	wrapper.u.chr.write  = cdp->cdr_write;
	wrapper.u.chr.ioctl  = cdp->cdr_ioctl;
	wrapper.u.chr.cancel = cdp->cdr_cancel;
	wrapper.u.chr.select = cdp->cdr_select;

	return &wrapper;
}

/*===========================================================================*
 *			gergios_wrap_netdriver				     *
 *===========================================================================*/
struct gergios_driver *
gergios_wrap_netdriver(const struct netdriver *ndp, const char *name)
{
	static struct gergios_driver wrapper;
	static char name_buf[32];

	memset(&wrapper, 0, sizeof(wrapper));

	strncpy(name_buf, name, sizeof(name_buf) - 1);
	name_buf[sizeof(name_buf) - 1] = '\0';
	wrapper.name = name_buf;
	wrapper.class = GERGIOS_DRIVER_NET;

	/* Save for adapter functions */
	wrapped_ndp = ndp;

	/* Generic operations */
	wrapper.ops.probe = adapter_ndr_probe;
	wrapper.ops.init  = adapter_ndr_init;
	wrapper.ops.irq   = adapter_ndr_irq;
	wrapper.ops.other = adapter_ndr_other;

	/* Net-specific operations — direct assignments (signatures match) */
	wrapper.u.net.init      = ndp->ndr_init;
	wrapper.u.net.stop      = ndp->ndr_stop;
	wrapper.u.net.set_mode  = ndp->ndr_set_mode;
	wrapper.u.net.set_caps  = ndp->ndr_set_caps;
	wrapper.u.net.set_flags = ndp->ndr_set_flags;
	wrapper.u.net.set_media  = ndp->ndr_set_media;
	wrapper.u.net.set_hwaddr = ndp->ndr_set_hwaddr;
	wrapper.u.net.recv      = ndp->ndr_recv;
	wrapper.u.net.send      = ndp->ndr_send;
	wrapper.u.net.get_link  = ndp->ndr_get_link;
	wrapper.u.net.tick      = ndp->ndr_tick;

	return &wrapper;
}
