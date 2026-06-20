/* ============================================================
 * arch_timer.c — ARM64 Generic Timer driver
 *
 * Implements the ARM Generic Timer interface for the MINIX kernel.
 * The ARM Generic Timer provides a system counter and per-CPU
 * timers accessed via system registers.
 *
 * Timer registers used:
 *   CNTPCT_EL0   — Physical Count Register (64-bit, monotonic)
 *   CNTP_CVAL_EL0 — Physical Timer Compare Value
 *   CNTP_TVAL_EL0 — Physical Timer Tick Value (32-bit)
 *   CNTP_CTL_EL0 — Physical Timer Control Register
 *   CNTFRQ_EL0   — Counter Frequency Register
 *
 * The physical timer (CNTP) generates PPI interrupt ID 30 (INTID 30)
 * on the GIC. This is the standard timer for Non-Secure EL1 code.
 *
 * References:
 *   ARM DDI 0487, Chapter D10 — Generic Timer
 *   ARM IHI 0048, Chapter 3 — GIC Interrupt Assignments
 * ============================================================ */

#include "kernel/kernel.h"
#include "kernel/clock.h"
#include "kernel/interrupt.h"
#include "kernel/glo.h"
#include "kernel/profile.h"
#include "kernel/spinlock.h"

#include <minix/u64.h>
#include <minix/board.h>

#include <sys/sched.h> /* for CP_*, CPUSTATES */
#if CPUSTATES != MINIX_CPUSTATES
#error "MINIX_CPUSTATES value is out of sync with NetBSD's!"
#endif

/* =========================================================================
 * Barrier macro (ARM64 inline asm)
 *
 * Used after MSR writes to system registers to ensure completion
 * before subsequent instructions execute.
 * ========================================================================= */

#define timer_isb()  __asm__ __volatile__("isb" : : : "memory")

/* =========================================================================
 * ARM Generic Timer register access (inline assembly)
 * ========================================================================= */

/* Physical Count Register (64-bit, monotonic system counter) */
static inline uint64_t read_cntpct(void)
{
	uint64_t val;
	__asm__ volatile("mrs %0, CNTPCT_EL0" : "=r"(val));
	return val;
}

/* Physical Timer Compare Value (64-bit) */
static inline void write_cntp_cval(uint64_t val)
{
	__asm__ volatile("msr CNTP_CVAL_EL0, %0" : : "r"(val));
	timer_isb();
}

/* Physical Timer Tick Value (32-bit, downcounter) */
static inline void write_cntp_tval(uint32_t val)
{
	__asm__ volatile("msr CNTP_TVAL_EL0, %0" : : "r"(val));
	timer_isb();
}

/* Physical Timer Control Register */
static inline uint32_t read_cntp_ctl(void)
{
	uint32_t val;
	__asm__ volatile("mrs %0, CNTP_CTL_EL0" : "=r"(val));
	return val;
}

static inline void write_cntp_ctl(uint32_t val)
{
	__asm__ volatile("msr CNTP_CTL_EL0, %0" : : "r"(val));
	timer_isb();
}

/* Counter Frequency Register (reads the system timer frequency) */
static inline uint32_t read_cntfrq(void)
{
	uint32_t val;
	__asm__ volatile("mrs %0, CNTFRQ_EL0" : "=r"(val));
	return val;
}

/* CNTP_CTL_EL0 fields */
#define CNTP_CTL_ENABLE         (1U << 0)   /* Timer enable */
#define CNTP_CTL_IMASK          (1U << 1)   /* Interrupt mask (1=disabled) */
#define CNTP_CTL_ISTATUS        (1U << 2)   /* Interrupt status (RO) */

/* =========================================================================
 * Constants
 * ========================================================================= */

/* The ARM Generic Timer physical timer is PPI ID 14 = INTID 30 on GIC.
 * INTID 30 = PPI base (16) + PPI index (14).
 * Reference: ARM IHI 0048, Table 3-2 (GIC v2/v3 PPI interrupt assignments). */
#define ARM64_TIMER_IRQ         30

/* Default timer frequency if CNTFRQ_EL0 returns 0 (shouldn't happen).
 * QEMU virt typically uses 62.5 MHz (62,500,000 Hz). */
#define DEFAULT_TIMER_FREQ      62500000UL

/* =========================================================================
 * Static state
 * ========================================================================= */

