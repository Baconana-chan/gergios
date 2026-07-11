/* bluetooth.c — Bluetooth C API implementation for GergiOS.
 *
 * Phase 8.6: Communicates with bluetoothd via MINIX IPC.
 *
 * Each bt_*() function packs arguments into a MINIX message using the
 * mess_4 layout (field names: m4_l1..m4_l4, m4_ll1) and sends it to
 * the bluetoothd daemon via sendrec().
 *
 * The daemon endpoint is looked up via ds_retrieve_label_endpt("bluetoothd").
 */

#define _SYSTEM 1
#define _MINIX_SYSTEM 1

#include <minix/config.h>
#include <minix/type.h>
#include <minix/const.h>
#include <minix/com.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <minix/ds.h>
#include <minix/ipc.h>

#include <sys/types.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>

#include "bluetooth.h"

/* ===========================================================================
 * Internal: handle structure
 * =========================================================================== */

struct bt_handle {
	endpoint_t daemon_ep;	/* bluetoothd endpoint (cached) */
	int valid;		/* 1 if endpoint has been resolved */
};

/* ===========================================================================
 * Internal: resolve daemon endpoint
 * =========================================================================== */

static int resolve_daemon(bt_handle_t *handle)
{
	int r;

	if (handle->valid)
		return BT_OK;

	r = ds_retrieve_label_endpt("bluetoothd", &handle->daemon_ep);
	if (r != OK) {
		errno = ESRCH;
		return -errno;
	}

	handle->valid = 1;
	return BT_OK;
}

/* ===========================================================================
 * Internal: helpres for message payload encoding
 * =========================================================================== */

/*
 * Message payload layout (all platforms):
 *   Offset 0-3:   m4_l1 (int32)
 *   Offset 8-11:  m4_l2 (int32)
 *   Offset 16-19: m4_l3 (int32)
 *   Offset 24-27: m4_l4 (int32)
 *   Offset 32-55: name / extra data (24 bytes)
 *
 * On LP64 (x86_64), these overlap with the first 4 bytes of each
 * `long` field in `mess_4`. We use raw byte access for portability.
 */

/* Get pointer to the payload area (skip 8-byte header: m_source + m_type). */
static uint8_t *msg_payload(message *m)
{
	return (uint8_t *)m + 8;
}

/* Write an int32 at the given payload offset. */
static void msg_write_i32(message *m, int offset, int32_t val)
{
	uint8_t *p = msg_payload(m) + offset;
	p[0] = (uint8_t)(val & 0xFF);
	p[1] = (uint8_t)((val >> 8) & 0xFF);
	p[2] = (uint8_t)((val >> 16) & 0xFF);
	p[3] = (uint8_t)((val >> 24) & 0xFF);
}

/* Read an int32 from the given payload offset (const-safe). */
static int32_t msg_read_i32(const message *m, int offset)
{
	const uint8_t *p = ((const uint8_t *)m + 8) + offset;
	return (int32_t)(p[0] | (p[1] << 8) | (p[2] << 16) | (p[3] << 24));
}

/* Pack a BD_ADDR into payload offsets 8 (low 32 bits) and 16 (high 16 bits). */
static void msg_pack_bdaddr(message *m, const bt_bdaddr_t *bdaddr)
{
	uint32_t low;
	uint16_t high;

	low = (uint32_t)bdaddr->addr[0]
	    | ((uint32_t)bdaddr->addr[1] << 8)
	    | ((uint32_t)bdaddr->addr[2] << 16)
	    | ((uint32_t)bdaddr->addr[3] << 24);
	high = (uint16_t)bdaddr->addr[4]
	     | ((uint16_t)bdaddr->addr[5] << 8);

	msg_write_i32(m, 8, (int32_t)low);
	msg_write_i32(m, 16, (int32_t)high);
}

/* Copy a name string into payload offset 32 (max 48 bytes including null). */
static void msg_write_name(message *m, const char *name, size_t maxlen)
{
	uint8_t *dst = msg_payload(m) + 32;
	size_t len = strnlen(name, maxlen - 1);
	memcpy(dst, name, len);
	dst[len] = '\0';
}

