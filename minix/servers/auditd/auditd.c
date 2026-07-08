/* auditd — Audit Daemon for GergiOS.
 *
 * Phase 5.2: auditd periodically reads the kernel audit ring buffer
 * via the SYS_AUDIT kernel call and writes structured audit records
 * to /var/log/audit/audit.log.
 *
 * Design:
 * - Uses SEF startup (standard MINIX service pattern)
 * - Periodic polling via sys_setalarm() + SIGALRM signal handler
 * - Writes one record per line to log file
 * - Supports auditctl IPC for enable/disable/status/reopen
 *
 * Log format (text, one record per line):
 *   serial|time|type|result|subject|object|extra_hex
 *
 * Communication:
 *   Kernel audit: _kernel_call(SYS_AUDIT, &msg)
 *   auditctl: send()/receive() with AUDITD_RQ_* message types
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
#include <minix/audit.h>

#include <sys/types.h>
#include <sys/time.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <signal.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>

/* Default paths. */
#define AUDITD_CONF_PATH    "/etc/auditd.conf"
#define AUDITD_LOG_DIR      "/var/log/audit"
#define AUDITD_LOG_PATH     "/var/log/audit/audit.log"

/* Default poll interval (HZ ticks ≈ 100Hz = 10ms per tick). */
#define AUDITD_POLL_INTERVAL    (10 * sys_hz())  /* ~10 seconds */

/* Buffer for retrieving records from the kernel. */
static struct audit_record audit_records[AUDIT_BUFFER_ENTRIES];

/* Runtime configuration. */
static struct {
	char log_path[256];
	clock_t poll_interval;	/* ticks between polls */
	int enabled;		/* 0 = paused, 1 = active */
} auditd_cfg = {
	.log_path = AUDITD_LOG_PATH,
	.poll_interval = AUDITD_POLL_INTERVAL,
	.enabled = 1,
};

/* Log file descriptor. */
static int log_fd = -1;

/*===========================================================================*
 *                    Event type name lookup                                 *
 *===========================================================================*/
static const char *audit_type_name(uint32_t type)
{
	switch (type) {
	case AUDIT_AUTH_SUCCESS:	return "AUTH_SUCCESS";
	case AUDIT_AUTH_FAILURE:	return "AUTH_FAILURE";
	case AUDIT_PRIV_CHANGE:		return "PRIV_CHANGE";
	case AUDIT_IPC_DENIED:		return "IPC_DENIED";
	case AUDIT_FILE_DENIED:		return "FILE_DENIED";
	case AUDIT_DEVICE_BIND:		return "DEVICE_BIND";
	case AUDIT_SYSCALL_AUTH:	return "SYSCALL_AUTH";
	case AUDIT_MAC_VIOLATION:	return "MAC_VIOLATION";
	case AUDIT_SERVICE_START:	return "SERVICE_START";
	case AUDIT_SERVICE_CRASH:	return "SERVICE_CRASH";
	default:			return "UNKNOWN";
	}
}

/*===========================================================================*
 *                    Open log file (append mode)                           *
 *===========================================================================*/
static int open_log(void)
{
	int fd;

	/* Ensure log directory exists. */
	(void)mkdir(AUDITD_LOG_DIR, 0755);

	fd = open(auditd_cfg.log_path, O_WRONLY | O_CREAT | O_APPEND, 0644);
	if (fd < 0) {
		printf("auditd: cannot open log %s: %s\n",
		    auditd_cfg.log_path, strerror(errno));
		return -1;
	}

	return fd;
}

/*===========================================================================*
 *                    Write one audit record to log                         *
 *===========================================================================*/
static void write_record(const struct audit_record *rec)
{
	char buf[256];
	int n;
	char extra_hex[sizeof(rec->ar_extra) * 2 + 1];
	int i;

	/* Format extra data as hex string. */
	if (rec->ar_extra_len > 0) {
		for (i = 0; i < (int)rec->ar_extra_len &&
		    i < (int)sizeof(rec->ar_extra); i++)
			sprintf(extra_hex + i * 2, "%02x", rec->ar_extra[i]);
		extra_hex[rec->ar_extra_len * 2] = '\0';
	} else {
		extra_hex[0] = '-';
		extra_hex[1] = '\0';
	}

	/* Format: serial|timestamp|type|result|subject|object|extra */
	n = snprintf(buf, sizeof(buf),
	    "%u|%llu|%s|%d|%d|%d|%s\n",
	    rec->ar_serial,
	    rec->ar_timestamp,
	    audit_type_name(rec->ar_type),
	    rec->ar_result,
	    rec->ar_subject,
	    rec->ar_object,
	    extra_hex);

	if (n > 0 && log_fd >= 0) {
		if (write(log_fd, buf, (size_t)n) < 0) {
			/* Write failed — try reopening log. */
			close(log_fd);
			log_fd = open_log();
		}
	}
}

/*===========================================================================*
 *                    Poll kernel audit buffer                              *
 *===========================================================================*/
