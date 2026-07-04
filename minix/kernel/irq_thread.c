/* irq_thread.c — Per-IRQ ring 0 kernel thread implementation.
 *
 * Creates NR_IRQ_THREADS (64) ring 0 processes that handle IRQs
 * at SCHED_FIFO priority. Created dynamically during boot.
 *
 * Architecture:
 *   Each IRQ thread is a real ring 0 kernel thread with:
 *     - struct proc entry (p_nr = IRQ_THREAD_BASE + irq)
 *     - Privilege structure (TSK_F-like: kernel task flags)
 *     - Private 4KB kernel stack (static array, 64 x 4KB = 256KB)
 *     - SCHED_FIFO priority based on IRQ vector number
 *     - IPC-based IRQ notification (mini_notify from interrupt handler)
 *
 * Context save/restore:
 *   Before blocking, irq_thread_entry saves RSP, RIP, and callee-saved
 *   registers (RBX, RBP, R12-R15) to p_reg. When rescheduled,
 *   arch_system.c's restore_kthread_context() restores these registers
 *   and does IRETQ to ring 0.
 *
 * Statistics:
 *   Each irq_thread tracks:
 *     - handled_count: incremented after each successful handler call
 *     - preempt_count: incremented each time the handler runs (the thread
 *       preempts whatever was running before, due to SCHED_FIFO priority)
 *     - last/max/total_latency: measured in TSC ticks from irq_thread_signal()
 *       to handler entry in irq_thread_entry(). Exported via GET_IRQTHREAD_STATS.
 */

#include "irq_thread.h"
#include "sched_rt.h"
#include "proc.h"
#include "clock.h"
#include "arch_proto.h"
#include <string.h>
#include <minix/const.h>
#include <minix/endpoint.h>
#include <minix/syslib.h>
#include <minix/com.h>

/* Forward declarations */
static void irq_thread_yield(void) __attribute__((noreturn));

/* Table of registered IRQ threads, indexed by IRQ vector. */
static struct irq_thread irq_thread_table[NR_IRQ_THREADS];

/* Per-IRQ kernel stacks (4KB each, 64 threads = 256KB total). */
static char irq_thread_stacks[NR_IRQ_THREADS][IRQ_THREAD_STACK_SIZE]
    __attribute__((aligned(16)));

/*===========================================================================*
 *               IRQ thread MMIO mapping for device fast-ack                *
 *===========================================================================*/

/* Dynamically-allocated virtual pages for per-IRQ device MMIO access.
 * irq_thread_set_mmio() creates page table entries on-demand using
 * alloc_pagetable() (pre-allocated static buffer, no kernel_may_alloc
 * dependency) and remaps the PTE to point to the driver's MMIO physical
 * address. This allows the IRQ thread handler to perform device-level
 * interrupt acknowledgment directly from ring 0. */

/* Virtual address base for IRQ thread MMIO window.
 * Chosen below kern_vir_start in a range that is not used by user space.
 * Each page is 4KB; we support up to 4 concurrent device MMIO mappings. */
#define IRQ_THREAD_MMIO_BASE    0xFFFFF000UL
#define IRQ_THREAD_MMIO_PAGES   4

/* Per-IRQ MMIO state. */
struct irq_thread_mmio {
    int         valid;       /* 1 if MMIO is mapped for this IRQ */
    phys_bytes  phys_addr;   /* physical base address of device MMIO */
    vir_bytes   virt_addr;   /* kernel virtual address for access */
};

/* One MMIO slot per IRQ thread (NR_IRQ_THREADS = 64).
 * Only the first IRQ_THREAD_MMIO_PAGES IRQs actually get a mapped page. */
static struct irq_thread_mmio irq_thread_mmio[NR_IRQ_THREADS];

/* Number of pages successfully mapped (0 initially, grows on demand). */
static int irq_thread_mmio_pages_mapped = 0;

/* AHCI-specific constants for fast-ack.
 * AHCI_HBA_IS (Interrupt Status) is at port register offset 2
 * from the HBA base. Reading gives pending interrupt bits;
 * writing back the read value clears them (write-1-to-clear). */
#define AHCI_HBA_IS_OFFSET      2
#define AHCI_HBA_IS_MASK        0xFFFFFFFFUL

