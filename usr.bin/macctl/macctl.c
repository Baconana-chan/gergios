/* macctl — MAC Daemon Control Tool for GergiOS.
 *
 * Phase 3.6: Runtime MAC enforcement toggle.
 *
 * Usage:
 *   macctl status           Show MAC enforcement status
 *   macctl on               Enable MAC enforcement
 *   macctl off              Disable MAC enforcement (all operations allowed)
 *   macctl -h               Show help
 *
 * Communication:
 *   - Looks up macd endpoint via DS label lookup
 *   - Sends IPC message (MACD_RQ_*) to macd
 *   - Receives reply and prints formatted output
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
#include <unistd.h>

static const char *progname;

/*===========================================================================*
 *              Look up macd endpoint via DS                                *
 *===========================================================================*/
static int find_macd(endpoint_t *ep)
{
	int r;

	r = ds_retrieve_label_endpt("macd", ep);
	if (r != OK) {
		fprintf(stderr, "%s: macd not found (DS error %d)\n",
		    progname, r);
		return -1;
	}
	return 0;
}

/*===========================================================================*
 *              Send IPC to macd, receive reply                             *
 *===========================================================================*/
static int macd_sendrec(message *m)
{
	endpoint_t macd_ep;
	int r;

	if (find_macd(&macd_ep) != 0)
		return -1;

	r = sendrec(macd_ep, m);
	if (r != OK) {
		fprintf(stderr, "%s: IPC error with macd: %d\n",
		    progname, r);
		return -1;
	}

	if (m->m_type != OK) {
		errno = m->m_type;
		fprintf(stderr, "%s: macd error: %s\n",
		    progname, strerror(m->m_type));
		return -1;
	}

	return 0;
}

/*===========================================================================*
 *              Command: show status                                        *
 *===========================================================================*/
static int cmd_status(void)
{
	message m;
	int r;

	memset(&m, 0, sizeof(m));
	m.m_type = MACD_RQ_STATUS;

	r = macd_sendrec(&m);
	if (r != 0)
		return 1;

	printf("MAC daemon status:\n");
	printf("  Enforcement: %s\n",
	    m.MACD_STATUS_ENABLED ? "ON" : "OFF");
	printf("  Policy rules: %d loaded\n",
	    m.MACD_STATUS_NRULES);

	return 0;
}

/*===========================================================================*
 *              Command: enable/disable                                     *
 *===========================================================================*/
static int cmd_enforce(int enable)
{
	message m;
	int r;

	memset(&m, 0, sizeof(m));
	m.m_type = enable ? MACD_RQ_ENABLE : MACD_RQ_DISABLE;

	r = macd_sendrec(&m);
	if (r != 0)
		return 1;

	printf("macd: MAC enforcement %s\n",
	    enable ? "enabled" : "disabled");
	return 0;
}

/*===========================================================================*
 *              Usage                                                       *
 *===========================================================================*/
static void usage(void)
{
	fprintf(stderr,
	    "Usage: %s status            Show MAC enforcement status\n"
	    "       %s on                Enable MAC enforcement\n"
	    "       %s off               Disable MAC enforcement\n"
	    "       %s -h                Show this help\n",
	    progname, progname, progname, progname);
}

/*===========================================================================*
 *              Main                                                        *
 *===========================================================================*/
int main(int argc, char *argv[])
{
	progname = argv[0];

	if (argc < 2) {
		usage();
		return 1;
	}

	if (strcmp(argv[1], "status") == 0)
		return cmd_status();
	else if (strcmp(argv[1], "on") == 0)
		return cmd_enforce(1);
	else if (strcmp(argv[1], "off") == 0)
		return cmd_enforce(0);
	else if (strcmp(argv[1], "-h") == 0) {
		usage();
		return 0;
	} else {
		fprintf(stderr, "%s: unknown command '%s'\n",
		    progname, argv[1]);
		usage();
		return 1;
	}
}
