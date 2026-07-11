/* bluetooth.h — Bluetooth C API for GergiOS.
 *
 * Phase 8.6: Userspace Bluetooth library.
 *
 * This library provides a C API for communicating with the Bluetooth
 * daemon (bluetoothd) via MINIX IPC.
 *
 * Usage:
 *   1. bt_open()          — find bluetoothd endpoint
 *   2. bt_*()             — send commands to the daemon
 *   3. bt_close()         — release resources
 *
 * All functions return BT_OK (0) on success, or a negative errno on error.
 */

#ifndef _LIBBLUETOOTH_H
#define _LIBBLUETOOTH_H

#include <minix/endpoint.h>
#include <minix/ipc.h>
#include <minix/com.h>
#include <sys/types.h>

/* Return value for successful operations. */
#define BT_OK		0

/* Maximum length of a device name. */
#define BT_MAX_NAME	248

/* Maximum number of devices returned in a list. */
#define BT_MAX_DEVICES	32

/* Maximum number of connections returned in a list. */
#define BT_MAX_CONNECTIONS 16

/* BD_ADDR length (6 bytes). */
#define BT_BDADDR_LEN	6

/* Bluetooth device type. */
#define BT_DEV_CLASSIC	0	/* BR/EDR only */
#define BT_DEV_LE	1	/* Low Energy only */
#define BT_DEV_DUAL	2	/* Dual-mode */

/* Connection state. */
#define BT_CONN_CONNECTING	0
#define BT_CONN_CONNECTED	1
#define BT_CONN_DISCONNECTING	2

/* Connection direction. */
#define BT_CONN_OUTGOING	0
#define BT_CONN_INCOMING	1

/* Opaque handle for the Bluetooth daemon connection. */
typedef struct bt_handle bt_handle_t;

/* BD_ADDR structure (6 bytes, LSB-first on wire). */
typedef struct {
	uint8_t addr[BT_BDADDR_LEN];
} bt_bdaddr_t;

/* Remote device information. */
typedef struct {
	bt_bdaddr_t	bdaddr;		/* Bluetooth address */
	char		name[BT_MAX_NAME]; /* Device name (null-terminated) */
	uint8_t		dev_class[3];	/* Class of device */
	int		dev_type;	/* BT_DEV_CLASSIC, _LE, _DUAL */
	int8_t		rssi;		/* Signal strength (dBm) */
	int		bonded;		/* 1 if paired */
} bt_device_t;

/* Connection information. */
typedef struct {
	bt_bdaddr_t	bdaddr;		/* Remote device address */
	uint16_t	handle;		/* HCI connection handle */
	int		direction;	/* BT_CONN_OUTGOING or _INCOMING */
	int		state;		/* BT_CONN_CONNECTING, _CONNECTED, _DISCONNECTING */
	int		encrypted;	/* 1 if link is encrypted */
	int		authenticated;	/* 1 if link is authenticated */
	uint8_t		link_type;	/* 0=SCO, 1=ACL, 2=eSCO, 3=LE */
} bt_connection_t;

/* Daemon status. */
typedef struct {
	int		running;	/* 1 if daemon is running */
	int		num_devices;	/* Number of known devices */
	int		num_connections;/* Number of active connections */
	int		enabled;	/* 1 if Bluetooth radio is on */
} bt_status_t;

/* ===========================================================================
 * Initialization and cleanup
 * =========================================================================== */

/* Open a connection to the Bluetooth daemon.
 * Returns a handle or NULL on error (errno set). */
bt_handle_t *bt_open(void);

/* Close the connection to the Bluetooth daemon. */
void bt_close(bt_handle_t *handle);

/* ===========================================================================
 * Device discovery
 * =========================================================================== */

/* Start device discovery (inquiry scan). */
int bt_start_discovery(bt_handle_t *handle);

/* Stop device discovery. */
int bt_stop_discovery(bt_handle_t *handle);

/* Get list of discovered devices.
 * 'devices' must point to an array of at least BT_MAX_DEVICES elements.
 * Returns the number of devices written to the array, or negative errno. */
int bt_get_devices(bt_handle_t *handle, bt_device_t *devices, int max);

/* ===========================================================================
 * Connection management
 * =========================================================================== */

/* Connect to a remote device by BD_ADDR (6 bytes LSB-first).
 * Returns 0 on success, or negative errno. */
int bt_connect(bt_handle_t *handle, const bt_bdaddr_t *bdaddr);

/* Disconnect from a remote device by HCI handle.
 * 'reason' is the HCI disconnect reason code (default: 0x13 = Remote User). */
int bt_disconnect(bt_handle_t *handle, uint16_t conn_handle, uint8_t reason);

/* Get list of active connections.
 * 'conns' must point to an array of at least BT_MAX_CONNECTIONS elements.
 * Returns the number of connections written, or negative errno. */
int bt_get_connections(bt_handle_t *handle, bt_connection_t *conns, int max);

/* ===========================================================================
 * Local configuration
 * =========================================================================== */

/* Set the local device name (max BT_MAX_NAME-1 chars). */
int bt_set_name(bt_handle_t *handle, const char *name);

/* Enable or disable discoverability (inquiry scan). */
int bt_set_discoverable(bt_handle_t *handle, int enable);

/* Enable or disable connectability (page scan). */
int bt_set_connectable(bt_handle_t *handle, int enable);

/* ===========================================================================
 * Status and pairing
 * =========================================================================== */

/* Get daemon status. */
int bt_get_status(bt_handle_t *handle, bt_status_t *status);

/* Pair with a remote device.
 * Returns 0 on success, or negative errno. */
int bt_pair(bt_handle_t *handle, const bt_bdaddr_t *bdaddr);

/* Unpair a remote device.
 * Returns 0 on success, or negative errno. */
int bt_unpair(bt_handle_t *handle, const bt_bdaddr_t *bdaddr);

/* ===========================================================================
 * Service registration
 * =========================================================================== */

/* Register an SDP service with the daemon.
 *   psm:     L2CAP PSM (e.g. 0x0003 for RFCOMM, 0 for no L2CAP)
 *   channel: protocol-specific channel (e.g. RFCOMM ch. 1-30, 0 for none)
 *   uuid16:  Service Class UUID16 (e.g. 0x1101 for Serial Port)
 *   name:    service name (max 23 chars)
 *
 * Returns: positive service handle on success,
 *          or negative errno on failure. */
int bt_register_service(bt_handle_t *handle, uint16_t psm, uint8_t channel, uint16_t uuid16, const char *name);

/* ===========================================================================
 * Utility
 * =========================================================================== */

/* Parse a BD_ADDR string ("XX:XX:XX:XX:XX:XX") into a bt_bdaddr_t.
 * Returns BT_OK on success, or negative errno on parse error. */
int bt_bdaddr_parse(const char *str, bt_bdaddr_t *bdaddr);

/* Format a bt_bdaddr_t as a string ("XX:XX:XX:XX:XX:XX").
 * 'buf' must be at least 18 bytes. Returns 'buf'. */
char *bt_bdaddr_format(const bt_bdaddr_t *bdaddr, char *buf, size_t bufsize);

#endif /* !_LIBBLUETOOTH_H */