static irq_hook_t timer_hook;               /* Interrupt handler hook */

static unsigned tsc_per_ms[CONFIG_MAX_CPUS];    /* TSC ticks per millisecond */
static unsigned tsc_per_tick[CONFIG_MAX_CPUS];  /* TSC ticks per kernel tick */
static uint64_t tsc_per_state[CONFIG_MAX_CPUS][CPUSTATES]; /* Per-state cycle accumulation */

static uint32_t timer_freq;                     /* System timer frequency from CNTFRQ_EL0 */

/* =========================================================================
 * Helper: 64-bit counter read for MINIX kernel
 *
 * The MINIX kernel uses read_tsc(u32_t *high, u32_t *low) for 32-bit
 * archs and read_tsc_64(u64_t *val) for 64-bit archs. Since ARM64's
 * CNTPCT_EL0 is a single 64-bit register, we define both helpers.
 * ========================================================================= */

/* 64-bit read (primary interface used by cycle accounting) */
static inline void read_cntpct_64(uint64_t *val)
{
	*val = read_cntpct();
}

/* 32-bit split read (required by proto.h: read_tsc(u32_t*, u32_t*)) */
void read_tsc(uint32_t *high, uint32_t *low)
{
	uint64_t val = read_cntpct();
	*high = (uint32_t)(val >> 32);
	*low  = (uint32_t)(val & 0xFFFFFFFF);
}

/* =========================================================================
 * init_local_timer — Initialize the ARM Generic Timer
 *
 * Sets up the timer to fire at the given frequency by programming
 * the CNTP_TVAL_EL0 downcounter. The timer automatically reloads
 * TVAL from the new compare value on each trigger.
 *
 * Parameters:
 *   freq  Desired timer interrupt frequency in Hz.
 *
 * Returns:
 *   0 on success.
 * ========================================================================= */

int init_local_timer(unsigned freq)
{
	unsigned cpu = cpuid;
	uint32_t ticks_per_tick;

	/* Read the system counter frequency.
	 * On QEMU virt, this is typically 62.5 MHz.
	 * On real hardware, it's set by firmware/bootloader. */
	timer_freq = read_cntfrq();
	if (timer_freq == 0) {
		printf("WARNING: CNTFRQ_EL0 returned 0, using default %lu Hz\n",
		       (unsigned long)DEFAULT_TIMER_FREQ);
		timer_freq = DEFAULT_TIMER_FREQ;
	}

	BOOT_VERBOSE(printf("ARM Generic Timer: CNTFRQ_EL0 = %u Hz\n", timer_freq));

	/* Calculate ticks per millisecond and per kernel tick */
	tsc_per_ms[cpu] = timer_freq / 1000;
	tsc_per_tick[cpu] = timer_freq / freq;

	BOOT_VERBOSE(printf("Timer: %u ticks/ms, %u ticks/tick (%u Hz)\n",
			    tsc_per_ms[cpu], tsc_per_tick[cpu], freq));

	/* Program the tick value.
	 * CNTP_TVAL_EL0 is a 32-bit downcounter that fires an interrupt
	 * when it reaches 0, then auto-reloads. */
	ticks_per_tick = timer_freq / freq;
	write_cntp_tval(ticks_per_tick);

	/* Enable the timer and unmask the interrupt.
	 * CNTP_CTL_EL0: ENABLE=1, IMASK=0 (interrupts enabled) */
	write_cntp_ctl(CNTP_CTL_ENABLE);

	return 0;
}

/* =========================================================================
 * stop_local_timer — Stop the local timer
 *
 * Disables the physical timer by clearing the ENABLE bit.
 * The counter continues running (it's always on), but no timer
 * interrupts will be generated.
 * ========================================================================= */

void stop_local_timer(void)
{
	write_cntp_ctl(0);
}

/* =========================================================================
 * restart_local_timer — Restart the local timer
 *
 * Re-enables the timer with the last programmed tick value.
 * The timer interrupt will fire again after the tick interval.
 * ========================================================================= */

void restart_local_timer(void)
{
	write_cntp_ctl(CNTP_CTL_ENABLE);
}

/* =========================================================================
 * register_local_timer_handler — Register the timer interrupt handler
 *
 * Registers the given handler function to be called on each timer
 * interrupt. The handler is hooked to IRQ 30 (ARM Generic Timer PPI).
 *
 * Parameters:
 *   handler  The interrupt handler function (typically timer_int_handler).
 *
 * Returns:
 *   0 on success.
 * ========================================================================= */

