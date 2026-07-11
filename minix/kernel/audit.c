/* kernel/audit.c — Kernel audit ring buffer
 *
 * Phase 5.1: Kernel audit buffer.
 * Provides a lock-free ring buffer for security-relevant events
 * (authentication, privilege changes, denied IPC, etc.) that the
 * auditd user-space daemon reads via the SYS_AUDIT kernel call.
 *
 * Design:
 * - Single writer (kernel, via audit_log()) — no locks needed.
 * - Single reader (auditd, via SYS_AUDIT) — no locks needed.
 * - Power-of-2 ring buffer for wrap-around via masking.
 * - Always-on: no enable/disable. Overflow silently discards oldest.
 * - The buffer pointer always advances; readers track their own
 *   position and never modify the shared state.
 *
 * SYS_AUDIT kernel call operations:
 *   AUDIT_OP_GET_COUNT — returns number of records available
 *   AUDIT_OP_RETRIEVE  — copies available records to user buffer
 *                        via data_copy()
 *   AUDIT_OP_STATUS    — returns buffer size and current write index
 */

#include "kernel/system.h"
#include <minix/audit.h>
#include <string.h>
#include <assert.h>
#include <minix/endpoint.h>
#include <minix/com.h>

#if USE_AUDIT

/* Audit ring buffer (power of 2). */
static struct audit_record audit_buf[AUDIT_BUFFER_ENTRIES];
static uint32_t audit_write_idx;	/* next slot to write (always advances) */
static uint32_t audit_serial;		/* monotonic event counter */

/* Mask for wrap-around: buffer size - 1 (must be power of 2). */
#define AUDIT_MASK (AUDIT_BUFFER_ENTRIES - 1)

/*===========================================================================*
 *				audit_log				     *
 *===========================================================================*/
int audit_log(uint32_t type, int result,
		endpoint_t subject, endpoint_t object,
		const void *extra, uint32_t extra_len)
{
	struct audit_record *rec;
	uint32_t idx;

	if (type == 0 || type > AUDIT_SERVICE_CRASH || extra_len > sizeof(rec->ar_extra))
		return 0;	/* invalid parameters — cannot log */

	/* Allocate a slot (always advances, may overwrite oldest). */
	idx = audit_write_idx++;
	rec = &audit_buf[idx & AUDIT_MASK];

	/* Fill the record. */
	rec->ar_serial = ++audit_serial;
	rec->ar_type = type;
	rec->ar_result = result;
	rec->ar_timestamp = get_monotonic();
	rec->ar_subject = subject;
	rec->ar_object = object;
	rec->ar_extra_len = extra_len;

	if (extra_len > 0 && extra != NULL) {
		memcpy(rec->ar_extra, extra, extra_len);
		/* Zero out remaining extra space for safety. */
		if (extra_len < sizeof(rec->ar_extra))
			memset(rec->ar_extra + extra_len, 0,
				sizeof(rec->ar_extra) - extra_len);
	} else {
		memset(rec->ar_extra, 0, sizeof(rec->ar_extra));
	}

	return (int)rec->ar_serial;
}

/*===========================================================================*
 *				do_audit				     *
 *===========================================================================*/
int do_audit(struct proc *caller, message *m_ptr)
{
	int op;

	op = m_ptr->AUDIT_OP;

	switch (op) {

	case AUDIT_OP_GET_COUNT:
	{
		/* Return the number of records available to read.
		 * Before the buffer fills up (< ENTRIES writes), return
		 * the actual count. Once full, always return ENTRIES —
		 * the reader fetches the oldest ENTRIES records. After
		 * uint32_t overflow (~4B events), the count may briefly
		 * be off, but this is negligible in practice. */
		uint32_t avail;
		if (audit_write_idx >= AUDIT_BUFFER_ENTRIES)
			avail = AUDIT_BUFFER_ENTRIES;
		else
			avail = audit_write_idx;
		m_ptr->AUDIT_COUNT = (int)avail;
		return OK;
	}

	case AUDIT_OP_RETRIEVE:
	{
		/* Copy up to 'count' records into the user's buffer.
		 * The user provides a virtual address via AUDIT_BUF.
		 * We copy starting from the oldest available record,
		 * wrapping around the ring buffer if needed. */
		uint32_t count, avail, start_idx;
		uint32_t first_chunk, remaining;
		vir_bytes user_buf;
		int r;

		if (m_ptr->AUDIT_COUNT <= 0 ||
		    m_ptr->AUDIT_COUNT > (int)AUDIT_BUFFER_ENTRIES)
			return EINVAL;

		user_buf = (vir_bytes)m_ptr->AUDIT_BUF;
		if (user_buf == 0)
			return EFAULT;

		count = (uint32_t)m_ptr->AUDIT_COUNT;
		avail = audit_write_idx;
		if (count > avail)
			count = avail;
		if (count > AUDIT_BUFFER_ENTRIES)
			count = AUDIT_BUFFER_ENTRIES;
		if (count == 0) {
			m_ptr->AUDIT_COUNT = 0;
			return OK;
		}

		/* Calculate the oldest record index. */
		start_idx = (avail - count) & AUDIT_MASK;

		/* Copy in possibly two chunks (wrap-around). */
		first_chunk = count;
		remaining = 0;
		if (start_idx + count > AUDIT_BUFFER_ENTRIES) {
			first_chunk = AUDIT_BUFFER_ENTRIES - start_idx;
			remaining = count - first_chunk;
		}

		if (first_chunk > 0) {
			r = data_copy(KERNEL,
				(vir_bytes)&audit_buf[start_idx],
				caller->p_endpoint, user_buf,
				first_chunk * sizeof(struct audit_record));
			if (r != OK)
				return r;
		}

		if (remaining > 0) {
			r = data_copy(KERNEL,
				(vir_bytes)&audit_buf[0],
				caller->p_endpoint,
				user_buf + first_chunk * sizeof(struct audit_record),
				remaining * sizeof(struct audit_record));
			if (r != OK)
				return r;
		}

		m_ptr->AUDIT_COUNT = (int)count;
		return OK;
	}

	case AUDIT_OP_STATUS:
	{
		/* Return buffer size and current write position.
		 * Only use m1_i1..m1_i3 + m1_p1 (noxfer_message-safe). */
		m_ptr->AUDIT_COUNT = AUDIT_BUFFER_ENTRIES;
		m_ptr->m1_i1 = (int)audit_write_idx;	/* write position */
		return OK;
	}

	case AUDIT_OP_LOG:
	{
		/* Log an event from a user-space server.
		 * Kernel's noxfer_message only has m1_i1..m1_i3 + m1_p1
		 * (no m1_i4). Subject is passed via m1_p1 (as intptr).
		 * Object defaults to NONE since we only have one
		 * pointer-sized field available. */
		uint32_t log_type;
		int log_result;
		endpoint_t log_subject;

		log_type = (uint32_t)m_ptr->m1_i1;
		log_result = (int)m_ptr->m1_i3;
		log_subject = (endpoint_t)(intptr_t)m_ptr->m1_p1;

		audit_log(log_type, log_result, log_subject,
		    NONE, NULL, 0);
		return OK;
	}

	default:
		return EINVAL;
	}
}

#endif /* USE_AUDIT */
