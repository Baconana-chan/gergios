/* sched_boost() — Boost scheduling priority and quantum for a process.
 *
 * This function is primarily used by the Reincarnation Server (RS) as part
 * of recovery strategies: after a service successfully restarts, its
 * priority and/or quantum may be increased to compensate for reduced
 * resources during minimal mode.
 *
 * For kernel-scheduled processes (scheduler_e == KERNEL), this directly
 * updates the kernel's scheduling parameters via sys_schedctl().
 *
 * For user-space scheduler processes, it sends a SCHEDULING_BOOST message.
 */
#include "syslib.h"
#include <assert.h>
#include <string.h>
#include <minix/sched.h>

/*===========================================================================*
 *				sched_boost				     *
 *===========================================================================*/
int sched_boost(endpoint_t scheduler_e,
		endpoint_t schedulee_e,
		int maxprio,
		int quantum)
{
	int rv;
	message m;

	/* No scheduler given? We are done. */
	if(scheduler_e == NONE) {
		return OK;
	}

	assert(_ENDPOINT_P(schedulee_e) >= 0);
	assert(maxprio >= 0);
	assert(quantum > 0);

	/* The KERNEL must schedule this process. */
	if(scheduler_e == KERNEL) {
		/* sys_schedctl(SCHEDCTL_FLAG_KERNEL, ep, maxprio, quantum, cpu)
		 * marks the process as kernel-scheduled and updates its
		 * priority and quantum. We set cpu=0 as default; the kernel
		 * will use its own CPU assignment. */
		if ((rv = sys_schedctl(SCHEDCTL_FLAG_KERNEL,
			schedulee_e, maxprio, quantum, 0)) != OK) {
			return rv;
		}
		return OK;
	}

	/* A user-space scheduler must handle this. */
	memset(&m, 0, sizeof(m));
	m.m_lsys_sched_scheduling_start.endpoint	= schedulee_e;
	m.m_lsys_sched_scheduling_start.parent		= schedulee_e;
	m.m_lsys_sched_scheduling_start.maxprio		= maxprio;
	m.m_lsys_sched_scheduling_start.quantum		= quantum;

	/* Send the boost request to the scheduler */
	if ((rv = _taskcall(scheduler_e, SCHEDULING_BOOST, &m))) {
		return rv;
	}

	return OK;
}