/*===========================================================================*
 *                      irq_thread_mmio_init                                *
 *===========================================================================*/
static void irq_thread_mmio_init(void)
{
    int i;
    for (i = 0; i < NR_IRQ_THREADS; i++)
        irq_thread_mmio[i].valid = 0;
}

/*===========================================================================*
 *                      irq_thread_set_mmio                                 *
 *===========================================================================*/
int irq_thread_set_mmio(int irq, phys_bytes phys_addr)
{
    int page_idx;
    vir_bytes vaddr;
    int r;

    if (irq < 0 || irq >= NR_IRQ_THREADS)
        return EINVAL;

    /* Assign a free MMIO slot (round-robin). */
    page_idx = irq % IRQ_THREAD_MMIO_PAGES;
    vaddr = IRQ_THREAD_MMIO_BASE + page_idx * X86_64_PAGE_SIZE;

    /* Ensure page table entries exist for vaddr.
     * If the PDE doesn't have a page table allocated, allocate one.
     * alloc_pagetable() uses a pre-allocated static buffer, so it is
     * safe to call even after kernel_may_alloc is cleared. */
    {
        int pde = X86_64_VM_PDE(vaddr);
        extern u64_t pagedir[];

        if (!(pagedir[pde] & X86_64_VM_PRESENT)) {
            phys_bytes pt_phys;
            u64_t *pt = alloc_pagetable(&pt_phys);
            if (!pt)
                return ENOMEM;
            memset(pt, 0, X86_64_PAGE_SIZE);
            pagedir[pde] = (pt_phys & X86_64_VM_ADDR_MASK) |
                            X86_64_VM_PRESENT | X86_64_VM_WRITE;
            pg_load();
        }
    }

    /* Remap the PTE to point to the device's MMIO region. */
    r = pg_remap_page(vaddr, phys_addr);
    if (r != OK) {
        printf("IRQ thread MMIO: pg_remap_page failed (%d) for irq %d\n",
            r, irq);
        return r;
    }

    /* Track highest mapped page index. */
    if (page_idx + 1 > irq_thread_mmio_pages_mapped)
        irq_thread_mmio_pages_mapped = page_idx + 1;

    irq_thread_mmio[irq].valid = 1;
    irq_thread_mmio[irq].phys_addr = phys_addr;
    irq_thread_mmio[irq].virt_addr = vaddr;

    DEBUGBASIC(("IRQ thread MMIO: IRQ %d mapped at vaddr 0x%lx (phys 0x%lx)\n",
        irq, (unsigned long)vaddr, (unsigned long)phys_addr));

    return OK;
}



/*===========================================================================*
 *                      irq_thread_entry                                    *
 *===========================================================================*/