static void poll_kernel_buffer(void)
{
	message m;
	int count, r;
	int i;

	/* Get number of available records. */
	memset(&m, 0, sizeof(m));
	m.AUDIT_OP = AUDIT_OP_GET_COUNT;

	r = _kernel_call(SYS_AUDIT, &m);
	if (r != OK)
		return;

	count = m.AUDIT_COUNT;
	if (count <= 0)
		return;

	/* Retrieve records. */
	memset(&m, 0, sizeof(m));
	m.AUDIT_OP = AUDIT_OP_RETRIEVE;
	m.AUDIT_COUNT = count;
	m.AUDIT_BUF = (vir_bytes)&audit_records[0];

	r = _kernel_call(SYS_AUDIT, &m);
	if (r != OK)
		return;

	count = m.AUDIT_COUNT;
	if (count <= 0)
		return;

	/* Write each record to the log. */
	for (i = 0; i < count; i++)
		write_record(&audit_records[i]);

	/* Flush log after each poll cycle. */
	if (log_fd >= 0)
		fsync(log_fd);
}

/*===========================================================================*
 *                    Set alarm for next poll cycle                         *
 *===========================================================================*/
static void set_poll_alarm(void)
{
	/* Set a relative alarm: fires after poll_interval ticks.
	 * 0 = relative, NULL = don't need time_left/uptime. */
	sys_setalarm2(auditd_cfg.poll_interval, 0, NULL, NULL);
}

/*===========================================================================*
 *                    Load configuration from file                          *
 *===========================================================================*/
static void load_config(void)
{
	FILE *fp;
	char line[256];
	char key[64], value[192];

	fp = fopen(AUDITD_CONF_PATH, "r");
	if (fp == NULL) {
		/* Config file not found — use defaults. */
		printf("auditd: %s not found, using defaults\n",
		    AUDITD_CONF_PATH);
		return;
	}

	while (fgets(line, sizeof(line), fp) != NULL) {
		if (line[0] == '#' || line[0] == '\n' || line[0] == '\r')
			continue;

		if (sscanf(line, "%63s = %191s", key, value) == 2) {
			if (strcmp(key, "log_path") == 0)
				strlcpy(auditd_cfg.log_path, value,
				    sizeof(auditd_cfg.log_path));
			else if (strcmp(key, "poll_interval_ms") == 0)
				auditd_cfg.poll_interval =
				    (atol(value) * sys_hz()) / 1000;
		}
	}

	fclose(fp);
	printf("auditd: config loaded from %s\n", AUDITD_CONF_PATH);
}

/*===========================================================================*
 *                    SEF callback: init                                    *
 *===========================================================================*/
static int sef_cb_init_fresh(int type, sef_init_info_t *info)
{
	/* Load configuration. */
	load_config();

	/* Open log file. */
	log_fd = open_log();
	if (log_fd < 0) {
		printf("auditd: WARNING: cannot open log, will retry\n");
	}

	/* Set initial poll alarm. */
	if (auditd_cfg.enabled)
		set_poll_alarm();

	return OK;
}

/*===========================================================================*
 *                    SEF callback: signal handler                         *
 *===========================================================================*/
static void sef_cb_signal_handler(int signo)
{
	switch (signo) {
	case SIGTERM:
		/* Flush and close log. */
		if (log_fd >= 0) {
			fsync(log_fd);
			close(log_fd);
			log_fd = -1;
		}
		break;

	case SIGHUP:
		/* Reopen log (log rotation). */
		if (log_fd >= 0)
			close(log_fd);
		log_fd = open_log();

		/* Reload config. */
		load_config();
		break;

	case SIGALRM:
		/* Periodic poll timer fired. */
		if (auditd_cfg.enabled)
			poll_kernel_buffer();

		/* Re-arm the alarm for next poll cycle. */
		if (auditd_cfg.enabled)
			set_poll_alarm();
		break;
	}
}

/*===========================================================================*
 *                    main                                                   *
 *===========================================================================*/
int main(int argc, char *argv[])
{
	int r;
	message m_in;

	/* Set up SEF callbacks. */
	sef_setcb_init_fresh(sef_cb_init_fresh);
	sef_setcb_signal_handler(sef_cb_signal_handler);

	/* Start the SEF framework. */
	sef_startup();

	/* Main loop: handle IPC messages from auditctl. */
	for (;;) {
		r = sef_receive(ANY, &m_in);
		if (r != OK) {
			printf("auditd: sef_receive error %d\n", r);
			continue;
		}

		switch (m_in.m_type) {
		case AUDITD_RQ_STATUS:
			m_in.m_type = OK;
			m_in.AUDITD_STATUS_LOG = (log_fd >= 0) ? 1 : 0;
			m_in.AUDITD_STATUS_ENABLED = auditd_cfg.enabled ? 1 : 0;
			m_in.AUDITD_STATUS_POLL_MS =
			    (int)((auditd_cfg.poll_interval * 1000) /
			    sys_hz());
			break;

		case AUDITD_RQ_ENABLE:
			auditd_cfg.enabled = 1;
			set_poll_alarm();
			m_in.m_type = OK;
			break;

		case AUDITD_RQ_DISABLE:
			auditd_cfg.enabled = 0;
			m_in.m_type = OK;
			break;

		case AUDITD_RQ_REOPEN:
			if (log_fd >= 0)
				close(log_fd);
			log_fd = open_log();
			m_in.m_type = (log_fd >= 0) ? OK : errno;
			break;

		case AUDITD_RQ_POLL_NOW:
			if (auditd_cfg.enabled)
				poll_kernel_buffer();
			m_in.m_type = OK;
			break;

		default:
			m_in.m_type = EINVAL;
			break;
		}

		/* Send reply (only if not from notification). */
		if (m_in.m_source != NONE)
			send(m_in.m_source, &m_in);
	}

	return OK;
}
