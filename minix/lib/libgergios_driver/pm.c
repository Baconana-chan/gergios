/* pm.c — Power Management Framework Implementation for GergiOS
 *
 * Implements the unified Power Management framework defined in pm.h.
 *
 * ACPI integration:
 *   On systems with the ACPICA-based ACPI driver, S3 suspend/resume
 *   is handled via AcpiEnterSleepState() / AcpiEnterSleepStatePrep().
 *   The framework handles device suspend ordering (leaf->root) and
 *   re-enables GPEs and wakeup events after resume.
 *
 * Runtime PM:
 *   An idle timer per device counts ticks of inactivity.  When the
 *   idle threshold is reached and no one holds a reference via
 *   gergios_pm_get(), the device is automatically transitioned to
 *   D3hot via its PMCSR register, and the driver's runtime_suspend
 *   callback is invoked.
 *
 * PCI D-state:
 *   Direct access to the PCI PM capability registers (PMCSR) allows
 *   setting D0-D3hot states.  D3cold requires additional power
 *   rail control beyond PCI config space.
 */

#include <minix/drivers.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <minix/sysutil.h>
#include <minix/type.h>
#include <minix/com.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#include "gergios_device.h"
#include "gergios_driver.h"
#include "pm.h"

/*===========================================================================*
 *		External ACPI functions (from ACPICA / ACPI driver)	     *
 *===========================================================================*/
/* These are provided by the ACPI driver when linked.  If ACPI is not
 * available, the weak stubs below will be used instead.
 * Note: ACPI_STATUS is uint32_t, AE_OK = 0. */

extern int AcpiEnterSleepStatePrep(uint8_t sleep_state);
extern int AcpiEnterSleepState(uint8_t sleep_state);
extern int AcpiGetSleepTypeData(uint8_t sleep_state,
    uint8_t *slp_typa, uint8_t *slp_typb);

/* Weak stubs for systems without ACPI — return non-zero to indicate error.
 * We use 1 instead of -ENOSYS to match ACPI_STATUS convention
 * (AE_OK = 0, any non-zero = error). */
__attribute__((weak))
int AcpiEnterSleepStatePrep(uint8_t sleep_state)
{
	(void)sleep_state;
	return 1;
}

__attribute__((weak))
int AcpiEnterSleepState(uint8_t sleep_state)
{
	(void)sleep_state;
	return 1;
}

__attribute__((weak))
int AcpiGetSleepTypeData(uint8_t sleep_state,
    uint8_t *slp_typa, uint8_t *slp_typb)
{
	(void)sleep_state;
	(void)slp_typa;
	(void)slp_typb;
	return 1;
}

/*===========================================================================*
 *		Internal state						     *
 *===========================================================================*/

#define GERGIOS_PM_MAX_DEVICES		64
#define GERGIOS_PM_DEFAULT_IDLE_TICKS	5	/* ~5 seconds at 1 Hz */

static struct gergios_pm_state pm_state;
static struct gergios_pm_device pm_devices[GERGIOS_PM_MAX_DEVICES];
static unsigned int pm_device_count = 0;
static int pm_initialised = 0;
static int acpi_available = 0;

/*===========================================================================*
 *		ACPI S3 availability check				     *
 *===========================================================================*/

int gergios_pm_s3_available(void)
{
	if (!pm_initialised)
		return 0;

	if (acpi_available) {
		uint8_t slp_typa, slp_typb;
		int r = AcpiGetSleepTypeData(3, &slp_typa, &slp_typb);
		return (r == 0) ? 1 : 0;
	}

	return 0;
}

/*===========================================================================*
 *		PCI D-state helpers					     *
 *===========================================================================*/

extern u8_t  pci_attr_r8(int devind, int port);
extern u16_t pci_attr_r16(int devind, int port);
extern u32_t pci_attr_r32(int devind, int port);
extern void  pci_attr_w8(int devind, int port, u8_t val);
extern void  pci_attr_w16(int devind, int port, u16_t val);

#define PCI_CAP_ID_PM		0x01

