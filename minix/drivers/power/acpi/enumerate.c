#include <stdio.h>
#include <string.h>
#include <assert.h>

#include <acpi.h>
#include <accommon.h>
#include "enumerate.h"
#include "pci.h"

/* Internal enumeration state */
static struct acpi_enum_state enum_state;

/* Bus number stack for nested PCI bridges (depth <= 16) */
#define ACPI_ENUM_BUS_STACK_MAX 16
static int enum_bus_stack[ACPI_ENUM_BUS_STACK_MAX];
static int enum_bus_depth = 0;

/* Save current bus number before entering a bridge subtree */
static void enum_bus_push(int bus)
{
	if (enum_bus_depth < ACPI_ENUM_BUS_STACK_MAX)
		enum_bus_stack[enum_bus_depth++] = bus;
}

/* Restore bus number after leaving a bridge subtree */
static int enum_bus_pop(void)
{
	if (enum_bus_depth > 0)
		return enum_bus_stack[--enum_bus_depth];
	return 0;
}

/* Peek at current bus number */
static int enum_bus_peek(void)
{
	if (enum_bus_depth > 0)
		return enum_bus_stack[enum_bus_depth - 1];
	return 0;
}

/*
 * PNP ID → PCI root bridge detection.
 */
static int is_pci_root_hid(const char *hid)
{
	if (!hid)
		return 0;
	return (strcmp(hid, "PNP0A03") == 0 || strcmp(hid, "PNP0A08") == 0);
}

/*
 * Try to read _BBN (Base Bus Number) from a device node.
 */
static int get_bbn(ACPI_HANDLE handle)
{
	ACPI_STATUS status;
	ACPI_BUFFER buf;
	ACPI_OBJECT obj;
	int bbn = 0;

	buf.Length = sizeof(obj);
	buf.Pointer = &obj;

	status = AcpiEvaluateObjectTyped(handle, (ACPI_STRING)"_BBN",
	    NULL, &buf, ACPI_TYPE_INTEGER);
	if (ACPI_SUCCESS(status))
		bbn = (int)obj.Integer.Value;

	return bbn;
}

/*
 * Try to read _CRS and parse Secondary Bus Number from PCI bridge resources.
 */
static int get_secondary_bus_from_crs(ACPI_HANDLE handle)
{
	ACPI_STATUS status;
	ACPI_BUFFER buf;
	u8_t crs_buf[512];
	ACPI_RESOURCE *res;

	buf.Length = sizeof(crs_buf);
	buf.Pointer = crs_buf;

	status = AcpiGetCurrentResources(handle, &buf);
	if (ACPI_FAILURE(status))
		return -1;

	res = (ACPI_RESOURCE *)crs_buf;
	while (res->Type != ACPI_RESOURCE_TYPE_END_TAG) {
		if (res->Type == ACPI_RESOURCE_TYPE_BUS_NUMBER_RANGE) {
			/* Secondary bus = _CRS BusNumberRange.Min */
			return (int)res->Data.BusNumberRange.Min;
		}
		res = ACPI_NEXT_RESOURCE(res);
	}

	return -1;
}

/*
 * Check if a device is a PCI bridge (has _CRS with bus number range).
 * If so, push current bus and set new bus from _CRS secondary bus.
 * Returns 1 if it's a PCI bridge, 0 otherwise.
 */
static int enum_enter_pci_bridge(ACPI_HANDLE handle, const char *hid,
    u32_t valid)
{
	int sec_bus;

	/* PCI-PCI bridges have _HID "PNP0A03" or similar */
	if (!(valid & ACPI_VALID_HID))
		return 0;
	if (strcmp(hid, "PNP0A03") != 0 && strcmp(hid, "PNP0A08") != 0)
		return 0;

	sec_bus = get_secondary_bus_from_crs(handle);
	if (sec_bus < 0)
		return 0;

	/* Push current bus and set new secondary bus */
	enum_bus_push(sec_bus);
	return 1;
}

/* Restore bus after leaving a PCI bridge subtree */
static void enum_leave_pci_bridge(void)
{
	enum_bus_pop();
}

/*
 * ACPI namespace walk callback — called for each Device() node.
 */
static ACPI_STATUS acpi_enum_walk_cb(
    ACPI_HANDLE		handle,
    UINT32			nesting_level,
    void			*context,
    void			**return_value)
{
	ACPI_DEVICE_INFO	*info = NULL;
	ACPI_STATUS		status;
	struct acpi_enum_device	*dev;
	int			idx;

	(void)nesting_level;
	(void)context;
	(void)return_value;

	/* Check device limit */
	idx = enum_state.count;
	if (idx >= ACPI_ENUM_DEVICES_MAX)
		return AE_OK;

	/* Get object info (_HID, _ADR, _STA, etc.) */
	status = AcpiGetObjectInfo(handle, &info);
	if (ACPI_FAILURE(status) || !info)
		return AE_OK;

	/* Skip non-Device nodes */
	if (info->Type != ACPI_TYPE_DEVICE) {
		ACPI_FREE(info);
		return AE_OK;
	}

	/* Skip devices not present (_STA bit 0) */
	if ((info->Valid & ACPI_VALID_STA) &&
	    !(info->CurrentStatus & ACPI_STA_DEVICE_PRESENT)) {
		ACPI_FREE(info);
		return AE_OK;
	}

	/* Enter PCI bridge subtree if applicable */
	if (info->Valid & ACPI_VALID_HID)
		enum_enter_pci_bridge(handle, info->HardwareId.String, info->Valid);

	/* Fill device entry with compact fields (no full ACPI_DEVICE_INFO stored) */
	memset(&enum_state.devices[idx], 0, sizeof(struct acpi_enum_device));
	dev = &enum_state.devices[idx];

	dev->handle = handle;
	dev->address = (info->Valid & ACPI_VALID_ADR) ? (u32_t)info->Address : 0;
	dev->valid = info->Valid;
	dev->sta = info->CurrentStatus;
	dev->bus = (u16_t)enum_bus_peek();
	dev->device = (u16_t)(info->Address >> 16);
	dev->func   = (u16_t)(info->Address & 0xFFFF);

	/* Copy _HID string */
	if (info->Valid & ACPI_VALID_HID && info->HardwareId.String)
		strncpy(dev->hid, info->HardwareId.String, sizeof(dev->hid) - 1);

	/* Copy _UID string */
	if (info->Valid & ACPI_VALID_UID && info->UniqueId.String)
		strncpy(dev->uid, info->UniqueId.String, sizeof(dev->uid) - 1);

	/* Detect PCI root bridge */
	if ((info->Valid & ACPI_VALID_HID) &&
	    is_pci_root_hid(info->HardwareId.String)) {
		int bbn = get_bbn(handle);
		dev->is_pci_root = 1;
		dev->bus = (u16_t)bbn;

		/* Push root bus onto the bus stack */
		enum_bus_push(bbn);
	}

	/* Free ACPI_DEVICE_INFO — all needed data is now in compact struct */
	ACPI_FREE(info);

	enum_state.count++;

	return AE_OK;
}