int register_local_timer_handler(const irq_handler_t handler)
{
	timer_hook.proc_nr_e = NONE;
	timer_hook.irq = ARM64_TIMER_IRQ;

	put_irq_handler(&timer_hook, ARM64_TIMER_IRQ, handler);

	return 0;
}

/* =========================================================================
 * arch_timer_int_handler — Arch-specific timer interrupt post-processing
 *
 * Called from timer_int_handler() in clock.c after generic tick processing.
 * On ARM64, no additional action is required here — the hardware
 * automatically handles the timer reload.
 * ========================================================================= */

void arch_timer_int_handler(void)
{
	/* No-op: ARM Generic Timer auto-reloads TVAL, and GICv3 EOIR
	 * is handled by the generic bsp_irq_handle() flow. */
}

/* =========================================================================
 * cycles_accounting_init — Initialize per-CPU cycle accounting
 *
 * Records the initial counter value as the switch point for
 * context_stop() differential cycle counting.
 * ========================================================================= */

void cycles_accounting_init(void)
{
#ifdef CONFIG_SMP
	unsigned cpu = cpuid;
#endif

	read_cntpct_64(get_cpu_var_ptr(cpu, tsc_ctr_switch));

	get_cpu_var(cpu, cpu_last_tsc) = 0;
	get_cpu_var(cpu, cpu_last_idle) = 0;
}

/* =========================================================================
 * context_stop — Stop cycle accounting for a process
 *
 * Called when the kernel is about to switch from the current process
 * to another. Computes the delta cycles since the last switch point
 * and attributes them to the outgoing process.
 *
 * Parameters:
 *   p  The process to stop accounting for.
 *
 * Reference: See i386/arch_clock.c for the canonical implementation.
 * ========================================================================= */

void context_stop(struct proc *p)
{
	uint64_t tsc, tsc_delta;
	uint64_t *__tsc_ctr_switch = get_cpulocal_var_ptr(tsc_ctr_switch);
	unsigned int cpu, tpt, counter;

	read_cntpct_64(&tsc);
	p->p_cycles = p->p_cycles + tsc - *__tsc_ctr_switch;
	cpu = cpuid;

	tsc_delta = tsc - *__tsc_ctr_switch;

	if (kbill_ipc) {
		kbill_ipc->p_kipc_cycles += tsc_delta;
		kbill_ipc = NULL;
	}

	if (kbill_kcall) {
		kbill_kcall->p_kcall_cycles += tsc_delta;
		kbill_kcall = NULL;
	}

	/*
	 * Perform CPU average accounting here, rather than in the generic
	 * clock handler.  Doing it here offers two advantages: 1) we can
	 * account for time spent in the kernel, and 2) we properly account for
	 * CPU time spent by a process that has a lot of short-lasting activity
	 * such that it spends serious CPU time but never actually runs when a
	 * clock tick triggers.
	 */
	tpt = tsc_per_tick[cpu];

	p->p_tick_cycles += tsc_delta;
	while (tpt > 0 && p->p_tick_cycles >= tpt) {
		p->p_tick_cycles -= tpt;

		cpuavg_increment(&p->p_cpuavg, kclockinfo.uptime, system_hz);
	}

	/*
	 * Deduct the just consumed CPU cycles from the CPU time left for this
	 * process during its current quantum. Skip IDLE and other pseudo kernel
	 * tasks, except for global accounting purposes.
	 */
	if (p->p_endpoint >= 0) {
		if (p->p_priv != priv_addr(USER_PRIV_ID))
			counter = CP_SYS;
		else if (p->p_misc_flags & MF_NICED)
			counter = CP_NICE;
		else
			counter = CP_USER;

#if DEBUG_RACE
		p->p_cpu_time_left = 0;
#else
		if (tsc_delta < p->p_cpu_time_left) {
			p->p_cpu_time_left -= tsc_delta;
		} else {
			p->p_cpu_time_left = 0;
		}
#endif
	} else {
		if (p->p_endpoint == IDLE)
			counter = CP_IDLE;
		else
			counter = CP_INTR;
	}

	tsc_per_state[cpu][counter] += tsc_delta;

	*__tsc_ctr_switch = tsc;
}