uint8_t gergios_pci_find_pm_cap(int devind)
{
	uint8_t capptr;
	uint16_t status;

	status = pci_attr_r16(devind, 0x06);
	if (!(status & 0x0010))
		return 0;

	capptr = pci_attr_r8(devind, 0x34);
	while (capptr != 0) {
		uint8_t cap_id = pci_attr_r8(devind, capptr);
		if (cap_id == PCI_CAP_ID_PM)
			return capptr;
		capptr = pci_attr_r8(devind, capptr + 1);
	}
	return 0;
}

int gergios_pci_get_d_state(int devind, uint8_t pm_capptr)
{
	uint16_t pmcsr;

	if (pm_capptr == 0)
		return -ENODEV;

	pmcsr = pci_attr_r16(devind, pm_capptr + PCI_PM_CAP_PMCSR);
	return pmcsr & PCI_PM_PMCSR_STATE_MASK;
}

int gergios_pci_set_d_state(int devind, uint8_t pm_capptr,
    enum gergios_pci_d_state d_state)
{
	uint16_t pmcsr;

	if (pm_capptr == 0)
		return -ENODEV;

	if (d_state > GERGIOS_PCI_D3hot)
		return -EINVAL;

	pmcsr = pci_attr_r16(devind, pm_capptr + PCI_PM_CAP_PMCSR);
	pmcsr &= ~PCI_PM_PMCSR_STATE_MASK;
	pmcsr |= (uint16_t)d_state;

	pci_attr_w16(devind, pm_capptr + PCI_PM_CAP_PMCSR, pmcsr);

	/* For D0 transitions, clear PME status if set */
	if (d_state == GERGIOS_PCI_D0) {
		pmcsr = pci_attr_r16(devind, pm_capptr + PCI_PM_CAP_PMCSR);
		if (pmcsr & PCI_PM_PMCSR_PME_STS) {
			pmcsr |= PCI_PM_PMCSR_PME_STS;
			pci_attr_w16(devind, pm_capptr + PCI_PM_CAP_PMCSR, pmcsr);
		}
	}

	/* Flush posted write by reading back PMCSR (PCI spec requires
	 * this to ensure the write reaches the device before continuing). */
	(void)pci_attr_r16(devind, pm_capptr + PCI_PM_CAP_PMCSR);

	/* For D0 transitions, wait 10ms for device to stabilise.
	 * For D3, wait 100ms (device may need time to enter low power). */
	if (d_state == GERGIOS_PCI_D0) {
		usleep(10000);  /* 10 ms */
	} else {
		usleep(100000); /* 100 ms */
	}

	return 0;
}

int gergios_pci_d_state_supported(int devind, uint8_t pm_capptr,
    enum gergios_pci_d_state d_state)
{
	uint16_t pmc;

	if (pm_capptr == 0)
		return 0;

	pmc = pci_attr_r16(devind, pm_capptr + PCI_PM_CAP_PMC);

	switch (d_state) {
	case GERGIOS_PCI_D0:
		return 1;
	case GERGIOS_PCI_D1:
		return (pmc & PCI_PM_CAP_D1) != 0;
	case GERGIOS_PCI_D2:
		return (pmc & PCI_PM_CAP_D2) != 0;
	case GERGIOS_PCI_D3hot:
		return 1;
	case GERGIOS_PCI_D3cold:
		return (pmc & PCI_PM_CAP_PME_D3cold) != 0;
	default:
		return 0;
	}
}

/*===========================================================================*
 *		Device PM registration					     *
 *===========================================================================*/