static void irq_thread_entry(void)
{
    struct proc *self = get_cpulocal_var(proc_ptr);
    int irq = -1;

    /* Determine IRQ from process name "irqd_NN" */
    {
        const char *name = self->p_name;
        if (name[0] == 'i' && name[1] == 'r' && name[2] == 'q' &&
            name[3] == 'd' && name[4] == '_') {
            irq = (name[5] - '0') * 10 + (name[6] - '0');
        }
    }

    for (;;) {
        message msg;
        int r;
        struct irq_thread *it = (irq >= 0 && irq < NR_IRQ_THREADS)
            ? &irq_thread_table[irq] : NULL;

        /* Save callee-saved registers + RSP + RIP.
         * When rescheduled, restore_kthread_context loads these
         * from p_reg and resumes execution at resume_point. */
        __asm__ volatile(
            "movq %%rsp, %0\n\t"
            "movq %%rbx, %1\n\t"
            "movq %%rbp, %2\n\t"
            "movq %%r12, %3\n\t"
            "movq %%r13, %4\n\t"
            "movq %%r14, %5\n\t"
            "movq %%r15, %6\n\t"
            "leaq 1f(%%rip), %7\n\t"
            : "=m" (self->p_reg.sp),
              "=m" (self->p_reg.rbx),
              "=m" (self->p_reg.rbp),
              "=m" (self->p_reg.r12),
              "=m" (self->p_reg.r13),
              "=m" (self->p_reg.r14),
              "=m" (self->p_reg.r15),
              "=m" (self->p_reg.pc)
            :
            : "memory"
        );

        /* Wait for an IRQ notification. May block (RTS_RECEIVING).
         * After blocking, irq_thread_yield() calls switch_to_user(). */
        r = mini_receive(self, ANY, &msg, 0);

        /* Resume point: after being woken and rescheduled,
         * execution continues here. */
        __asm__("1:");

        if (r != OK)
            goto done;

        /* Handle the IRQ notification. */
        if (msg.m_type == NOTIFY_MESSAGE && it &&
            it->registered && it->handler) {
            u64_t entry_tsc;
            irq_hook_t hook;

            /* Record entry TSC for latency measurement. */
            read_tsc_64(&entry_tsc);

            /* Compute latency from signal time to handler entry. */
            if (it->signal_tsc != 0) {
                u64_t delta = entry_tsc - it->signal_tsc;
                it->last_latency = delta;
                if (delta > it->max_latency)
                    it->max_latency = delta;
                it->total_latency += delta;
                it->signal_tsc = 0;  /* consumed */
            }

            /* Call the actual IRQ handler. */
            memset(&hook, 0, sizeof(hook));
            hook.irq = irq;
            hook.handler = it->handler;
            it->handler(&hook);

            /* Update counts. run_count is incremented because at
             * SCHED_FIFO priority, waking this thread typically preempted
             * whatever was running on this CPU (unless a higher-prio RT
             * task was already running). */
            it->handled_count++;
            it->run_count++;
        }

done:
        /* Yield to the scheduler. Never returns — execution continues
         * from resume point when rescheduled. */
        irq_thread_yield();
    }
}

/*===========================================================================*
 *                      irq_thread_yield                                    *
 *===========================================================================*/
static void irq_thread_yield(void)
{
    /* switch_to_user() selects the next process. If blocked
     * (RTS_RECEIVING), another process runs until woken by notify. */
    switch_to_user();
    __builtin_unreachable();
}

/*===========================================================================*
 *                      irq_thread_init                                     *
 *===========================================================================*/
