#include <stdio.h>
#include <acpi.h>
#include "gpe.h"

/*
 * Fixed event handler for power button, sleep button, and RTC alarm.
 * ACPICA calls this when a fixed event fires.
 */
UINT32 acpi_fixed_event_handler(void *Context)
{
	ACPI_EVENT_TYPE event = (ACPI_EVENT_TYPE)(uintptr_t)Context;

	switch (event) {
	case ACPI_EVENT_POWER_BUTTON:
		printf("ACPI: power button pressed\n");
		break;
	case ACPI_EVENT_SLEEP_BUTTON:
		printf("ACPI: sleep button pressed\n");
		break;
	case ACPI_EVENT_RTC:
		/* RTC alarm — can be used for wake timer */
		printf("ACPI: RTC alarm\n");
		break;
	default:
		printf("ACPI: fixed event %d\n", event);
		break;
	}

	return ACPI_INTERRUPT_HANDLED;
}

/*
 * Global event handler — called for all GPE and fixed events.
 * ACPI_EVENT_TYPE_GPE (0) or ACPI_EVENT_TYPE_FIXED (1).
 */
void acpi_global_event_handler(UINT32 EventType, ACPI_HANDLE Device,
    UINT32 EventNumber, void *Context)
{
	(void)Context;

	if (EventType == ACPI_EVENT_TYPE_GPE) {
		printf("ACPI: GPE event device=%p num=%u\n",
		    (void *)Device, EventNumber);
	} else if (EventType == ACPI_EVENT_TYPE_FIXED) {
		printf("ACPI: fixed event num=%u\n", EventNumber);
	}
}

/*
 * Initialize GPE subsystem.
 * 
 * 1. Install handlers for fixed events (power button, sleep button)
 * 2. Register a global event handler for debugging/monitoring
 * 3. Enable all runtime GPEs so that ACPICA dispatches them via the SCI
 * 
 * Must be called after AcpiEnableSubsystem().
 */
void acpi_gpe_init(void)
{
	ACPI_STATUS status;
	int i;

	/*
	 * Install fixed event handlers.
	 * ACPICA already handles dispatch internally, but we install
	 * handlers to get notified of events we care about.
	 */
	static const struct {
		UINT32		event;
		const char	*name;
	} fixed_events[] = {
		{ ACPI_EVENT_POWER_BUTTON,	"power button" },
		{ ACPI_EVENT_SLEEP_BUTTON,	"sleep button" },
		{ ACPI_EVENT_RTC,		"RTC" },
	};

	for (i = 0; i < 3; i++) {
		status = AcpiInstallFixedEventHandler(fixed_events[i].event,
		    acpi_fixed_event_handler,
		    (void *)(uintptr_t)fixed_events[i].event);
		if (ACPI_FAILURE(status) && status != AE_ALREADY_EXISTS) {
			printf("ACPI: failed to install %s handler: %d\n",
			    fixed_events[i].name, status);
		}
	}

	/*
	 * Install global event handler — catches all GPEs and fixed events
	 * that are not handled by specific handlers.
	 */
	status = AcpiInstallGlobalEventHandler(
	    acpi_global_event_handler, NULL);
	if (ACPI_FAILURE(status)) {
		printf("ACPI: failed to install global event handler: %d\n",
		    status);
	}

	/*
	 * Enable all runtime GPEs. This allows ACPICA to dispatch GPE
	 * events to registered handlers when the SCI fires.
	 */
	status = AcpiEnableAllRuntimeGpes();
	if (ACPI_FAILURE(status)) {
		printf("ACPI: failed to enable runtime GPEs: %d\n", status);
	} else {
		printf("ACPI: runtime GPEs enabled\n");
	}

	/*
	 * Update all GPEs — ensure ACPICA has synchronized GPE enable masks
	 * with the hardware.
	 */
	AcpiUpdateAllGpes();

	printf("ACPI: GPE subsystem initialized\n");
}
