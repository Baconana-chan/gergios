/* sched_rt.h — Real-Time scheduling class for MINIX (x86_64 + i386)
 *
 * Adds SCHED_FIFO (priority 1-99) and SCHED_RR (priority 1-99) classes
 * on top of the existing SCHED_OTHER (best-effort) scheduler.
 *
 * Design:
 *   - RT priorities 1-99 are mapped onto the existing NR_SCHED_QUEUES (16)
 *     run queue indices so that higher RT priority → lower queue index.
 *   - pick_proc() naturally picks the highest-priority RT process first
 *     because the queues are scanned from 0 upward.
 *   - RT processes never decay in priority (no SCHED server notification).
 *   - When an RT process is enqueued and has higher priority than the
 *     currently running non-RT process, the current process is preempted.
 */

#ifndef SCHED_RT_H
#define SCHED_RT_H

#include "proc.h"

/* Scheduling classes */
#define SCHED_OTHER	0	/* normal MINIX best-effort scheduling */
#define SCHED_FIFO	1	/* fixed-priority FIFO (runs until blocks) */
#define SCHED_RR	2	/* fixed-priority round-robin (time-sliced) */

/* RT priority range (Linux-compatible) */
#define RT_PRIO_MIN	1
#define RT_PRIO_MAX	99

/* Default RT time quantum for SCHED_RR (milliseconds) */
#define RT_RR_QUANTUM_MS	100

/* Map RT priority (1..99) to run queue index (0..NR_SCHED_QUEUES-1).
 * RT prio 99 → queue 0 (highest), RT prio 1 → queue 14.
 * RT prio 0 (not RT) → unchanged.
 */
static inline int rt_prio_to_queue(int rt_prio)
{
	if (rt_prio <= 0)
		return -1;	/* not RT */
	if (rt_prio >= RT_PRIO_MAX)
		return 0;
	return (NR_SCHED_QUEUES - 1) -
		((rt_prio - RT_PRIO_MIN) * (NR_SCHED_QUEUES - 1) /
		 (RT_PRIO_MAX - RT_PRIO_MIN));
}

/* Check if a process uses an RT scheduling class. */
static inline int proc_is_rt(const struct proc *p)
{
	return p->p_sched_class == SCHED_FIFO ||
	       p->p_sched_class == SCHED_RR;
}

/* Get the effective scheduling priority for a process.
 * For RT processes this is the queue index determined by rt_prio.
 * For SCHED_OTHER processes this is the existing p_priority.
 */
static inline int proc_effective_prio(const struct proc *p)
{
	if (proc_is_rt(p))
		return rt_prio_to_queue(p->p_rt_priority);
	return p->p_priority;
}

/* Initialize RT fields in a process table slot. */
static inline void sched_rt_proc_init(struct proc *p)
{
	p->p_sched_class = SCHED_OTHER;
	p->p_rt_priority = 0;
}

#endif /* SCHED_RT_H */
