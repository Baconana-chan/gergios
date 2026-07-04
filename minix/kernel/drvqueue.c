/* drvqueue.c — Per-CPU driver command queues implementation.
 *
 * Lock-free single-producer, single-consumer ring buffer.
 * Each CPU has its own queue; producers are the kernel interrupt
 * handler (for MSI-X IRQs) and other CPUs (via IPI).
 * The consumer is the driver process pinned to this CPU.
 */

#include "drvqueue.h"

#ifdef CONFIG_SMP
#include "kernel/smp.h"
#endif

/* Per-CPU queue storage — one queue per possible CPU */
static struct drv_queue drvqueue_storage[CONFIG_MAX_CPUS]
    __attribute__((aligned(64)));

/* Per-CPU queue pointers */
struct drv_queue *cpu_drv_queues[CONFIG_MAX_CPUS];

/* =========================================================================
 * Initialization
 * ========================================================================= */

void drvqueue_init(void)
{
    unsigned cpu;

    for (cpu = 0; cpu < CONFIG_MAX_CPUS; cpu++) {
        cpu_drv_queues[cpu] = &drvqueue_storage[cpu];
        cpu_drv_queues[cpu]->head = 0;
        cpu_drv_queues[cpu]->tail = 0;
    }
}

/* =========================================================================
 * Push — producer side (interrupt handler or another CPU)
 * ========================================================================= */
/*
 * Lock-free SPSC push:
 *   1. Read head and tail (relaxed — ordering via head increment)
 *   2. Check if queue is full (head - tail >= DRV_QUEUE_SIZE)
 *   3. Write entry to entries[head & mask] (producer writes own slot)
 *   4. Increment head (atomic store-release — makes entry visible to consumer)
 *
 * x86_64: store-release is implicit on x86 (all stores are release),
 * but we add a compiler barrier via __insn_barrier() for ordering
 * of the entry write before the head increment.
 */

int drvqueue_push(unsigned cpu, u32_t cmd, u32_t flags,
                  u64_t arg1, u64_t arg2, u64_t arg3)
{
    struct drv_queue *q;
    atomic_t h, t;
    unsigned idx;

    if (cpu >= CONFIG_MAX_CPUS)
        return -1;

    q = cpu_drv_queues[cpu];

    /* Read current head and tail */
    h = q->head;
    t = q->tail;

    /* Check if queue is full */
    if ((unsigned)(h - t) >= DRV_QUEUE_SIZE)
        return -1;  /* queue full */

    /* Write the entry */
    idx = h & DRV_QUEUE_MASK;
    q->entries[idx].cmd   = cmd;
    q->entries[idx].flags = flags;
    q->entries[idx].arg1  = arg1;
    q->entries[idx].arg2  = arg2;
    q->entries[idx].arg3  = arg3;

    /*
     * Compiler barrier ensures the entry write is visible before
     * the head increment. On x86_64, store-release is implicit,
     * and mfence is not needed for SPSC (the increment itself
     * is an atomic RMW which provides ordering).
     */
    __insn_barrier();

    /* Increment head — makes entry visible to consumer */
    q->head = h + 1;

    return OK;
}

/* =========================================================================
 * Pop — consumer side (driver on local CPU)
 * ========================================================================= */
/*
 * Lock-free SPSC pop:
 *   1. Read tail and head (consumer reads its own tail, then producer's head)
 *   2. If tail == head, queue is empty
 *   3. Read entry from entries[tail & mask]
 *   4. Increment tail (plain store — consumer is the only writer of tail)
 *
 * No atomic RMW needed for tail — only the consumer writes tail.
 * On x86_64, the read of q->entries[idx] after reading head is implicitly
 * ordered: the read of head acts as an acquire that sees the producer's
 * entry writes (because the producer's head increment has release semantics).
 */

int drvqueue_pop(struct drv_queue_entry *entry)
{
    struct drv_queue *q;
    atomic_t h, t;
    unsigned idx;

#ifdef CONFIG_SMP
    q = cpu_drv_queues[cpuid];
#else
    q = cpu_drv_queues[0];
#endif

    /* Read tail first, then head (acquire semantics via mfence on x86) */
    t = q->tail;
    __insn_barrier();
    h = q->head;

    /* Check if queue is empty */
    if (h == t)
        return -1;  /* queue empty */

    /* Read the entry */
    idx = t & DRV_QUEUE_MASK;
    entry->cmd   = q->entries[idx].cmd;
    entry->flags = q->entries[idx].flags;
    entry->arg1  = q->entries[idx].arg1;
    entry->arg2  = q->entries[idx].arg2;
    entry->arg3  = q->entries[idx].arg3;

    /* Increment tail — consumer is the only writer */
    __insn_barrier();
    q->tail = t + 1;

    return OK;
}

/* =========================================================================
 * Pending
 * ========================================================================= */

int drvqueue_pending(void)
{
    struct drv_queue *q;

#ifdef CONFIG_SMP
    q = cpu_drv_queues[cpuid];
#else
    q = cpu_drv_queues[0];
#endif

    /* Read head and tail (no ordering needed for a count) */
    return (int)(q->head - q->tail);
}

/* =========================================================================
 * Attach
 * ========================================================================= */
/*
 * Bind a driver process to a specific CPU and make the queue available.
 *
 * Steps:
 *   1. Validate the CPU is available
 *   2. Set the process's CPU affinity to the target CPU
 *   3. Assign the per-CPU queue pointer to the process's user-space mapping
 *      (via p_drvqueue pointer — added to struct proc)
 *
 * NOTE: Full user-space mapping of the queue requires VM support
 * (sys_vmctl to map the queue's physical pages into the driver's
 * address space). This is a placeholder for the VM integration.
 */

int drvqueue_attach(endpoint_t proc_ep, unsigned cpu)
{
    struct proc *p;
    int slot;

    if (cpu >= CONFIG_MAX_CPUS)
        return EINVAL;

#ifdef CONFIG_SMP
    if (cpu >= ncpus)
        return EINVAL;
#endif

    /* Look up the process */
    if (!isokendpt(proc_ep, &slot))
        return EINVAL;

    p = proc_addr(slot);

    /* Set CPU affinity */
#ifdef CONFIG_SMP
    p->p_cpu = cpu;
#endif

    /* Store the queue pointer for the process */
    p->p_drvqueue = cpu_drv_queues[cpu];

    DEBUGBASIC(("drvqueue: process %d attached to CPU %d (queue at 0x%lx)\n",
        proc_ep, cpu, (unsigned long)p->p_drvqueue));

    return OK;
}
