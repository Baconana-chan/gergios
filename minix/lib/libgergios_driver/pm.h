/* pm.h — Power Management Framework for GergiOS
 *
 * Provides a unified Power Management (PM) framework for GergiOS drivers,
 * covering three major areas:
 *
 * 1. System Sleep (ACPI S3/S4): Coordinated suspend/resume of all devices
 *    during system sleep state transitions.
 * 2. Runtime PM: Per-device automatic power state management based on
 *    idle detection, allowing devices to enter low-power states when
 *    not in use.
 * 3. PCI D-state Management: Direct control of PCI Power Management
 *    Capability registers (PMCSR) for D0-D3hot transitions.
 *
 * Device suspend ordering follows the bus hierarchy:
 *   suspend: children → parents (leaf devices first)
 *   resume:  parents → children (root devices first)
 *
 * Integration with the existing gergios_pm_ops from gergios_driver.h
 * allows drivers to register suspend/resume callbacks.
 */

#ifndef _GERGIOS_PM_H
#define _GERGIOS_PM_H

#include <minix/config.h>
#include <minix/type.h>
#include <minix/endpoint.h>

#include "gergios_driver.h"	/* gergios_device, gergios_pm_ops, gergios_pm_state_t */

/* Forward declarations */
struct gergios_device;

/*===========================================================================*
 *		System sleep states (ACPI S-states)			     *
 *===========================================================================*/
enum gergios_system_sleep_state {
	GERGIOS_SLEEP_S0	= 0,	/* Working (fully on) */
	GERGIOS_SLEEP_S1	= 1,	/* Standby (light sleep, CPU halted) */
	GERGIOS_SLEEP_S2	= 2,	/* Standby (deeper, CPU powered off) */
	GERGIOS_SLEEP_S3	= 3,	/* Suspend-to-RAM (most common) */
	GERGIOS_SLEEP_S4	= 4,	/* Hibernate (suspend-to-disk) */
	GERGIOS_SLEEP_S5	= 5,	/* Soft-off (poweroff) */
};

/*===========================================================================*
 *		PCI Power Management (D-state)				     *
 *===========================================================================*/
enum gergios_pci_d_state {
	GERGIOS_PCI_D0		= 0,	/* Fully on, active */
	GERGIOS_PCI_D1		= 1,	/* Light sleep (optional) */
	GERGIOS_PCI_D2		= 2,	/* Deep sleep (optional) */
	GERGIOS_PCI_D3hot	= 3,	/* Sleep, Vaux maintained */
	GERGIOS_PCI_D3cold	= 4,	/* Power removed (no bus power) */
};

/* PCI PM capability register offsets (from capability pointer) */
#define PCI_PM_CAP_ID		0x00	/* 8-bit: 0x01 = PM capability */
#define PCI_PM_CAP_NEXT		0x01	/* 8-bit: next capability pointer */
#define PCI_PM_CAP_PMC		0x02	/* 16-bit: PM capabilities */
#define 	PCI_PM_CAP_VER_MASK	0x0007	/* PM spec version */
#define 	PCI_PM_CAP_PME_CLOCK	0x0008	/* PME clock required */
#define 	PCI_PM_CAP_DSI		0x0020	/* Device-specific init required */
#define 	PCI_PM_CAP_AUX_CUR_MASK	0x01C0	/* Aux current required */
#define 	PCI_PM_CAP_D1		0x0200	/* D1 state supported */
#define 	PCI_PM_CAP_D2		0x0400	/* D2 state supported */
#define 	PCI_PM_CAP_PME_D0	0x0800	/* PME from D0 */
#define 	PCI_PM_CAP_PME_D1	0x1000	/* PME from D1 */
#define 	PCI_PM_CAP_PME_D2	0x2000	/* PME from D2 */
#define 	PCI_PM_CAP_PME_D3hot	0x4000	/* PME from D3hot */
#define 	PCI_PM_CAP_PME_D3cold	0x8000	/* PME from D3cold */
#define PCI_PM_CAP_PMCSR		0x04	/* 16-bit: PM control/status */
#define 	PCI_PM_PMCSR_STATE_MASK	0x0003	/* Power state (D0-D3hot) */
#define 	PCI_PM_PMCSR_PME_EN	0x0010	/* PME enable */
#define 	PCI_PM_PMCSR_DATA_SEL	0x01E0	/* PME data select */
#define 	PCI_PM_PMCSR_DATA_SCALE	0x0600	/* PME data scale */
#define 	PCI_PM_PMCSR_PME_STS	0x8000	/* PME status */
#define PCI_PM_CAP_PMCSR_BSE	0x06	/* 8-bit: bridge support extensions */
#define PCI_PM_CAP_DATA		0x07	/* 8-bit: PME data register */

/*===========================================================================*
 *		Device PM state tracking				     *
 *===========================================================================*/