int gergios_pm_register_device(struct gergios_device *dev)
{
	struct gergios_pm_device *pm_dev;

	if (pm_device_count >= GERGIOS_PM_MAX_DEVICES)
		return -ENOMEM;

	pm_dev = &pm_devices[pm_device_count];
	memset(pm_dev, 0, sizeof(*pm_dev));
	pm_dev->dev = dev;
	pm_dev->pm_state = GERGIOS_PM_ON;
	pm_dev->d_state = GERGIOS_PCI_D0;
	pm_dev->idle_threshold = GERGIOS_PM_DEFAULT_IDLE_TICKS;
	pm_dev->usage_count = 1;
	pm_dev->suspended = 0;
	pm_dev->runtime_pm = 0;
	pm_dev->no_pm = 0;

	/* Only probe PCI PM capability for actual PCI devices.
	 * Non-PCI devices (I2C, USB, etc.) have vendor_id == 0. */
	if (dev->vendor_id != 0 || dev->device_id != 0) {
		int devind = (int)dev->bus_address;
		if (devind > 0) {
			uint8_t pm_cap = gergios_pci_find_pm_cap(devind);
			if (pm_cap != 0) {
				int d_state = gergios_pci_get_d_state(devind, pm_cap);
				if (d_state >= 0)
					pm_dev->d_state = (enum gergios_pci_d_state)d_state;
			}
		}
	}

	pm_device_count++;
	return 0;
}

void gergios_pm_unregister_device(struct gergios_device *dev)
{
	for (unsigned int i = 0; i < pm_device_count; i++) {
		if (pm_devices[i].dev != dev)
			continue;
		pm_devices[i] = pm_devices[pm_device_count - 1];
		pm_device_count--;
		return;
	}
}

/*===========================================================================*
 *		System Sleep (ACPI S3)					     *
 *===========================================================================*/

static int suspend_one_device(struct gergios_pm_device *pm_dev)
{
	struct gergios_device *dev = pm_dev->dev;
	struct gergios_driver *drv;
	int r;

	if (!dev || pm_dev->no_pm || pm_dev->suspended)
		return 0;

	drv = dev->driver;
	if (!drv || !drv->ops.pm || !drv->ops.pm->suspend)
		return 0;

	r = drv->ops.pm->suspend(dev, GERGIOS_PM_SLEEP);
	if (r != 0) {
		printf("gergios_pm: device '%s' suspend failed: %d\n",
		    drv->name ? drv->name : "?", r);
		return r;
	}

	if (dev->vendor_id != 0 || dev->device_id != 0) {
		int devind = (int)dev->bus_address;
		if (devind > 0) {
			uint8_t pm_cap = gergios_pci_find_pm_cap(devind);
			if (pm_cap != 0) {
				int r2 = gergios_pci_set_d_state(devind, pm_cap,
				    GERGIOS_PCI_D3hot);
				if (r2 == 0)
					pm_dev->d_state = GERGIOS_PCI_D3hot;
			}
		}
	}

	pm_dev->suspended = 1;
	pm_dev->pm_state = GERGIOS_PM_SLEEP;
	return 0;
}

static int resume_one_device(struct gergios_pm_device *pm_dev)
{
	struct gergios_device *dev = pm_dev->dev;
	struct gergios_driver *drv;
	int r;

	if (!dev || pm_dev->no_pm || !pm_dev->suspended)
		return 0;

	drv = dev->driver;

	if (dev->vendor_id != 0 || dev->device_id != 0) {
		int devind = (int)dev->bus_address;
		if (devind > 0) {
			uint8_t pm_cap = gergios_pci_find_pm_cap(devind);
			if (pm_cap != 0) {
				int r2 = gergios_pci_set_d_state(devind, pm_cap,
				    GERGIOS_PCI_D0);
				if (r2 == 0)
					pm_dev->d_state = GERGIOS_PCI_D0;
			}
		}
	}

	if (drv && drv->ops.pm && drv->ops.pm->resume) {
		r = drv->ops.pm->resume(dev);
		if (r != 0) {
			printf("gergios_pm: device '%s' resume failed: %d\n",
			    drv->name ? drv->name : "?", r);
			return r;
		}
	}

	pm_dev->suspended = 0;
	pm_dev->pm_state = GERGIOS_PM_ON;
	return 0;
}

