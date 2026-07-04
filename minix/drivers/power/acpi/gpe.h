#ifndef __ACPI_GPE_H__
#define __ACPI_GPE_H__

#include <acpi.h>

/*
 * Initialize GPE subsystem — enable all runtime GPEs, install fixed event
 * handlers (power button, sleep button), and optionally register a GPE
 * handler for hot-plug events.
 * Must be called after ACPICA initialization (AcpiEnableSubsystem).
 */
void acpi_gpe_init(void);

/*
 * Fixed event handler — called by ACPICA when a fixed event fires.
 * Defined as ACPI_EVENT_HANDLER type.
 */
UINT32 acpi_fixed_event_handler(void *Context);

/*
 * Global event handler — called for all GPE and fixed events.
 * Defined as ACPI_GBL_EVENT_HANDLER type.
 */
void acpi_global_event_handler(UINT32 EventType, ACPI_HANDLE Device,
    UINT32 EventNumber, void *Context);

#endif /* __ACPI_GPE_H__ */
