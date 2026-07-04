/* sched_rt.c — Real-Time scheduling class implementation.
 *
 * Key behaviours:
 *   - SCHED_FIFO: runs until it blocks or is preempted by higher-prio RT.
 *     No quantum expiry -> never calls notify_scheduler().
 *   - SCHED_RR: runs for RT_RR_QUANTUM_MS then cycles to end of its queue.
 *     Quantum is renewed (no priority decay).
 *   - RT preemption: when a higher-prio RT process becomes runnable on a CPU
 *     that is running a lower-prio RT or non-RT process, preempt immediately.
 */

#include "sched_rt.h"
#include "proc.h"
#include "smp.h"
#include "clock.h"

/*===========================================================================*
 *                      sched_rt_set_class                                   *
 *===========================================================================*/
int sched_rt_set_class(struct proc *p, int sched_class, int rt_prio,
        int rr_quantum_ms)
{
        /* Validate parameters. */
        if (sched_class < SCHED_OTHER || sched_class > SCHED_RR)
                return EINVAL;

        if (sched_class == SCHED_OTHER) {
                /* Clear RT fields; priority is managed by SCHED server. */
                p->p_sched_class = SCHED_OTHER;
                p->p_rt_priority = 0;
                return OK;
        }

        /* RT classes require rt_prio in [1, 99]. */
        if (rt_prio < RT_PRIO_MIN || rt_prio > RT_PRIO_MAX)
                return EINVAL;

        p->p_sched_class = sched_class;
        p->p_rt_priority = rt_prio;

        /* Map RT priority to the effective run queue index.
         * This puts RT processes in the high-priority queues (0-14).
         */
        int queue = rt_prio_to_queue(rt_prio);
        p->p_priority = queue;

        /* Set quantum.
         * SCHED_RR: use the given quantum or default (RT_RR_QUANTUM_MS).
         * SCHED_FIFO: set a very large quantum so proc_no_time() is never
         * called during normal FIFO execution. The caller must set a
         * non-zero p_cpu_time_left because MINIX scheduler checks
         * switch_to_user(): if (!p->p_cpu_time_left) proc_no_time(p);
         */
        if (sched_class == SCHED_RR) {
                int qms = (rr_quantum_ms > 0) ? rr_quantum_ms : RT_RR_QUANTUM_MS;
                p->p_quantum_size_ms = qms;
                p->p_cpu_time_left = ms_2_cpu_time(qms);
        } else {
                /* SCHED_FIFO: large quantum so it never expires in practice.
                 * 1000000 ms = ~16.7 minutes, enough for any realistic FIFO
                 * workload between blocking points.
                 */
                p->p_quantum_size_ms = 0;       /* unlimited for FIFO */
                p->p_cpu_time_left = ms_2_cpu_time(1000000); /* ~16.7 min */
        }

        return OK;
}

/*===========================================================================*
 *                      sched_rt_may_preempt                                 *
 *===========================================================================*/
/* Check whether 'new_p' (being enqueued) should preempt the currently
 * running process on the same CPU.
 *
 * Returns TRUE if the current process should be preempted.
 * NOTE: the caller must also check priv(current)->s_flags & PREEMPTIBLE.
 */
int sched_rt_may_preempt(const struct proc *current,
        const struct proc *new_p)
{
        /* If the current process is itself RT, compare RT priorities. */
        if (proc_is_rt(current) && proc_is_rt(new_p)) {
                return new_p->p_rt_priority > current->p_rt_priority;
        }

        /* If the new process is RT and the current is not, always preempt. */
        if (proc_is_rt(new_p) && !proc_is_rt(current))
                return TRUE;

        /* Non-RT vs non-RT: let the existing priority-based preemption work. */
        if (!proc_is_rt(new_p)) {
                return new_p->p_priority < current->p_priority;
        }

        return FALSE;
}

/*===========================================================================*
 *                      sched_rt_handle_quantum                              *
 *===========================================================================*/
/* Called when a process's quantum has expired (p_cpu_time_left == 0).
 * For RT processes, we renew the quantum instead of notifying the SCHED
 * server (which would lower priority). Returns TRUE if the quantum was
 * handled (caller should skip notify_scheduler), FALSE if the caller
 * should handle it normally (for SCHED_OTHER).
 */
int sched_rt_handle_quantum(struct proc *p)
{
        if (!proc_is_rt(p))
                return FALSE;   /* not RT, handle normally */

        if (p->p_sched_class == SCHED_FIFO) {
                /* SCHED_FIFO: quantum never expires. Set a large value. */
                p->p_cpu_time_left = ms_2_cpu_time(1000000);   /* ~16.7 min */
                return TRUE;
        }

        /* SCHED_RR: renew quantum and rotate to end of queue.
         * Setting p_cpu_time_left and letting the normal enqueue/dequeue
         * path handle the rest. The process has been dequeued by
         * notify_scheduler() -> RTS_SET(p, RTS_NO_QUANTUM).
         * We just renew the quantum and clear NO_QUANTUM so it gets
         * enqueued at the end of its priority queue.
         */
        p->p_cpu_time_left = ms_2_cpu_time(p->p_quantum_size_ms);
        RTS_UNSET(p, RTS_NO_QUANTUM);
        return TRUE;
}