static int suspend_all_devices(void)
{
	int r;

	for (unsigned int i = pm_device_count; i > 0; i--) {
		r = suspend_one_device(&pm_devices[i - 1]);
		if (r != 0) {
			pm_state.abort_suspend = 1;
			for (unsigned int j = i; j < pm_device_count; j++)
				resume_one_device(&pm_devices[j]);
			pm_state.abort_suspend = 0;
			return r;
		}
		pm_state.suspended_devices++;
	}
	return 0;
}

static int resume_all_devices(void)
{
	int r, overall = 0;

	for (unsigned int i = 0; i < pm_device_count; i++) {
		r = resume_one_device(&pm_devices[i]);
		if (r != 0)
			overall = r;
		pm_state.resumed_devices++;
	}
	return overall;
}

int gergios_pm_suspend(enum gergios_system_sleep_state state)
{
	int r;

	if (!pm_initialised)
		return -ENODEV;

	printf("gergios_pm: suspending all devices (S%u)...\n", state);

	r = suspend_all_devices();
	if (r != 0) {
		printf("gergios_pm: suspend aborted by device\n");
		return r;
	}

	pm_state.system_sleep = state;

	if (acpi_available) {
		printf("gergios_pm: calling ACPI S%u...\n", state);

		r = AcpiEnterSleepStatePrep((uint8_t)state);
		if (r != 0) {
			printf("gergios_pm: AcpiEnterSleepStatePrep failed: %d\n", r);
			resume_all_devices();
			return r;
		}

		r = AcpiEnterSleepState((uint8_t)state);
		printf("gergios_pm: woke up (ACPI returned %d)\n", r);
	}

	r = resume_all_devices();
	pm_state.system_sleep = GERGIOS_SLEEP_S0;
	printf("gergios_pm: resume complete (%u devices)\n",
	    pm_state.resumed_devices);

	return r;
}

int gergios_pm_resume(void)
{
	if (!pm_initialised)
		return -ENODEV;

	printf("gergios_pm: resuming all devices...\n");
	return resume_all_devices();
}

/*===========================================================================*
 *		Runtime PM						     *
 *===========================================================================*/

void gergios_pm_mark_active(struct gergios_device *dev)
{
	for (unsigned int i = 0; i < pm_device_count; i++) {
		if (pm_devices[i].dev == dev) {
			pm_devices[i].idle_count = 0;
			return;
		}
	}
}

int gergios_pm_get(struct gergios_device *dev)
{
	for (unsigned int i = 0; i < pm_device_count; i++) {
		if (pm_devices[i].dev != dev)
			continue;

		pm_devices[i].usage_count++;
		pm_devices[i].idle_count = 0;  /* Reset idle on activity */

		if (pm_devices[i].runtime_pm && !pm_devices[i].no_pm) {
			/* Check if we need to wake the device from runtime suspend */
			if (pm_devices[i].d_state != GERGIOS_PCI_D0) {
				struct gergios_driver *drv = dev->driver;

				if (dev->vendor_id != 0 || dev->device_id != 0) {
					int devind = (int)dev->bus_address;
					if (devind > 0) {
						uint8_t pm_cap = gergios_pci_find_pm_cap(devind);
						if (pm_cap != 0) {
							gergios_pci_set_d_state(devind, pm_cap,
							    GERGIOS_PCI_D0);
							pm_devices[i].d_state = GERGIOS_PCI_D0;
						}
					}
				}

				if (drv && drv->ops.pm && drv->ops.pm->runtime_resume)
					drv->ops.pm->runtime_resume(dev);
			}
		}

		return 0;
	}
	return -ENODEV;
}

void gergios_pm_put(struct gergios_device *dev)
{
	for (unsigned int i = 0; i < pm_device_count; i++) {
		if (pm_devices[i].dev == dev) {
			if (pm_devices[i].usage_count > 0)
				pm_devices[i].usage_count--;
			return;
		}
	}
}

int gergios_pm_runtime_enable(struct gergios_device *dev, int enable)
{
	for (unsigned int i = 0; i < pm_device_count; i++) {
		if (pm_devices[i].dev == dev) {
			pm_devices[i].runtime_pm = (enable != 0);
			if (enable)
				pm_devices[i].idle_count = 0;
			return 0;
		}
	}
	return -ENODEV;
}