void irq_thread_init(void)
{
    int i;

    DEBUGBASIC(("IRQ thread init: creating %d ring 0 threads...\n",
        NR_IRQ_THREADS));

    /* Initialize the IRQ thread table with priority mapping. */
    for (i = 0; i < NR_IRQ_THREADS; i++) {
        irq_thread_table[i].irq = i;
        irq_thread_table[i].rt_prio = IRQ_PRIO_MAX -
            (i * (IRQ_PRIO_MAX - IRQ_PRIO_MIN) / (NR_IRQ_THREADS - 1));
        irq_thread_table[i].endpoint = NONE;
        irq_thread_table[i].handler = NULL;
        irq_thread_table[i].registered = 0;
        irq_thread_table[i].proc = NULL;
        irq_thread_table[i].handled_count = 0;
        irq_thread_table[i].run_count = 0;
        irq_thread_table[i].signal_tsc = 0;
        irq_thread_table[i].last_latency = 0;
        irq_thread_table[i].max_latency = 0;
        irq_thread_table[i].total_latency = 0;
    }

    for (i = 0; i < NR_IRQ_THREADS; i++) {
        struct proc *rp;
        int proc_nr = IRQ_THREAD_BASE + i;
        int r;

        rp = proc_addr(proc_nr);

        if (!isemptyn(proc_nr)) {
            printf("IRQ thread %d: slot %d in use!\n", i, proc_nr);
            continue;
        }

        memset(rp, 0, sizeof(*rp));

        rp->p_magic = PMAGIC;
        rp->p_nr = proc_nr;
        rp->p_endpoint = _ENDPOINT(0, proc_nr);
        rp->p_cpu = cpuid;
        rp->p_rts_flags = RTS_SLOT_FREE;
        rp->p_kthread = 1;

        /* Process name: "irqd_NN" */
        {
            char *name = rp->p_name;
            name[0] = 'i'; name[1] = 'r'; name[2] = 'q'; name[3] = 'd';
            name[4] = '_';
            name[5] = '0' + (i / 10);
            name[6] = '0' + (i % 10);
            name[7] = '\0';
        }

        /* Allocate privilege structure dynamically */
        r = get_priv(rp, NULL_PRIV_ID);
        if (r != OK) {
            printf("IRQ thread %d: get_priv failed (%d)\n", i, r);
            continue;
        }

        priv(rp)->s_flags = SYS_PROC | PREEMPTIBLE;
        priv(rp)->s_trap_mask = CSK_T;
        priv(rp)->s_sig_mgr = SELF;

        /* IPC mask: allow receiving from HARDWARE, sending to SYSTEM */
        {
            sys_map_t map;
            memset(&map, 0, sizeof(map));
            set_sys_bit(map, priv(proc_addr(SYSTEM))->s_id);
            fill_sendto_mask(rp, &map);
        }

        for (int j = 0; j < SYS_CALL_MASK_SIZE; j++)
            priv(rp)->s_k_call_mask[j] = ~0;

        /* Arch reset (FPU state) */
        arch_proc_reset(rp);

        /* Set up ring 0 context */
        {
            struct stackframe_s reg;
            memset(&reg, 0, sizeof(reg));
            reg.psw = INIT_TASK_PSW | IF_MASK | 0x3000;
            reg.cs = KERN_CS_SELECTOR;
            reg.ss = KERN_DS_SELECTOR;
            reg.pc = (reg_t) irq_thread_entry;
            reg.sp = (reg_t)
                &irq_thread_stacks[i][IRQ_THREAD_STACK_SIZE - 16];
            arch_proc_setcontext(rp, &reg, 0, KTS_FULLCONTEXT);
        }

        /* SCHED_FIFO priority based on IRQ vector */
        {
            int prio = irq_thread_table[i].rt_prio;
            r = sched_rt_set_class(rp, SCHED_FIFO, prio, 0);
            if (r != OK)
                printf("IRQ thread %d: sched_rt_set_class failed (%d)\n",
                    i, r);
        }

        rp->p_quantum_size_ms = 0;
        rp->p_cpu_time_left = ms_2_cpu_time(1000000);

        /* Mark runnable — clear all inhibit flags */
        rp->p_rts_flags &= ~(RTS_SLOT_FREE | RTS_VMINHIBIT |
                             RTS_BOOTINHIBIT | RTS_PROC_STOP);

        irq_thread_table[i].proc = rp;
        irq_thread_table[i].endpoint = rp->p_endpoint;

        DEBUGBASIC(("  irqd_%02d: nr=%d ep=%d prio=%d\n",
            i, proc_nr, rp->p_endpoint, irq_thread_table[i].rt_prio));
    }

    DEBUGBASIC(("IRQ thread init: done (%d threads)\n", NR_IRQ_THREADS));

    /* Pre-allocate page table entries for device-level fast-ack MMIO.
     * This must be done before kernel_may_alloc is cleared. */
    irq_thread_mmio_init();
}

/*===========================================================================*
 *                      irq_thread_register                                 *
 *===========================================================================*/
int irq_thread_register(int irq, irq_handler_t handler)
{
    if (irq < 0 || irq >= NR_IRQ_THREADS)
        return EINVAL;
    if (!handler)
        return EINVAL;
    irq_thread_table[irq].handler = handler;
    irq_thread_table[irq].registered = 1;
    return OK;
}

/*===========================================================================*
 *                      irq_thread_signal                                   *
 *===========================================================================*/
void irq_thread_signal(int irq)
{
    struct irq_thread *it;

    if (irq < 0 || irq >= NR_IRQ_THREADS)
        return;

    it = &irq_thread_table[irq];

    if (!it->registered || it->endpoint == NONE)
        return;

    /* Record TSC timestamp for latency measurement.
     * This is called from irq_handle() in interrupt context,
     * so we capture the earliest possible time. */
    read_tsc_64(&it->signal_tsc);

    mini_notify(proc_addr(HARDWARE), it->endpoint);
}

/*===========================================================================*
 *                      irq_thread_get_stats                                *
 *===========================================================================*/
