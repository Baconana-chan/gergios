/* irq_thread.h — Per-IRQ kernel thread framework for MINIX (GergiOS)
 *
 * Provides per-IRQ ring 0 kernel threads with SCHED_FIFO priority.
 * Each IRQ vector gets a dedicated kernel thread that runs at CPL 0
 * with its own kernel stack and struct proc entry.
 *
 * Design:
 *   - irq_thread_init() creates NR_IRQ_THREADS (64) ring 0 processes
 *   - Each has: struct proc, priv structure, 4KB kernel stack
 *   - Each runs irq_thread_main() at CPL 0 (ring 0)
 *   - irq_thread_main() calls mini_receive() to wait for IRQ notifications
 *   - When an IRQ fires, irq_thread_signal() calls mini_notify() to wake
 *     the appropriate thread
 *   - The thread receives the notification, runs the handler, loops
 *
 * Context switching:
 *   - Kthreads save/restore context via irq_thread_yield()
 *   - Before blocking on mini_receive, save p_reg.sp (RSP) and p_reg.pc (RIP)
 *   - When rescheduled, restore_kthread_context() sets RSP and IRETQ to ring 0
 *
 * IRQ → RT priority mapping:
 *   IRQ 0  (timer)       → SCHED_FIFO prio 99
 *   IRQ 15 (keyboard)    → SCHED_FIFO prio 85
 *   IRQ 48-63 (MSI-X)    → SCHED_FIFO prio 51-36
 *
 * Statistics exported via SYS_GETINFO (GET_IRQTHREAD_STATS).
 */

#ifndef IRQ_THREAD_H
#define IRQ_THREAD_H

#include "proc.h"
#include <minix/type.h>

/* Stack size for each IRQ thread (4KB = one page). */
#define IRQ_THREAD_STACK_SIZE   4096

/* IRQ → RT priority mapping constants. */
#define IRQ_PRIO_MIN      1
#define IRQ_PRIO_MAX      99

/* Per-IRQ thread statistics structure.
 * Exported to userspace via SYS_GETINFO / GET_IRQTHREAD_STATS.
 * Latency is measured in TSC ticks (conversion: ns = tsc_ticks / cpu_mhz). */
struct irq_thread_stats {
    int             irq;            /* IRQ vector number (0-63) */
    int             rt_prio;        /* SCHED_FIFO priority (1-99) */
    int             registered;     /* 1 if slot is in use */
    endpoint_t      endpoint;       /* IRQ thread endpoint */
    uint64_t        handled_count;  /* total IRQs handled */
    uint64_t        run_count;      /* total times IRQ thread ran the handler.
                                     * At SCHED_FIFO priority, each run means
                                     * it preempted a lower-prio process. */
    uint64_t        last_latency;   /* last handler latency (TSC ticks) */
    uint64_t        max_latency;    /* max handler latency (TSC ticks) */
    uint64_t        total_latency;  /* sum of all latencies (TSC ticks) */
};

/* Per-IRQ thread descriptor. */
struct irq_thread {
    int             irq;            /* IRQ vector number (0-63) */
    int             rt_prio;        /* SCHED_FIFO priority (1-99) */
    int             endpoint;       /* endpoint of the IRQ thread process */
    irq_handler_t   handler;        /* handler function to call */
    int             registered;     /* 1 if slot is in use */
    struct proc     *proc;          /* pointer to proc table entry */

    /* Statistics */
    uint64_t        handled_count;  /* total IRQs handled */
    uint64_t        run_count;      /* total times handler was invoked */
    volatile uint64_t signal_tsc;   /* TSC at signal time (for latency calc).
                                     * volatile: written from interrupt
                                     * context, read from thread context. */
    uint64_t        last_latency;   /* last handler latency (TSC ticks) */
    uint64_t        max_latency;    /* max handler latency (TSC ticks) */
    uint64_t        total_latency;  /* sum of all latencies (TSC ticks) */
};

/* Initialize the IRQ thread framework.
 * Called once during kernel init (from arch_init).
 * Creates NR_IRQ_THREADS ring 0 processes. */
void irq_thread_init(void);

/* Register an IRQ handler for a specific IRQ vector.
 * The handler runs in the IRQ thread's context (ring 0) when the IRQ fires. */
int irq_thread_register(int irq, irq_handler_t handler);

/* Signal an IRQ thread that its IRQ has fired.
 * Called from irq_handle() in interrupt context.
 * Wakes the IRQ thread via mini_notify(). */
void irq_thread_signal(int irq);

/* Fill an irq_thread_stats array from the internal table.
 * Used by do_getinfo() for GET_IRQTHREAD_STATS. */
void irq_thread_get_stats(struct irq_thread_stats *stats, size_t max);

/* Set the SCHED_FIFO priority of an IRQ thread.
 * Called from do_irqctl() when a driver requests a specific priority
 * (e.g. SCHED_FIFO 90 for storage controllers). */
int irq_thread_set_priority(int irq, int prio);

/* Generic device IRQ handler registered by do_irqctl() when a userspace
 * driver calls sys_irqsetpolicy(). Runs in the IRQ thread context
 * at the assigned SCHED_FIFO priority. Does NOT re-notify the driver
 * — that is already done by the main hook chain in irq_handle().
 *
 * If MMIO is registered for this IRQ via irq_thread_set_mmio(), the handler
 * performs a device-level fast-ack: reads the interrupt status register
 * (assumed to be at offset 0 from the mapped base) and clears all pending
 * interrupts. This reduces interrupt delivery latency by acknowledging
 * at the device level before the userspace driver gets the notification. */
int irq_thread_device_handler(irq_hook_t *hook);

/* Unregister an IRQ thread and clean up its resources.
 * Clears the handler, resets registration, and if MMIO was mapped
 * for this IRQ, unmaps the kernel page via pg_unmap_page().
 * Called from do_irqctl() IRQ_RMPOLICY. */
int irq_thread_unregister(int irq);

/* Register a device MMIO physical address for kernel-level fast-ack.
 * The kernel maps one page starting at phys_addr into its address space.
 * On IRQ, irq_thread_device_handler() reads the u32_t at HBA_IS_OFFSET
 * (typically 2 for AHCI) and writes ~0 to clear all pending bits.
 *
 * Must be called before the IRQ fires (typically during driver init,
 * after sys_irqsetpolicy()). */
int irq_thread_set_mmio(int irq, phys_bytes phys_addr);

#endif /* IRQ_THREAD_H */