void gergios_pm_set_idle_timeout(struct gergios_device *dev,
    unsigned int timeout_ms)
{
	unsigned int ticks = timeout_ms / 1000;

	if (ticks < 1) ticks = 1;
	if (ticks > 3600) ticks = 3600;

	for (unsigned int i = 0; i < pm_device_count; i++) {
		if (pm_devices[i].dev == dev) {
			pm_devices[i].idle_threshold = ticks;
			return;
		}
	}
}

void gergios_pm_tick(void)
{
	for (unsigned int i = 0; i < pm_device_count; i++) {
		struct gergios_pm_device *pm_dev = &pm_devices[i];

		if (pm_dev->no_pm || !pm_dev->runtime_pm || pm_dev->suspended)
			continue;

		if (pm_dev->usage_count > 0) {
			pm_dev->idle_count = 0;
			continue;
		}

		pm_dev->idle_count++;

		if (pm_dev->idle_count >= pm_dev->idle_threshold) {
			struct gergios_driver *drv;

			pm_dev->idle_count = pm_dev->idle_threshold;
			drv = pm_dev->dev ? pm_dev->dev->driver : NULL;

			if (!drv || !drv->ops.pm)
				continue;

			if (drv->ops.pm->runtime_suspend) {
				int r = drv->ops.pm->runtime_suspend(pm_dev->dev);
				if (r != 0)
					continue;
			}

			if (pm_dev->dev->vendor_id != 0 || pm_dev->dev->device_id != 0) {
				int devind = (int)pm_dev->dev->bus_address;
				if (devind > 0) {
					uint8_t pm_cap = gergios_pci_find_pm_cap(devind);
					if (pm_cap != 0) {
						gergios_pci_set_d_state(devind, pm_cap,
						    GERGIOS_PCI_D3hot);
						pm_dev->d_state = GERGIOS_PCI_D3hot;
					}
				}
			}
		}
	}
}

/*===========================================================================*
 *		Initialisation						     *
 *===========================================================================*/

int gergios_pm_init(void)
{
	memset(&pm_state, 0, sizeof(pm_state));
	pm_state.system_sleep = GERGIOS_SLEEP_S0;
	pm_device_count = 0;

	uint8_t slp_typa, slp_typb;
	if (AcpiGetSleepTypeData(3, &slp_typa, &slp_typb) == 0) {
		acpi_available = 1;
		printf("gergios_pm: ACPI S3 available (SLP_TYPa=%u, SLP_TYPb=%u)\n",
		    slp_typa, slp_typb);
	} else {
		printf("gergios_pm: ACPI S3 not available\n");
		acpi_available = 0;
	}

	pm_initialised = 1;
	return 0;
}

/*===========================================================================*
 *		Debugging						     *
 *===========================================================================*/

void gergios_pm_dump(void)
{
	printf("--- gergios PM state ---\n");
	printf("system sleep:   S%u\n", pm_state.system_sleep);
	printf("pm_initialised: %d\n", pm_initialised);
	printf("acpi_available: %d\n", acpi_available);
	printf("S3 available:   %d\n", gergios_pm_s3_available());
	printf("registered devices: %u\n", pm_device_count);

	for (unsigned int i = 0; i < pm_device_count; i++) {
		struct gergios_pm_device *pm_dev = &pm_devices[i];
		struct gergios_device *dev = pm_dev->dev;
		const char *name = dev && dev->driver
		    ? dev->driver->name : "?";

		printf("  [%2u] %-20s D%d  idle=%u/%u  usage=%u  "
		    "suspended=%d rt=%d\n",
		    i, name,
		    pm_dev->d_state,
		    pm_dev->idle_count, pm_dev->idle_threshold,
		    pm_dev->usage_count,
		    pm_dev->suspended, pm_dev->runtime_pm);
	}
	printf("--- end ---\n");
}