/* ===========================================================================
 * Internal: send a command message to the daemon and receive reply
 * =========================================================================== */

static int bt_sendrec(bt_handle_t *handle, message *m)
{
	int r;

	r = resolve_daemon(handle);
	if (r != BT_OK)
		return r;

	r = sendrec(handle->daemon_ep, m);
	if (r != OK) {
		errno = ENETDOWN;
		return -errno;
	}

	/* m_type holds the result: OK (0) or negative errno. */
	if (m->m_type != BT_OK) {
		errno = -m->m_type;
		return m->m_type;
	}

	return BT_OK;
}

/* ===========================================================================
 * Public API
 * =========================================================================== */

bt_handle_t *bt_open(void)
{
	bt_handle_t *handle;

	handle = (bt_handle_t *)malloc(sizeof(*handle));
	if (handle == NULL)
		return NULL;

	memset(handle, 0, sizeof(*handle));
	handle->valid = 0;

	/* Pre-resolve the daemon endpoint on open. */
	if (resolve_daemon(handle) != BT_OK) {
		free(handle);
		return NULL;
	}

	return handle;
}

void bt_close(bt_handle_t *handle)
{
	if (handle != NULL)
		free(handle);
}

int bt_start_discovery(bt_handle_t *handle)
{
	message m;

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_START_DISCOVERY;

	return bt_sendrec(handle, &m);
}

int bt_stop_discovery(bt_handle_t *handle)
{
	message m;

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_STOP_DISCOVERY;

	return bt_sendrec(handle, &m);
}

int bt_get_devices(bt_handle_t *handle, bt_device_t *devices, int max)
{
	message m;
	int r;
	int i;
	int count;

	if (devices == NULL || max <= 0) {
		errno = EINVAL;
		return -errno;
	}

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_GET_DEVICES;

	msg_write_i32(&m, 0, max);

	r = bt_sendrec(handle, &m);
	if (r != BT_OK)
		return r;

	count = msg_read_i32(&m, 0);
	if (count < 0)
		count = 0;
	if (count > max)
		count = max;

	/* For now, return empty list (daemon doesn't copy data via grant yet). */
	for (i = 0; i < count; i++)
		memset(&devices[i], 0, sizeof(bt_device_t));

	return count;
}

int bt_connect(bt_handle_t *handle, const bt_bdaddr_t *bdaddr)
{
	message m;

	if (bdaddr == NULL) {
		errno = EINVAL;
		return -errno;
	}

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_CONNECT;

	msg_pack_bdaddr(&m, bdaddr);

	return bt_sendrec(handle, &m);
}

int bt_disconnect(bt_handle_t *handle, uint16_t conn_handle, uint8_t reason)
{
	message m;

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_DISCONNECT;

	msg_write_i32(&m, 16, ((int32_t)conn_handle) << 16);
	msg_write_i32(&m, 24, reason & 0xFF);

	return bt_sendrec(handle, &m);
}

int bt_get_connections(bt_handle_t *handle, bt_connection_t *conns, int max)
{
	message m;
	int r;
	int i;
	int count;

	if (conns == NULL || max <= 0) {
		errno = EINVAL;
		return -errno;
	}

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_GET_CONNECTIONS;
	msg_write_i32(&m, 0, max);

	r = bt_sendrec(handle, &m);
	if (r != BT_OK)
		return r;

	count = msg_read_i32(&m, 0);
	if (count < 0)
		count = 0;
	if (count > max)
		count = max;

	/* Placeholder — empty list. */
	for (i = 0; i < count; i++)
		memset(&conns[i], 0, sizeof(bt_connection_t));

	return count;
}

int bt_set_name(bt_handle_t *handle, const char *name)
{
	message m;
	size_t len;

	if (name == NULL) {
		errno = EINVAL;
		return -errno;
	}

	if (strlen(name) >= BT_MAX_NAME) {
		errno = ENAMETOOLONG;
		return -errno;
	}

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_SET_NAME;

	/* Copy name into payload offset 32 (max 48 bytes). */
	msg_write_name(&m, name, 48);

	return bt_sendrec(handle, &m);
}

