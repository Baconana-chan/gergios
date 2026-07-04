/* drvqueue.h — Per-CPU driver command queues (lock-free SPSC).
 *
 * Each CPU has a single-producer, single-consumer ring buffer used
 * to deliver commands from device interrupts (or other CPUs) to a
 * driver pinned to that CPU.
 *
 * Producers:
 *   - The kernel interrupt handler (irq_handle → drvqueue_push on MSI-X IRQs)
 *   - Another CPU (via IPI + drvqueue_push)
 *
 * Consumer:
 *   - The driver process pinned to this CPU (reads via drvqueue_pop)
 *
 * The queue is lock-free SPSC:
 *   - head (producer index) written only by producers (atomic increment)
 *   - tail (consumer index) written only by consumer (plain store)
 *   - No locking needed as long as SPSC invariant is maintained
 *
 * Queue capacity is DRV_QUEUE_SIZE; push returns -1 if full.
 */

#ifndef _DRVQUEUE_H
#define _DRVQUEUE_H

#include "kernel/kernel.h"

/* Queue size — must be a power of 2 for efficient masking */
#define DRV_QUEUE_SHIFT     8
#define DRV_QUEUE_SIZE      (1 << DRV_QUEUE_SHIFT)  /* 256 */
#define DRV_QUEUE_MASK      (DRV_QUEUE_SIZE - 1)

/* Command flags */
#define DRVQ_F_NONE         0x00
#define DRVQ_F_URGENT       0x01  /* high-priority command */
#define DRVQ_F_MSIX         0x02  /* originated from MSI-X interrupt */

/* Predefined command codes */
#define DRVQ_CMD_NOP        0x00  /* no operation (queue filler) */
#define DRVQ_CMD_IRQ        0x01  /* device interrupt notification */
#define DRVQ_CMD_IO         0x02  /* I/O completion */
#define DRVQ_CMD_SCHED      0x03  /* scheduler migration */
#define DRVQ_CMD_USER       0x10  /* first user-defined command */

/* Queue entry — 32 bytes, cache-line-friendly */
struct drv_queue_entry {
    u32_t       cmd;        /* command code */
    u32_t       flags;      /* DRVQ_F_* */
    u64_t       arg1;       /* command-specific argument */
    u64_t       arg2;       /* command-specific argument */
    u64_t       arg3;       /* command-specific argument */
};

/*
 * Per-CPU driver queue.
 *
 * Lock-free SPSC ring buffer:
 *   head = producer index (may be ahead of tail by at most DRV_QUEUE_SIZE)
 *   tail = consumer index (never ahead of head)
 *
 * Empty:   head == tail
 * Full:    head - tail >= DRV_QUEUE_SIZE
 * Entries: entries[tail & mask] .. entries[(head-1) & mask]
 */
struct drv_queue {
    /* Producer/consumer indices — must be in separate cache lines */
    atomic_t    head;       /* written by producers (INCREMENT only) */
    char        _pad1[60];  /* padding to 64-byte cache line */
    atomic_t    tail;       /* written by consumer (driver) */
    char        _pad2[60];  /* padding to 64-byte cache line */

    /* Ring buffer */
    struct drv_queue_entry entries[DRV_QUEUE_SIZE];
} __attribute__((aligned(64)));

/* Per-CPU queue pointers — indexed by cpuid */
extern struct drv_queue *cpu_drv_queues[CONFIG_MAX_CPUS];

/* =========================================================================
 * API
 * ========================================================================= */

/*
 * Initialize all per-CPU driver queues.
 * Called once during kernel init (arch_init or smp_init).
 */
void drvqueue_init(void);

/*
 * Push a command onto the queue for the given CPU.
 * Returns OK on success, -1 if queue is full.
 *
 * Safe to call from interrupt context and from any CPU.
 */
int drvqueue_push(unsigned cpu, u32_t cmd, u32_t flags,
                  u64_t arg1, u64_t arg2, u64_t arg3);

/*
 * Pop a command from the local CPU's queue.
 * Returns OK on success, -1 if queue is empty.
 *
 * Should only be called by the driver process pinned to this CPU.
 */
int drvqueue_pop(struct drv_queue_entry *entry);

/*
 * Check if the local CPU's queue has pending commands.
 * Returns number of entries pending.
 */
int drvqueue_pending(void);

/*
 * Attach a process to the driver queue on a specific CPU.
 * Returns OK on success.
 *
 * This does two things:
 *   1. Sets the process's CPU affinity (p_cpu = cpu)
 *   2. Maps the queue into the process's address space (via VM grant)
 *
 * Called from the driver process during IRQ_DRVQUEUE_SETUP.
 */
int drvqueue_attach(endpoint_t proc_ep, unsigned cpu);

#endif /* _DRVQUEUE_H */
