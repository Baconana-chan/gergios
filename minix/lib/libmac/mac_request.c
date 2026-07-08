/* libmac — userspace MAC (Mandatory Access Control) check library.
 *
 * This file implements mac_request(), which sends a MAC check request
 * to the macd policy daemon via IPC and returns the decision.
 *
 * If macd is not available (e.g., not started yet), the default is to
 * ALLOW the operation (fail-open). This can be changed to fail-closed
 * later.
 *
 * Protocol:
 *   Client → macd:  MACD_CHECK (m_type)
 *                    MACD_WHAT  (m4_l1) — MAC_* hook type
 *                    MACD_SRC   (m4_l2) — source endpoint
 *                    MACD_DST   (m4_l3) — target endpoint
 *                    MACD_CTX1  (m4_l4) — context word 1
 *
 *   macd → Client:  reply m_type = MAC_ALLOW (0) or EACCES (denied)
 *
 * Dependencies: requires libsys (provides minix_rs_lookup).
 * All MINIX servers already link with -lsys, so this is automatic.
 */

#include <minix/mac.h>
#include <minix/com.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <minix/rs.h>
#include <stdlib.h>
#include <string.h>

/*===========================================================================*
 *				mac_request				     *
 *===========================================================================*/
int mac_request(int what, mac_context_t *ctx)
{
	message m;
	int r;
	static int macd_initialized = 0;
	static endpoint_t macd_ep = NONE;

	if (ctx == NULL)
		return MAC_DENY;

	/* Lazily look up macd via RS. */
	if (!macd_initialized) {
		r = minix_rs_lookup("macd", &macd_ep);
		if (r != OK) {
			macd_ep = NONE;
		}
		macd_initialized = 1;
	}

	/* If macd is not running, default to ALLOW. */
	if (macd_ep == NONE)
		return MAC_ALLOW;

	memset(&m, 0, sizeof(m));
	m.m_type = MACD_CHECK;
	m.MACD_WHAT = what;

	/* Pack context into message based on hook type. */
	switch (what) {
	case MAC_IPC_SEND:
	case MAC_IPC_RECEIVE:
		m.MACD_SRC = ctx->ipc.mac_src;
		m.MACD_DST = ctx->ipc.mac_dst;
		m.MACD_CTX1 = ctx->ipc.mac_call_nr;
		break;

	case MAC_FILE_ACCESS:
		m.MACD_SRC = ctx->file.mac_proc;
		m.MACD_DST = ctx->file.mac_fs_ep;
		m.MACD_CTX1 = (int)ctx->file.mac_access_desired;
		break;

	case MAC_PRIVCTL_SET_SYS:
		m.MACD_SRC = ctx->privctl.mac_caller;
		m.MACD_DST = ctx->privctl.mac_target;
		m.MACD_CTX1 = ctx->privctl.mac_request;
		break;

	case MAC_DEVICE_BIND:
		m.MACD_SRC = ctx->device.mac_driver;
		m.MACD_DST = ctx->device.mac_devman;
		m.MACD_CTX1 = ctx->device.mac_device_id;
		break;

	case MAC_PROC_FORK:
	case MAC_PROC_EXEC:
	case MAC_PROC_KILL:
		m.MACD_SRC = ctx->proc.mac_caller;
		m.MACD_DST = ctx->proc.mac_target;
		m.MACD_CTX1 = ctx->proc.mac_signal;
		break;

	default:
		/* Unknown hook type — allow. */
		return MAC_ALLOW;
	}

	/* Send request and wait for reply. */
	r = ipc_sendrec(macd_ep, &m);
	if (r != OK) {
		/* Communication failure — allow by default. */
		return MAC_ALLOW;
	}

	/* The reply m_type indicates the decision. */
	if (m.m_type == MAC_ALLOW)
		return MAC_ALLOW;

	return MAC_DENY;
}