void irq_thread_get_stats(struct irq_thread_stats *stats, size_t max)
{
    int i;
    size_t n = (max < NR_IRQ_THREADS) ? max : NR_IRQ_THREADS;

    for (i = 0; i < n; i++) {
        struct irq_thread *it = &irq_thread_table[i];
        stats[i].irq = it->irq;
        stats[i].rt_prio = it->rt_prio;
        stats[i].registered = it->registered;
        stats[i].endpoint = it->endpoint;
        stats[i].handled_count = it->handled_count;
        stats[i].run_count = it->run_count;
        stats[i].last_latency = it->last_latency;
        stats[i].max_latency = it->max_latency;
        stats[i].total_latency = it->total_latency;
    }
}

/*===========================================================================*
 *                      irq_thread_unregister                               *
 *===========================================================================*/
int irq_thread_unregister(int irq)
{
    if (irq < 0 || irq >= NR_IRQ_THREADS)
        return EINVAL;

    /* Clear the handler and registration. */
    irq_thread_table[irq].handler = NULL;
    irq_thread_table[irq].registered = 0;

    /* Clean up MMIO mapping if one was registered. */
    if (irq_thread_mmio[irq].valid) {
        vir_bytes vaddr = irq_thread_mmio[irq].virt_addr;

        if (vaddr != 0) {
            int r = pg_unmap_page(vaddr);
            if (r != OK)
                printf("IRQ thread %d: pg_unmap_page(0x%lx) failed (%d)\n",
                    irq, (unsigned long)vaddr, r);
        }

        irq_thread_mmio[irq].valid = 0;
        irq_thread_mmio[irq].phys_addr = 0;
        irq_thread_mmio[irq].virt_addr = 0;

        DEBUGBASIC(("IRQ thread %d: MMIO unmapped\n", irq));
    }

    return OK;
}

/*===========================================================================*
 *                      irq_thread_set_priority                             *
 *===========================================================================*/
int irq_thread_set_priority(int irq, int prio)
{
    struct irq_thread *it;
    int r;

    if (irq < 0 || irq >= NR_IRQ_THREADS)
        return EINVAL;
    if (prio < IRQ_PRIO_MIN || prio > IRQ_PRIO_MAX)
        return EINVAL;

    it = &irq_thread_table[irq];
    if (!it->proc)
        return ESRCH;

    /* Update the internal priority and apply via sched_rt_set_class. */
    it->rt_prio = prio;
    r = sched_rt_set_class(it->proc, SCHED_FIFO, prio, 0);
    if (r != OK)
        printf("IRQ thread %d: sched_rt_set_priority failed (%d)\n", irq, r);

    return r;
}

/*===========================================================================*
 *                      irq_thread_device_handler                           *
 *===========================================================================*/
int irq_thread_device_handler(irq_hook_t *hook)
{
    /* Lightweight IRQ thread handler for device IRQs registered by
     * userspace drivers. Runs in the IRQ thread's context at the
     * assigned SCHED_FIFO priority.
     *
     * This handler does NOT re-notify the userspace driver — the
     * main hook chain in irq_handle() already handles that via
     * generic_handler in do_irqctl.c.
     *
     * The value of this handler is:
     *   1. The IRQ thread exists and can be scheduled at RT priority
     *   2. Latency statistics are tracked (done in irq_thread_entry)
     *   3. Kernel-level fast-ack: if MMIO is mapped for this IRQ,
     *      read the device's interrupt status register (at HBA_IS
     *      offset) and clear all pending bits. This reduces latency
     *      by acknowledging at the device level before the userspace
     *      driver processes the interrupt.
     */
    int irq;

    if (!hook)
        return 1;

    irq = hook->irq;

    /* Fast-ack: if MMIO is mapped for this IRQ, read and clear
     * the interrupt status register at the device level. */
    if (irq >= 0 && irq < NR_IRQ_THREADS && irq_thread_mmio[irq].valid) {
        vir_bytes vaddr = irq_thread_mmio[irq].virt_addr;

        if (vaddr != 0) {
            volatile u32_t *is_reg =
                (volatile u32_t *)(vaddr + AHCI_HBA_IS_OFFSET * sizeof(u32_t));
            u32_t pending = *is_reg;

            if (pending & AHCI_HBA_IS_MASK) {
                /* Clear all pending interrupt bits by writing back
                 * the value we read (AHCI: write-1-to-clear). */
                *is_reg = pending;
            }
        }
    }

    return 1; /* IRQ handled — allow re-enable */
}


