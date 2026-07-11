/* auditctl — Audit Daemon Control Tool for GergiOS.
 *
 * Phase 5.3: Command-line interface to manage the auditd daemon.
 *
 * Usage:
 *   auditctl -s          Show auditd status
 *   auditctl -e enable   Enable audit logging
 *   auditctl -e disable  Disable audit logging
 *   auditctl -f          Force immediate poll of kernel buffer
 *   auditctl -r          Reopen log file (for log rotation)
 *   auditctl -h          Show help
 *
 * Communication:
 *   - Looks up auditd endpoint via DS label lookup
 *   - Sends IPC message (AUDITD_RQ_*) to auditd
 *   - Receives reply and prints formatted output
 */

/* _SYSTEM enables MINIX system-level APIs (SEF, DS label lookup, etc.).
 * _MINIX_SYSTEM is also needed explicitly for ds.h prototypes. */
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
#include <minix/audit.h>

#include <sys/types.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <unistd.h>

/* Program name for error messages. */
static const char *progname;

/*===========================================================================*
 *				Look up auditd endpoint                      *
 *===========================================================================*/
static int find_auditd(endpoint_t *auditd_ep)
{
	int r;

	r = ds_retrieve_label_endpt("auditd", auditd_ep);
	if (r != OK) {
		fprintf(stderr, "%s: auditd not found (DS error %d)\n",
		    progname, r);
		return -1;
	}

	return 0;
}

/*===========================================================================*
 *				Send IPC to auditd                           *
 *===========================================================================*/
static int auditd_sendrec(message *m)
{
	endpoint_t auditd_ep;
	int r;

	if (find_auditd(&auditd_ep) != 0)
		return -1;

	r = sendrec(auditd_ep, m);
	if (r != OK) {
		fprintf(stderr, "%s: IPC error with auditd: %d\n",
		    progname, r);
		return -1;
	}

	/* Check reply status. */
	if (m->m_type != OK) {
		errno = m->m_type;  /* reply carries errno value */
		fprintf(stderr, "%s: auditd returned error: %s\n",
		    progname, strerror(m->m_type));
		return -1;
	}

	return 0;
}

/*===========================================================================*
 *				Show status (-s)                             *
 *===========================================================================*/
static int cmd_status(void)
{
	message m;
	int r;

	memset(&m, 0, sizeof(m));
	m.m_type = AUDITD_RQ_STATUS;

	r = auditd_sendrec(&m);
	if (r != 0)
		return 1;

	printf("Audit daemon status:\n");
	printf("  Log open:     %s\n",
	    (m.AUDITD_STATUS_LOG   != 0) ? "yes" : "no");
	printf("  Enabled:      %s\n",
	    (m.AUDITD_STATUS_ENABLED != 0) ? "yes" : "no");
	printf("  Poll interval: %ld ms\n",
	    (long)m.AUDITD_STATUS_POLL_MS);

	return 0;
}

/*===========================================================================*
 *				Enable/disable (-e)                          *
 *===========================================================================*/
static int cmd_enable(int enable)
{
	message m;
	int r;

	memset(&m, 0, sizeof(m));
	m.m_type = enable ? AUDITD_RQ_ENABLE : AUDITD_RQ_DISABLE;

	r = auditd_sendrec(&m);
	if (r != 0)
		return 1;

	printf("auditd: %s\n", enable ? "enabled" : "disabled");
	return 0;
}

/*===========================================================================*
 *				Force poll (-f)                              *
 *===========================================================================*/
static int cmd_poll(void)
{
	message m;
	int r;

	memset(&m, 0, sizeof(m));
	m.m_type = AUDITD_RQ_POLL_NOW;

	r = auditd_sendrec(&m);
	if (r != 0)
		return 1;

	printf("auditd: forced poll complete\n");
	return 0;
}

/*===========================================================================*
 *				Reopen log (-r)                              *
 *===========================================================================*/
static int cmd_reopen(void)
{
	message m;
	int r;

	memset(&m, 0, sizeof(m));
	m.m_type = AUDITD_RQ_REOPEN;

	r = auditd_sendrec(&m);
	if (r != 0)
		return 1;

	printf("auditd: log file reopened\n");
	return 0;
}

/*===========================================================================*
 *				Force rotation (-R)                          *
 *===========================================================================*/
static int cmd_rotate(void)
{
	message m;
	int r;

	memset(&m, 0, sizeof(m));
	m.m_type = AUDITD_RQ_ROTATE;

	r = auditd_sendrec(&m);
	if (r != 0)
		return 1;

	printf("auditd: log rotated, old logs cleaned\n");
	return 0;
}

/*===========================================================================*
 *				Usage                                         *
 *===========================================================================*/
static void usage(void)
{
	fprintf(stderr,
	    "Usage: %s -s              Show auditd status\n"
	    "       %s -e enable       Enable audit logging\n"
	    "       %s -e disable      Disable audit logging\n"
	    "       %s -f              Force immediate poll\n"
	    "       %s -r              Reopen log file\n"
	    "       %s -R              Force log rotation\n"
	    "       %s -h              Show this help\n",
	    progname, progname, progname, progname, progname,
	    progname, progname);
}

/*===========================================================================*
 *				Main                                          *
 *===========================================================================*/
int main(int argc, char *argv[])
{
	int opt;
	int err = 0;

	progname = argv[0];

	if (argc < 2) {
		usage();
		return 1;
	}

	while ((opt = getopt(argc, argv, "se:frhR")) != -1) {
		switch (opt) {
		case 's':
			/* Status — only valid alone. */
			if (err != 0) {
				fprintf(stderr, "%s: -s cannot be combined "
				    "with other options\n", progname);
				return 1;
			}
			err = cmd_status();
			break;

		case 'e':
			if (err != 0) {
				fprintf(stderr, "%s: only one option at a "
				    "time\n", progname);
				return 1;
			}
			if (strcmp(optarg, "enable") == 0)
				err = cmd_enable(1);
			else if (strcmp(optarg, "disable") == 0)
				err = cmd_enable(0);
			else {
				fprintf(stderr, "%s: -e requires 'enable' "
				    "or 'disable'\n", progname);
				return 1;
			}
			break;

		case 'f':
			if (err != 0) {
				fprintf(stderr, "%s: only one option at a "
				    "time\n", progname);
				return 1;
			}
			err = cmd_poll();
			break;

		case 'r':
			if (err != 0) {
				fprintf(stderr, "%s: only one option at a "
				    "time\n", progname);
				return 1;
			}
			err = cmd_reopen();
			break;

		case 'R':
			if (err != 0) {
				fprintf(stderr, "%s: only one option at a "
				    "time\n", progname);
				return 1;
			}
			err = cmd_rotate();
			break;

		case 'h':
			usage();
			return 0;

		default:
			usage();
			return 1;
		}
	}

	return err;
}
