#include <stdio.h>
#include <minix/driver.h>
#include <acpi.h>
#include <assert.h>
#include <minix/acpi.h>
#include <minix/syslib.h>
#include <minix/sysutil.h>
#include <minix/com.h>

#include "pci.h"
#include "enumerate.h"
#include "gpe.h"
#include "hotplug.h"

int acpi_enabled;
struct machine machine;

/* SCI notification handler — called from osminixxf.c */
extern void acpi_dispatch_sci(void);

/* Deferred execution queue drain — called from osminixxf.c */
extern int acpi_os_process_exec_queue(void);

/* ACPI sleep states detected from _Sx objects */
static int acpi_sleep_states[5]; /* index 0..4 -> S0..S4 */

/*
 * Query ACPI _Sx (sleep state) objects to determine supported sleep states.
 */
static void acpi_query_sleep_states(void)
{
	static const char *sx_names[5] = { "_S0", "_S1", "_S2", "_S3", "_S4" };
	ACPI_STATUS status;
	ACPI_BUFFER buf;
	char obj_buf[64];
	int i;

	for (i = 0; i < 5; i++) {
		acpi_sleep_states[i] = 0;
		buf.Length = sizeof(obj_buf);
		buf.Pointer = obj_buf;

		status = AcpiEvaluateObject(ACPI_ROOT_OBJECT,
		    (ACPI_STRING)sx_names[i], NULL, &buf);
		if (ACPI_SUCCESS(status))
			acpi_sleep_states[i] = 1;
	}

	printf("ACPI: sleep states:");
	for (i = 0; i < 5; i++) {
		if (acpi_sleep_states[i])
			printf(" S%d", i);
	}
	printf("\n");
}

/*
 * Execute _PS0 (Power State 0 — fully on) on an ACPI device.
 */
static int acpi_set_device_power_state(ACPI_HANDLE device, int state)
{
	ACPI_STATUS status;
	char method[5];

	snprintf(method, sizeof(method), "_PS%d", state);
	status = AcpiEvaluateObject(device, method, NULL, NULL);
	if (ACPI_FAILURE(status) && status != AE_NOT_FOUND)
		return -1;

	return 0;
}

/*
 * Execute _PS0 on a device — wake from D3hot to D0.
 */
int acpi_power_on_device(ACPI_HANDLE device)
{
	return acpi_set_device_power_state(device, 0);
}

/*
 * Execute _PS3 on a device — put to D3hot (power off).
 */
int acpi_power_off_device(ACPI_HANDLE device)
{
	return acpi_set_device_power_state(device, 3);
}

/*
 * IPC handler: ACPI_REQ_POWER_ON — call _PS0 on a device
 */
static void do_power_on(message *m)
{
	struct acpi_power_req *req = (struct acpi_power_req *)m;
	struct acpi_power_resp *resp = (struct acpi_power_resp *)m;

	resp->err = acpi_power_on_device(req->device_handle);
}

/*
 * IPC handler: ACPI_REQ_POWER_OFF — call _PS3 on a device
 */
static void do_power_off(message *m)
{
	struct acpi_power_req *req = (struct acpi_power_req *)m;
	struct acpi_power_resp *resp = (struct acpi_power_resp *)m;

	resp->err = acpi_power_off_device(req->device_handle);
}

/*
 * IPC handler: ACPI_REQ_GET_SLEEP_STATES — return supported S-states
 */
static void do_get_sleep_states(message *m)
{
	struct acpi_sleep_states_resp *resp = (struct acpi_sleep_states_resp *)m;
	int i;

	for (i = 0; i < 5; i++)
		resp->supported[i] = acpi_sleep_states[i];
}

/* don't know where ACPI tables are, we may need to access any memory */
static int init_mem_priv(void)
{
	struct minix_mem_range mr;

	mr.mr_base = 0;
	mr.mr_limit = 0xffffffff;

	return sys_privctl(SELF, SYS_PRIV_ADD_MEM, &mr);
}

static void set_machine_mode(void)
{
    ACPI_OBJECT arg1;
    ACPI_OBJECT_LIST args;
    ACPI_STATUS as;

    arg1.Type = ACPI_TYPE_INTEGER;
    arg1.Integer.Value = machine.apic_enabled ? 1 : 0;
    args.Count = 1;
    args.Pointer = &arg1;

    as = AcpiEvaluateObject(ACPI_ROOT_OBJECT, "_PIC", &args, NULL);
    /*
     * We can silently ignore failure as it may not be implemented, ACPI should
     * provide us with correct information anyway
     */
    if (ACPI_SUCCESS(as))
	    printf("ACPI: machine set to %s mode\n",
			    machine.apic_enabled ? "APIC" : "PIC");
}

