/* minix/include/minix/audit.h — Kernel audit subsystem
 *
 * Phase 5: Audit & Monitoring — kernel audit ring buffer.
 *
 * Provides a lock-free ring buffer for security-relevant events
 * (authentication, privilege changes, denied IPC, etc.) that the
 * auditd user-space daemon reads via the SYS_AUDIT kernel call.
 *
 * The buffer has a single writer (kernel, via audit_log()) and a
 * single reader (auditd, via SYS_AUDIT/AUDIT_OP_RETRIEVE), so no
 * locking is needed beyond careful memory ordering.
 *
 * Audit events are always logged. There is no "enable/disable" bit —
 * the ring buffer is always-on. If it overflows, the oldest records
 * are silently discarded. auditd can read the buffer at any rate
 * without affecting kernel performance.
 */

#ifndef _MINIX_AUDIT_H
#define _MINIX_AUDIT_H

#include <minix/endpoint.h>
#include <minix/types.h>

/* Audit event types. */
#define AUDIT_AUTH_SUCCESS    1	/* Successful authentication */
#define AUDIT_AUTH_FAILURE    2	/* Failed authentication */
#define AUDIT_PRIV_CHANGE     3	/* Capability or privilege change */
#define AUDIT_IPC_DENIED      4	/* IPC send denied by MAC/capability */
#define AUDIT_FILE_DENIED     5	/* File access denied (DAC or MAC) */
#define AUDIT_DEVICE_BIND     6	/* Device bound/unbound */
#define AUDIT_SYSCALL_AUTH    7	/* Authorized kernel call (privctl, capctl) */
#define AUDIT_MAC_VIOLATION   8	/* MAC policy violation (permissive mode) */
#define AUDIT_SERVICE_START   9	/* Service started/stopped/updated */
#define AUDIT_SERVICE_CRASH  10	/* Service crash (panicked or segfaulted) */

/* Audit record — 128 bytes total, cache-line friendly. */
struct audit_record {
	uint32_t  ar_serial;		/* monotonic sequence number */
	uint32_t  ar_type;		/* event type (AUDIT_*) */
	int       ar_result;		/* OK / EPERM / EACCES / etc. */
	uint32_t  ar_pad;		/* padding to 64-bit align */
	uint64_t  ar_timestamp;		/* monotonic time (Hz ticks) */
	endpoint_t ar_subject;		/* process that triggered event */
	endpoint_t ar_object;		/* target process or object */
	uint32_t  ar_extra_len;		/* bytes of extra data (0-32) */
	uint8_t   ar_extra[32];		/* event-specific payload */
};

/* Number of entries in the kernel ring buffer (must be power of 2). */
#define AUDIT_BUFFER_ENTRIES	1024

/* Audit kernel call operations (for SYS_AUDIT). */
#define AUDIT_OP_GET_COUNT	1	/* get number of available records */
#define AUDIT_OP_RETRIEVE	2	/* copy records to user-supplied buffer */
#define AUDIT_OP_STATUS		3	/* get audit subsystem status */

#ifndef __ASSEMBLY__

/* Kernel-side: log an audit event.
 * Called from security-relevant code paths. Must not block.
 * Returns the serial number assigned, or 0 on buffer full.
 */
int audit_log(uint32_t type, int result,
		endpoint_t subject, endpoint_t object,
		const void *extra, uint32_t extra_len);

#endif /* __ASSEMBLY__ */

#endif /* _MINIX_AUDIT_H */
