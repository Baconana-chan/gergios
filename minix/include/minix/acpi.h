#include <sys/types.h>
#include <minix/ipc.h>

/* Forward declaration for IPC structs that reference ACPI_HANDLE.
 * ACPI_HANDLE is defined in acpi.h which is only available when
 * ACPICA is linked.  For users of libgergios_driver, this definition
 * serves as an opaque pointer type. */
#ifndef _ACPI_HANDLE_DEFINED
#define _ACPI_HANDLE_DEFINED
typedef void *ACPI_HANDLE;
#endif

#define ACPI_REQ_GET_IRQ			1
#define ACPI_REQ_MAP_BRIDGE			2
#define ACPI_REQ_POWER_ON			3
#define ACPI_REQ_POWER_OFF			4
#define ACPI_REQ_GET_SLEEP_STATES		5
#define ACPI_REQ_ENUM_DEVICES			6
#define ACPI_REQ_WAKEUP_ENABLE			7
#define ACPI_REQ_WAKEUP_DISABLE			8
#define ACPI_REQ_WAKEUP_GPE			9

struct acpi_request_hdr {
	endpoint_t 	m_source; /* message header */
	u32_t		request;
};

/*
 * Message to request dev/pin translation to IRQ by acpi using the acpi routing
 * tables
 */
struct acpi_get_irq_req {
	struct acpi_request_hdr	hdr;
	u32_t			bus;
	u32_t			dev;
	u32_t			pin;
	u32_t			__padding[4];
};

/* response from acpi to acpi_get_irq_req */
struct acpi_get_irq_resp {
	endpoint_t 	m_source; /* message header */
	i32_t		irq;
	u32_t		__padding[7];
};

/* message format for pci bridge mappings to acpi */
struct acpi_map_bridge_req {
	struct acpi_request_hdr	hdr;
	u32_t	primary_bus;
	u32_t	secondary_bus;
	u32_t	device;
	u32_t	__padding[4];
};

struct acpi_map_bridge_resp {
	endpoint_t 	m_source; /* message header */
	int		err;
	u32_t		__padding[7];
};

/*
 * ACPI_REQ_POWER_ON / ACPI_REQ_POWER_OFF — control device D-state
 * device_handle: ACPI handle of the target device (from AcpiGetHandle)
 */
struct acpi_power_req {
	struct acpi_request_hdr	hdr;
	ACPI_HANDLE		device_handle;
	u32_t			__padding[6];
};

struct acpi_power_resp {
	endpoint_t	m_source;
	int		err;
	u32_t		__padding[7];
};

/*
 * ACPI_REQ_GET_SLEEP_STATES — query supported S-states
 * supported[i] = 1 if S<i> is supported (i = 0..4)
 */
struct acpi_sleep_states_resp {
	endpoint_t	m_source;
	int		supported[5];
	u32_t		__padding[3];
};

/*
 * ACPI_REQ_ENUM_DEVICES — paginated query of enumerated device list.
 * The caller sets offset (starting index) and receives up to 3 entries.
 * Makes multiple calls to iterate the full list.
 */
#define ACPI_ENUM_PAGE_SIZE	3

struct acpi_enum_req {
	struct acpi_request_hdr	hdr;
	u32_t			offset;	/* start index in device table */
	u32_t			__padding[6];
};

struct acpi_enum_entry {
	u32_t			handle;
	u32_t			bus_dev_func;	/* packed: bus[15:0] | dev[7:0] | func[7:0] */
	u16_t			flags;		/* bit 0: is_pci_root */
	u16_t			status;		/* _STA value */
};

struct acpi_enum_resp {
	endpoint_t		m_source;
	int			count;		/* total discovered devices */
	int			returned;	/* entries in this response (0 if done) */
	struct acpi_enum_entry	entries[ACPI_ENUM_PAGE_SIZE];
};

int acpi_init(void);
int acpi_get_irq(unsigned bus, unsigned dev, unsigned pin);
void acpi_map_bridge(unsigned int pbnr, unsigned int dev, unsigned int sbnr);

/*
 * ACPI_REQ_WAKEUP_ENABLE / ACPI_REQ_WAKEUP_DISABLE —
 * Configure the ACPI GPE wake mask for a device.
 * The ACPI driver queries _PRW under the device, finds the wake GPE,
 * and calls AcpiSetGpeWakeMask() to enable/disable it.
 * device_handle: ACPI handle of the target device (from AcpiGetHandle).
 */
struct acpi_wakeup_req {
	struct acpi_request_hdr	hdr;
	ACPI_HANDLE		device_handle;
	u32_t			__padding[6];
};

struct acpi_wakeup_resp {
	endpoint_t	m_source;
	int		err;
	u32_t		__padding[7];
};

/*
 * ACPI_REQ_WAKEUP_GPE —
 * Query the _PRW GPE number associated with a device.
 * Returns the GPE number, or -1 if no _PRW found.
 */
struct acpi_wakeup_gpe_req {
	struct acpi_request_hdr	hdr;
	ACPI_HANDLE		device_handle;
	u32_t			__padding[6];
};

struct acpi_wakeup_gpe_resp {
	endpoint_t	m_source;
	int		gpe_number;
	u32_t		__padding[7];
};

/*
 * Power management helpers — call _PS0 / _PS3 on an ACPI device.
 * Returns 0 on success, -1 if _PSx method failed with error.
 */
int acpi_power_on_device(ACPI_HANDLE device);
int acpi_power_off_device(ACPI_HANDLE device);