/* =========================================================================
 * context_stop_idle — Stop idle cycle accounting
 *
 * Wrapper around context_stop() for the idle process.
 * Also restarts the local timer which may have been stopped during idle.
 * ========================================================================= */

void context_stop_idle(void)
{
	int is_idle;
#ifdef CONFIG_SMP
	unsigned cpu = cpuid;
#endif

	is_idle = get_cpu_var(cpu, cpu_is_idle);
	get_cpu_var(cpu, cpu_is_idle) = 0;

	context_stop(get_cpulocal_var_ptr(idle_proc));

	if (is_idle)
		restart_local_timer();
#if SPROFILE
	if (sprofiling)
		get_cpulocal_var(idle_interrupted) = 1;
#endif
}

/* =========================================================================
 * ms_2_cpu_time — Convert milliseconds to CPU time units (cycles)
 *
 * Parameters:
 *   ms  Number of milliseconds.
 *
 * Returns:
 *   Equivalent number of timer ticks (CNTPCT_EL0 counts).
 * ========================================================================= */

uint64_t ms_2_cpu_time(unsigned ms)
{
	return (uint64_t)tsc_per_ms[cpuid] * ms;
}

/* =========================================================================
 * cpu_time_2_ms — Convert CPU time units to milliseconds
 *
 * Parameters:
 *   cpu_time  Timer tick count.
 *
 * Returns:
 *   Equivalent number of milliseconds.
 * ========================================================================= */

unsigned cpu_time_2_ms(uint64_t cpu_time)
{
	return (unsigned)(cpu_time / tsc_per_ms[cpuid]);
}

/* =========================================================================
 * cpu_load — Return current CPU load as percentage
 *
 * Computes the fraction of non-idle CPU time since the last call.
 *
 * Returns:
 *   CPU load percentage (0-100).
 * ========================================================================= */

short cpu_load(void)
{
	uint64_t current_tsc, *current_idle;
	uint64_t tsc_delta, idle_delta, busy;
	struct proc *idle;
	short load;
#ifdef CONFIG_SMP
	unsigned cpu = cpuid;
#endif

	uint64_t *last_tsc, *last_idle;

	last_tsc  = get_cpu_var_ptr(cpu, cpu_last_tsc);
	last_idle = get_cpu_var_ptr(cpu, cpu_last_idle);

	idle = get_cpu_var_ptr(cpu, idle_proc);
	read_cntpct_64(&current_tsc);
	current_idle = &idle->p_cycles;

	/* Calculate load since last cpu_load invocation */
	if (*last_tsc) {
		tsc_delta  = current_tsc - *last_tsc;
		idle_delta = *current_idle - *last_idle;

		busy = tsc_delta - idle_delta;
		busy = busy * 100;
		load = (short)ex64lo(busy / tsc_delta);

		if (load > 100)
			load = 100;
	} else {
		load = 0;
	}

	*last_tsc    = current_tsc;
	*last_idle   = *current_idle;

	return load;
}

/* =========================================================================
 * busy_delay_ms — Busy-wait delay in milliseconds
 *
 * Spins in a tight loop reading CNTPCT_EL0 until the specified
 * number of milliseconds have elapsed.
 *
 * Parameters:
 *   ms  Delay duration in milliseconds.
 * ========================================================================= */

void busy_delay_ms(int ms)
{
	uint64_t cycles = ms_2_cpu_time((unsigned)ms);
	uint64_t tsc0, tsc1;

	read_cntpct_64(&tsc0);
	tsc1 = tsc0 + cycles;
	do {
		read_cntpct_64(&tsc0);
	} while (tsc0 < tsc1);
}

/* =========================================================================
 * get_cpu_ticks — Return per-state CPU tick counts
 *
 * Fills the provided array with the number of clock ticks spent in
 * each CPU state (USER, NICE, SYS, IDLE, INTR) for the given CPU.
 *
 * Parameters:
 *   cpu    CPU number.
 *   ticks  Output array of CPUSTATES entries.
 * ========================================================================= */

void get_cpu_ticks(unsigned int cpu, uint64_t ticks[CPUSTATES])
{
	int i;

	/* TODO: make this inter-CPU safe for SMP! */
	for (i = 0; i < CPUSTATES; i++)
		ticks[i] = tsc_per_state[cpu][i] / tsc_per_tick[cpu];
}
