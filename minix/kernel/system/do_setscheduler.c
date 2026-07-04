/* do_setscheduler.c — SYS_SETSCHEDULER kernel call handler.
 *
 * Allows a process to set the scheduling class and RT priority
 * of another process (or itself).
 *
 * Message format:
 *   m2_i1  = endpoint of target process
 *   m2_i2  = scheduling class (SCHED_OTHER=0, SCHED_FIFO=1, SCHED_RR=2)
 *   m2_i3  = RT priority (1-99, ignored for SCHED_OTHER)
 *   m2_l1  = RR quantum in ms (0 = default, ignored for !SCHED_RR)
 *
 * Permissions:
 *   - Any process can set SCHED_OTHER for itself.
 *   - PM, RS, or the process itself can set RT scheduling (SCHED_FIFO/RR).
 */

#include "kernel/system.h"
#include <minix/endpoint.h>
#include "kernel/sched_rt.h"
#include "kernel/clock.h"

/*===========================================================================*
 *                              do_setscheduler                              *
 *===========================================================================*/
int do_setscheduler(struct proc * caller, message * m_ptr)
{
	struct proc *p;
	int proc_nr, sched_class, rt_prio, rr_quantum;
	int r;

	/* Extract parameters. */
	if (!isokendpt(m_ptr->m2_i1, &proc_nr))
		return EINVAL;

	p = proc_addr(proc_nr);
	sched_class = m_ptr->m2_i2;
	rt_prio = m_ptr->m2_i3;
	rr_quantum = (int)m_ptr->m2_l1;

	/* Permission check for RT scheduling classes.
	 * Use endpoint comparison for robustness (PM/RS can restart).
	 */
	if (sched_class != SCHED_OTHER) {
		endpoint_t caller_ep = caller->p_endpoint;
		endpoint_t target_ep = p->p_endpoint;

		/* Allow: caller is the target itself, PM, or RS */
		if (caller_ep != target_ep &&
		    caller_ep != PM_PROC_NR &&
		    caller_ep != RS_PROC_NR)
			return EPERM;
	}

	/* Apply the scheduling class change. */
	if ((r = sched_rt_set_class(p, sched_class, rt_prio, rr_quantum)) != OK)
		return r;

	/* If the process is currently runnable, re-enqueue it so the new
	 * priority takes effect immediately.
	 */
	if (proc_is_runnable(p)) {
		RTS_SET(p, RTS_NO_QUANTUM);
		p->p_cpu_time_left = ms_2_cpu_time(
			(sched_class == SCHED_RR && rr_quantum > 0) ?
				rr_quantum : RT_RR_QUANTUM_MS);
		RTS_UNSET(p, RTS_NO_QUANTUM);
	}

	return OK;
}