struct gergios_pm_device {
	struct gergios_device   *dev;		/* Owning device */
	enum gergios_pci_d_state d_state;	/* Current PCI D-state */
	enum gergios_pm_state_t  pm_state;	/* Current PM state (S0-S2) */
	unsigned int		 idle_count;	/* Idle timer ticks */
	unsigned int		 idle_threshold; /* Ticks before auto-suspend */
	unsigned int		 usage_count;	/* Active usage count */
	unsigned int		 suspended : 1;	/* 1 = device is suspended */
	unsigned int		 wakeup_capable : 1; /* Can wake system */
	unsigned int		 wakeup_enabled : 1; /* Wake enabled */
	unsigned int		 runtime_pm : 1; /* Runtime PM active */
	unsigned int		 no_pm : 1;	/* PM disabled for this device */
};

/*===========================================================================*
 *		System PM state					     *
 *===========================================================================*/
struct gergios_pm_state {
	enum gergios_system_sleep_state system_sleep;	/* Current system state */
	unsigned int			 suspended_devices; /* Count of suspended */
	unsigned int			 resumed_devices;   /* Count of resumed */
	int				 abort_suspend;	    /* Suspend aborted */
};

/*===========================================================================*
 *		Public API						     *
 *===========================================================================*/

/* --- Initialisation ----------------------------------------------------- */

/* Initialise the Power Management framework.
 * Must be called once at system startup.  Detects ACPI capabilities
 * and sets up the device PM tracking list.
 * Returns 0 on success, negative errno on failure. */
int gergios_pm_init(void);

/* Register a device with the PM framework.
 * Called during device probe/init. */
int gergios_pm_register_device(struct gergios_device *dev);

/* Unregister a device from the PM framework.
 * Called during device removal. */
void gergios_pm_unregister_device(struct gergios_device *dev);

/* --- System Sleep (ACPI S3) -------------------------------------------- */

/* Suspend all devices and enter the specified ACPI sleep state.
 * Suspends devices in leaf→root order, then calls ACPI to enter sleep.
 * Returns 0 on success (system resumed), negative errno on failure. */
int gergios_pm_suspend(enum gergios_system_sleep_state state);

/* Resume all devices after a system sleep.
 * Resumes devices in root→leaf order.
 * Returns 0 on success, negative errno on failure. */
int gergios_pm_resume(void);

/* Check if ACPI S3 (suspend-to-RAM) is available on this system. */
int gergios_pm_s3_available(void);

/* --- Runtime PM --------------------------------------------------------- */

/* Notify the PM framework of device activity (resets idle timer).
 * Drivers should call this on every I/O operation. */
void gergios_pm_mark_active(struct gergios_device *dev);

/* Get a device (increment usage count, prevent runtime suspend).
 * Returns 0 on success (device is active), negative if resume needed. */
int gergios_pm_get(struct gergios_device *dev);

/* Release a device (decrement usage count, allow runtime suspend). */
void gergios_pm_put(struct gergios_device *dev);

/* Enable or disable runtime PM for a device.
 * When enabled, the device may be automatically suspended after
 * the idle timeout (default 5 seconds of inactivity). */
int gergios_pm_runtime_enable(struct gergios_device *dev, int enable);

/* Set the idle timeout for automatic runtime suspend (in milliseconds).
 * Default: 5000 ms. */
void gergios_pm_set_idle_timeout(struct gergios_device *dev,
    unsigned int timeout_ms);

/* --- PCI D-state Management --------------------------------------------- */

/* Read the current PCI power state (D0-D3hot) from the PMCSR register.
 * Returns the D-state, or negative errno on failure. */
int gergios_pci_get_d_state(int devind, uint8_t pm_capptr);

/* Set the PCI power state (D0-D3hot) via the PMCSR register.
 * Returns 0 on success, negative errno on failure. */
int gergios_pci_set_d_state(int devind, uint8_t pm_capptr,
    enum gergios_pci_d_state d_state);

/* Find the PCI Power Management capability pointer for a device.
 * Scans the capabilities list for PCI_CAP_ID_PM (0x01).
 * Returns the capability offset, or 0 if not found. */
uint8_t gergios_pci_find_pm_cap(int devind);

/* Check if a PCI device supports a specific D-state (D1, D2).
 * Returns 1 if supported, 0 if not. */
int gergios_pci_d_state_supported(int devind, uint8_t pm_capptr,
    enum gergios_pci_d_state d_state);

/* --- PM Timer / Tick ---------------------------------------------------- */

/* Periodic tick for runtime PM idle detection.
 * Should be called by drivers at ~1 Hz from their alarm handler,
 * or from a central PM timer.  Idle-counting is performed here. */
void gergios_pm_tick(void);

/* --- Debug / Diagnostics ------------------------------------------------ */

/* Print the current PM state of all registered devices (for debugging). */
void gergios_pm_dump(void);

#endif /* _GERGIOS_PM_H */