int bt_set_discoverable(bt_handle_t *handle, int enable)
{
	message m;

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_SET_DISCOVERABLE;

	msg_write_i32(&m, 8, enable ? 1 : 0);

	return bt_sendrec(handle, &m);
}

int bt_set_connectable(bt_handle_t *handle, int enable)
{
	message m;

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_SET_CONNECTABLE;

	msg_write_i32(&m, 8, enable ? 1 : 0);

	return bt_sendrec(handle, &m);
}

int bt_get_status(bt_handle_t *handle, bt_status_t *status)
{
	message m;
	int r;

	if (status == NULL) {
		errno = EINVAL;
		return -errno;
	}

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_GET_STATUS;

	r = bt_sendrec(handle, &m);
	if (r != BT_OK)
		return r;

	/* Unpack status from payload offsets 0, 8, 16, 24. */
	status->running = (int)msg_read_i32(&m, 0);
	status->num_devices = (int)msg_read_i32(&m, 8);
	status->num_connections = (int)msg_read_i32(&m, 16);
	status->enabled = (int)msg_read_i32(&m, 24);

	return BT_OK;
}

int bt_pair(bt_handle_t *handle, const bt_bdaddr_t *bdaddr)
{
	message m;

	if (bdaddr == NULL) {
		errno = EINVAL;
		return -errno;
	}

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_PAIR;

	msg_pack_bdaddr(&m, bdaddr);

	return bt_sendrec(handle, &m);
}

int bt_unpair(bt_handle_t *handle, const bt_bdaddr_t *bdaddr)
{
	message m;

	if (bdaddr == NULL) {
		errno = EINVAL;
		return -errno;
	}

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_UNPAIR;

	msg_pack_bdaddr(&m, bdaddr);

	return bt_sendrec(handle, &m);
}

int bt_register_service(bt_handle_t *handle, uint16_t psm, uint8_t channel,
    uint16_t uuid16, const char *name)
{
	message m;

	if (name == NULL) {
		errno = EINVAL;
		return -errno;
	}

	memset(&m, 0, sizeof(m));
	m.m_type = BT_RQ_REGISTER_SERVICE;

	/* Pack fields into message payload. */
	msg_write_i32(&m, 0, (int32_t)psm);		/* BT_REG_PSM */
	msg_write_i32(&m, 8, (int32_t)channel);	/* BT_REG_CHANNEL */
	msg_write_i32(&m, 16, (int32_t)uuid16);	/* BT_REG_UUID16 */
	msg_write_i32(&m, 24, 0);			/* BT_REG_FLAGS = 0 */
	msg_write_name(&m, name, 24);			/* name at offset 32 */

	int r = bt_sendrec(handle, &m);
	if (r != BT_OK)
		return r;

	/* Read the assigned service handle from offset 0. */
	int32_t handle_val = msg_read_i32(&m, 0);
	if (handle_val <= 0)
		return -EIO;

	return (int)handle_val;
}

int bt_bdaddr_parse(const char *str, bt_bdaddr_t *bdaddr)
{
	unsigned int bytes[6];
	int i;

	if (str == NULL || bdaddr == NULL) {
		errno = EINVAL;
		return -errno;
	}

	if (sscanf(str, "%02x:%02x:%02x:%02x:%02x:%02x",
	    &bytes[0], &bytes[1], &bytes[2],
	    &bytes[3], &bytes[4], &bytes[5]) != 6) {
		errno = EINVAL;
		return -errno;
	}

	for (i = 0; i < 6; i++)
		bdaddr->addr[i] = (uint8_t)(bytes[i] & 0xFF);

	return BT_OK;
}

char *bt_bdaddr_format(const bt_bdaddr_t *bdaddr, char *buf, size_t bufsize)
{
	if (bdaddr == NULL || buf == NULL || bufsize < 18)
		return NULL;

	snprintf(buf, bufsize, "%02x:%02x:%02x:%02x:%02x:%02x",
	    bdaddr->addr[0], bdaddr->addr[1], bdaddr->addr[2],
	    bdaddr->addr[3], bdaddr->addr[4], bdaddr->addr[5]);

	return buf;
}