/*
 * Ascending callback — called when leaving a namespace subtree.
 * Pops bus stack for PCI bridges.
 */
static ACPI_STATUS acpi_enum_ascend_cb(
    ACPI_HANDLE		handle,
    UINT32			nesting_level,
    void			*context,
    void			**return_value)
{
	ACPI_DEVICE_INFO	*info = NULL;

	(void)nesting_level;
	(void)context;
	(void)return_value;

	if (ACPI_FAILURE(AcpiGetObjectInfo(handle, &info)) || !info)
		return AE_OK;

	if (info->Type == ACPI_TYPE_DEVICE &&
	    (info->Valid & ACPI_VALID_HID)) {
		/* Pop bus stack when leaving a PCI bridge */
		enum_leave_pci_bridge();
	}

	if (info)
		ACPI_FREE(info);

	return AE_OK;
}

int acpi_enumerate_devices(void)
{
	ACPI_STATUS status;
	int i;

	/* Reset state */
	enum_state.count = 0;
	enum_bus_depth = 0;
	memset(&enum_state.devices, 0, sizeof(enum_state.devices));

	/* Walk the entire ACPI namespace for Device() nodes */
	status = AcpiWalkNamespace(ACPI_TYPE_DEVICE, ACPI_ROOT_OBJECT,
	    ACPI_UINT32_MAX, acpi_enum_walk_cb, acpi_enum_ascend_cb,
	    NULL, NULL);

	if (ACPI_FAILURE(status)) {
		printf("ACPI: enum walk failed: %d\n", status);
		return -1;
	}

	printf("ACPI: discovered %d devices\n", enum_state.count);

	/* Print summary */
	for (i = 0; i < enum_state.count; i++) {
		struct acpi_enum_device *dev = &enum_state.devices[i];
		const char *hid = dev->hid[0] ? dev->hid : "(no HID)";
		const char *uid = dev->uid[0] ? dev->uid : "";

		printf("ACPI:   [%2d]", i);
		if (dev->is_pci_root)
			printf(" PCI_ROOT bus=%d", dev->bus);
		else if (dev->valid & ACPI_VALID_ADR)
			printf(" PCI %02x:%02x.%x", dev->bus, dev->device, dev->func);
		printf(" HID=%s", hid);
		if (uid[0])
			printf(" UID=%s", uid);
		printf(" STA=0x%x\n", dev->sta);
	}

	return 0;
}

struct acpi_enum_state *acpi_get_enum_state(void)
{
	return &enum_state;
}

void acpi_enum_dump(void)
{
	int i;

	printf("ACPI enumerated devices (%d):\n", enum_state.count);
	for (i = 0; i < enum_state.count; i++) {
		struct acpi_enum_device *dev = &enum_state.devices[i];

		printf("  [%2d] handle=%p bus=%02x dev=%02x func=%x "
		    "ADR=0x%x HID=%s UID=%s STA=0x%x\n",
		    i, (void*)dev->handle, dev->bus, dev->device, dev->func,
		    dev->address, dev->hid, dev->uid, dev->sta);
	}
}

/*
 * IPC handler: ACPI_REQ_ENUM_DEVICES (paginated).
 * Caller sets offset in req, we return up to ACPI_ENUM_PAGE_SIZE entries.
 */
void do_enum_devices(message *m)
{
	struct acpi_enum_req *req = (struct acpi_enum_req *)m;
	struct acpi_enum_resp *resp = (struct acpi_enum_resp *)m;
	int start = (int)req->offset;
	int i;

	resp->count = enum_state.count;

	if (start >= enum_state.count || start < 0) {
		resp->returned = 0;
		return;
	}

	/* Copy up to ACPI_ENUM_PAGE_SIZE entries starting from offset */
	for (i = 0; i < ACPI_ENUM_PAGE_SIZE && (start + i) < enum_state.count; i++) {
		struct acpi_enum_device *src = &enum_state.devices[start + i];

		resp->entries[i].handle = (u32_t)(uintptr_t)src->handle;
		resp->entries[i].bus_dev_func =
		    ((u32_t)src->bus << 16) | ((u32_t)src->device << 8) | src->func;
		resp->entries[i].flags = src->is_pci_root ? 1 : 0;
		resp->entries[i].status = src->sta;
	}

	resp->returned = i;
}
