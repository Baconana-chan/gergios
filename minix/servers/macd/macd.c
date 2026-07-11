/* macd — MAC (Mandatory Access Control) Policy Daemon.
 *
 * macd is the central policy decision point for the MINIX MAC framework.
 * It receives MAC_CHECK_REQUEST messages from servers (VFS, PM, etc.)
 * and kernel hooks, evaluates them against the configured security
 * policy, and replies with MAC_ALLOW or MAC_DENY.
 *
 * Policy is loaded from /etc/macd.conf at startup and can specify
 * simple allow/deny rules based on source endpoint, destination,
 * and operation type.
 *
 * Default policy: ALLOW ALL (compatible mode).
 * To enable enforcement, configure macd.conf with deny rules.
 *
 * Communication protocol:
 *   Request: m_type = MACD_CHECK
 *            MACD_WHAT  (m4_l1) — MAC_* hook type
 *            MACD_SRC   (m4_l2) — source endpoint
 *            MACD_DST   (m4_l3) — target endpoint
 *            MACD_CTX1  (m4_l4) — context word 1
 *            MACD_CTX2  (m4_l5) — context word 2
 *
 *   Reply:   m_type = MAC_ALLOW (0) or EACCES (denied)
 */

#define _SYSTEM 1

#include <minix/config.h>
#include <minix/type.h>
#include <minix/const.h>
#include <minix/com.h>
#include <minix/callnr.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <minix/sysutil.h>
#include <minix/safecopies.h>
#include <minix/bitmap.h>
#include <minix/rs.h>
#include <minix/mac.h>

#include <sys/types.h>
#include <sys/time.h>
#include <sys/resource.h>
#include <signal.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <errno.h>

#include "policy.h"

/* Buffer for one IPC message. */
static message m_in;

/* Global MAC enforcement flag.
 * When disabled (0), all handle_mac_check() calls return MAC_ALLOW
 * without consulting the policy engine, effectively disabling MAC
 * enforcement without restarting the daemon.
 * Enabled by default after successful policy load. */
static int mac_enabled = 1;

/*===========================================================================*
 *				handle_mac_check			     *
 *===========================================================================*/
static int handle_mac_check(message *m)
{
	int what;
	mac_context_t ctx;
	int result;

	/* If MAC enforcement is disabled, allow everything. */
	if (!mac_enabled)
		return MAC_ALLOW;

	what = m->MACD_WHAT;

	/* Decode context from message. */
	memset(&ctx, 0, sizeof(ctx));

	switch (what) {
	case MAC_IPC_SEND:
	case MAC_IPC_RECEIVE:
		ctx.ipc.mac_src = m->MACD_SRC;
		ctx.ipc.mac_dst = m->MACD_DST;
		ctx.ipc.mac_call_nr = m->MACD_CTX1;
		break;

	case MAC_FILE_ACCESS:
		ctx.file.mac_proc = m->MACD_SRC;
		ctx.file.mac_fs_ep = m->MACD_DST;
		ctx.file.mac_access_desired = (mode_t)m->MACD_CTX1;
		break;

	case MAC_PRIVCTL_SET_SYS:
		ctx.privctl.mac_caller = m->MACD_SRC;
		ctx.privctl.mac_target = m->MACD_DST;
		ctx.privctl.mac_request = m->MACD_CTX1;
		break;

	case MAC_DEVICE_BIND:
		ctx.device.mac_driver = m->MACD_SRC;
		ctx.device.mac_devman = m->MACD_DST;
		ctx.device.mac_device_id = m->MACD_CTX1;
		break;

	case MAC_PROC_FORK:
	case MAC_PROC_EXEC:
	case MAC_PROC_KILL:
		ctx.proc.mac_caller = m->MACD_SRC;
		ctx.proc.mac_target = m->MACD_DST;
		ctx.proc.mac_signal = m->MACD_CTX1;
		break;

	default:
		/* Unknown hook type — allow. */
		return MAC_ALLOW;
	}

	/* Evaluate policy. */
	result = policy_check(what, &ctx);

	return result;
}

/*===========================================================================*
 *				handle_mac_enable			     *
 *===========================================================================*/
static void handle_mac_enable(int enable, message *m)
{
	if (enable) {
		mac_enabled = 1;
		printf("macd: MAC enforcement ENABLED\n");
	} else {
		mac_enabled = 0;
		printf("macd: MAC enforcement DISABLED (all operations allowed)\n");
	}
	m->m_type = OK;
}

/*===========================================================================*
 *				handle_mac_status			     *
 *===========================================================================*/
static void handle_mac_status(message *m)
{
	/* Count rules in policy. */
	m->m_type = OK;
	m->MACD_STATUS_ENABLED = mac_enabled ? 1 : 0;
	m->MACD_STATUS_NRULES = 0;
	policy_count_rules(&m->MACD_STATUS_NRULES);
}

/*===========================================================================*
 *				sef_cb_init_fresh			     *
 *===========================================================================*/
static int sef_cb_init_fresh(int type, sef_init_info_t *info)
{
	/* Initialize policy engine. */
	policy_init();

	return OK;
}

/*===========================================================================*
 *				sef_cb_signal_handler			     *
 *===========================================================================*/
static void sef_cb_signal_handler(int signo)
{
	/* Only handle SIGTERM for now. */
	if (signo == SIGTERM) {
		policy_cleanup();
	}
}

/*===========================================================================*
 *				main					     *
 *===========================================================================*/
int main(int argc, char *argv[])
{
	int r, result;
	endpoint_t caller;

	/* Set up SEF startup and signal handling. */
	sef_setcb_init_fresh(sef_cb_init_fresh);
	sef_setcb_signal_handler(sef_cb_signal_handler);

	/* Start the SEF framework. */
	sef_startup();

	/* Main loop: receive MAC check requests and respond. */
	for (;;) {
		r = sef_receive_status(ANY, &m_in, NULL);
		if (r != OK) {
			printf("macd: sef_receive_status error %d\n", r);
			continue;
		}
		caller = m_in.m_source;

		switch (m_in.m_type) {
		case MACD_CHECK:
			result = handle_mac_check(&m_in);
			m_in.m_type = result;
			break;

		case MACD_RQ_ENABLE:
			handle_mac_enable(1, &m_in);
			break;

		case MACD_RQ_DISABLE:
			handle_mac_enable(0, &m_in);
			break;

		case MACD_RQ_STATUS:
			handle_mac_status(&m_in);
			break;

		default:
			/* Unknown message type — ignore. */
			m_in.m_type = EINVAL;
			break;
		}

		/* Send reply. */
		if ((r = send(caller, &m_in)) != OK) {
			printf("macd: send to %d failed: %d\n", caller, r);
		}
	}

	return OK;
}
