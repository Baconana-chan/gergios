#ifndef __ACPI_ENUMERATE_H__
#define __ACPI_ENUMERATE_H__

#include <minix/ipc.h>
#include <acpi.h>

/* Maximum number of discovered ACPI devices */
#define ACPI_ENUM_DEVICES_MAX 128

/* ACPI device entry — compact struct for post-enum storage */
struct acpi_enum_device {
	ACPI_HANDLE		handle;		/* ACPI namespace handle */
	u32_t			address;	/* _ADR value */
	char			hid[16];	/* _HID string */
	char			uid[16];	/* _UID string */
	u16_t			bus;		/* PCI bus number */
	u16_t			device;		/* PCI device (from _ADR >> 16) */
	u16_t			func;		/* PCI function (from _ADR & 0xFFFF) */
	u16_t			sta;		/* _STA cached value */
	u8_t			is_pci_root;	/* 1 if PNP0A03/PNP0A08 */
	u8_t			valid;		/* ACPI_VALID_* bitmask */
};

/* Discovered device table */
struct acpi_enum_state {
	struct acpi_enum_device	devices[ACPI_ENUM_DEVICES_MAX];
	int			count;
};

/*
 * Walk the entire ACPI namespace and discover all Device() nodes.
 * Returns 0 on success, negative on error.
 */
int acpi_enumerate_devices(void);

/*
 * Get the enumerated device table.
 */
struct acpi_enum_state *acpi_get_enum_state(void);

/*
 * Print discovered devices to console (for debugging).
 */
void acpi_enum_dump(void);

/*
 * IPC handler: ACPI_REQ_ENUM_DEVICES — paginated device list query
 */
void do_enum_devices(message *m);

#endif /* __ACPI_ENUMERATE_H__ */