static ACPI_STATUS init_acpica(void)
{
	ACPI_STATUS status;

	status = AcpiInitializeSubsystem();
	if (ACPI_FAILURE(status))
		return status;

	status = AcpiInitializeTables(NULL, 16, FALSE);
	if (ACPI_FAILURE(status))
		return status;

	status = AcpiLoadTables();
	if (ACPI_FAILURE(status))
		return status;

	status = AcpiEnableSubsystem(0);
	if (ACPI_FAILURE(status))
		return status;

	status = AcpiInitializeObjects(0);
	if (ACPI_FAILURE(status))
		return status;

	set_machine_mode();
	
	pci_scan_devices();

	/* Walk ACPI namespace for Device() nodes */
	acpi_enumerate_devices();

	/*
	 * Initialize GPE routing — enable runtime GPEs, install fixed event
	 * handlers (power button, sleep button, RTC alarm).
	 */
	acpi_gpe_init();

	/*
	 * Initialize PCI hot-plug — find PCI root bridges and install
	 * ACPI System Notify handlers for BUS_CHECK, DEVICE_CHECK,
	 * and EJECT_REQUEST events.
	 */
	acpi_hotplug_init();

	return AE_OK;
}

void init_acpi(void)
{
	ACPI_STATUS acpi_err;
	/* test conditions for acpi */
	if (sys_getmachine(&machine)) {
		printf("ACPI: no machine\n");
		return;
	}
	if (machine.acpi_rsdp == 0) {
		printf("ACPI: no RSDP\n");
		return;
	}
	if (init_mem_priv()) {
		printf("ACPI: no mem access\n");
		return;
	}

	if ((acpi_err = init_acpica()) == AE_OK) {
		acpi_enabled = 1;
		printf("ACPI: ACPI enabled\n");
	}
	else {
		acpi_enabled = 0;
		printf("ACPI: ACPI failed with err %d\n", acpi_err);
	}
}

static int sef_cb_init_fresh(int type, sef_init_info_t *info)
{
	int r;

	init_acpi();

	/* Let SEF know about ACPI special cache word. */
	r = sef_llvm_add_special_mem_region((void*)0xCACACACA, 1,
	    "%MMAP_CACHE_WORD");
	if(r < 0) {
	    printf("acpi: sef_llvm_add_special_mem_region failed %d\n", r);
	}

	/* XXX To-do: acpi requires custom state transfer handlers for
	 * unions acpi_operand_object and acpi_generic_state (and nested unions)
	 * for generic state transfer to work correctly.
	 */

	return OK;
}

static void sef_local_startup()
{
  /* Register init callbacks. */
  sef_setcb_init_fresh(sef_cb_init_fresh);
  sef_setcb_init_lu(sef_cb_init_fresh);
  sef_setcb_init_restart(sef_cb_init_fresh);

  /* Let SEF perform startup. */
  sef_startup();
}

static void do_power_on(message *m);
static void do_power_off(message *m);
static void do_get_sleep_states(message *m);

int main(void)
{
	int err;
	message m;
	int ipc_status;
	int notify_pending;

	sef_local_startup();

	/* Query supported sleep states */
	acpi_query_sleep_states();

	for(;;) {
		err = driver_receive(ANY, &m, &ipc_status);
		if (err != OK) {
			printf("ACPI: driver_receive failed: %d\n", err);
			continue;
		}

		/*
		 * Handle HARDWARE notification — SCI (System Control Interrupt).
		 * The SCI handler was installed by AcpiOsInstallInterruptHandler.
		 * Check m_source (not call_nr) — correct MINIX notify detection.
		 */
		if (is_notify(ipc_status) && m.m_source == HARDWARE) {
			acpi_dispatch_sci();
			continue;
		}

		/* Process deferred execution queue (max 16 per iteration to stay responsive) */
		acpi_os_process_exec_queue();

		switch (((struct acpi_request_hdr *)&m)->request) {
		case ACPI_REQ_GET_IRQ:
			do_get_irq(&m);
			break;
		case ACPI_REQ_MAP_BRIDGE:
			do_map_bridge(&m);
			break;
		case ACPI_REQ_POWER_ON:
			do_power_on(&m);
			break;
		case ACPI_REQ_POWER_OFF:
			do_power_off(&m);
			break;
	case ACPI_REQ_GET_SLEEP_STATES:
		do_get_sleep_states(&m);
		break;
	case ACPI_REQ_ENUM_DEVICES:
		do_enum_devices(&m);
		break;
	default:
			printf("ACPI: ignoring unsupported request %d "
			    "from %d\n",
			    ((struct acpi_request_hdr *)&m)->request,
			    ((struct acpi_request_hdr *)&m)->m_source);
		}

		err = ipc_send(m.m_source, &m);
		if (err != OK) {
			printf("ACPI: ipc_send failed: %d\n", err);
		}
	}
}
