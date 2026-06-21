/* ============================================================
 * arch_system.c — ARM64 architecture system support
 *
 * Architecture-specific functions for system call handling,
 * CPU initialization, context management, and FPU support.
 *
 * References:
 *   ARM DDI 0487 — ARMv8-A Architecture Reference Manual
 * ============================================================ */

#include "kernel/kernel.h"

#include <unistd.h>
#include <ctype.h>
#include <string.h>
#include <minix/cpufeature.h>
#include <assert.h>
#include <signal.h>
#include <machine/vm.h>
#include <machine/signal.h>

#include <minix/u64.h>

#include "arch_proto.h"
#include "kernel/proc.h"
#include "kernel/debug.h"
#include "kernel/glo.h"

/* =========================================================================
 * Constants
 * ========================================================================= */

/* Initial PSTATE (SPSR_EL1) values.
 * ARM64 PSTATE format: M[3:0] = mode, DAIF[9:6] = interrupt masks
 *   EL0t:  M=0x0 (user mode, SP_EL0)
 *   EL1h:  M=0x5 (kernel mode, SP_EL1)
 *   DAIF masked: bits 6,7,8,9 set (D,A,I,F masked) */
#define ARM64_INIT_USR_PSR      0x00000000      /* EL0t, no DAIF masking */
#define ARM64_INIT_TASK_PSR     0x000003C5      /* EL1h, DAIF masked */

/* CPACR_EL1 fields */
#define CPACR_EL1_FPEN_SHIFT    20
#define CPACR_EL1_FPEN_MASK     (3UL << CPACR_EL1_FPEN_SHIFT)
#define CPACR_EL1_FPEN_TRAP_ALL (0UL << CPACR_EL1_FPEN_SHIFT)  /* Trap all */
#define CPACR_EL1_FPEN_TRAP_EL0 (1UL << CPACR_EL1_FPEN_SHIFT)  /* Trap EL0 only */
#define CPACR_EL1_FPEN_NONE     (3UL << CPACR_EL1_FPEN_SHIFT)  /* No traps */

/* PMU (Performance Monitors) register fields */
#define PMCR_EL0_E              (1UL << 0)   /* Enable counters */
#define PMCR_EL0_P              (1UL << 1)   /* Reset all counters */
#define PMCR_EL0_C              (1UL << 2)   /* Cycle counter reset */
#define PMCR_EL0_D              (1UL << 3)   /* Clock divider */
#define PMCR_EL0_X              (1UL << 4)   /* Export enable */
#define PMCR_EL0_DP             (1UL << 5)   /* Disable cycle counter when prog */
#define PMCNTENSET_EL0_C        (1UL << 31)  /* Cycle counter enable bit */
#define PMUSERENR_EL0_EN        (1UL << 0)   /* EL0 access enable */
#define PMUSERENR_EL0_CR        (1UL << 2)   /* Cycle counter read enable at EL0 */

/* =========================================================================
 * Kernel stacks
 * ========================================================================= */

void *k_stacks;

/* =========================================================================
 * arch_init — Architecture initialization
 *
 * Called during kernel initialization to set up architecture-specific
 * features: kernel stacks, cycle counter, PMU, etc.
 * ========================================================================= */

void arch_init(void)
{
	uint64_t value;

	k_stacks = (void *)&k_stacks_start;
	assert(!((vir_bytes)k_stacks % K_STACK_SIZE));

#ifndef CONFIG_SMP
	/* Use stack 0 and cpu id 0 on single processor.
	 * SMP does this in smp_init() for all CPUs. */
	tss_init(0, get_k_stack_top(0));
#endif

	/* Enable user-space access to the cycle counter (PMCCNTR_EL0).
	 *
	 * Step 1: Reset and enable the cycle counter via PMCR_EL0.
	 *   PMCR_EL0.C = 1: reset cycle counter to 0
	 *   PMCR_EL0.E = 1: enable all counters */
	__asm__ volatile("mrs %0, pmcr_el0" : "=r"(value));
	value |= PMCR_EL0_C | PMCR_EL0_E;
	__asm__ volatile("msr pmcr_el0, %0" : : "r"(value));
	isb();

	/* Step 2: Enable cycle counter counting via PMCNTENSET_EL0.
	 *   Bit 31 enables PMCCNTR (the cycle counter). */
	value = PMCNTENSET_EL0_C;
	__asm__ volatile("msr pmcntenset_el0, %0" : : "r"(value));
	isb();

	/* Step 3: Enable user-mode access via PMUSERENR_EL0.
	 *   Bit 0: enable EL0 access to PMU registers
	 *   Bit 2: enable EL0 read of PMCCNTR_EL0 */
	value = PMUSERENR_EL0_EN | PMUSERENR_EL0_CR;
	__asm__ volatile("msr pmuserenr_el0, %0" : : "r"(value));
	isb();
}

/* =========================================================================
 * fpu_init — Initialize FPU/SIMD for current CPU
 *
 * ARMv8-A always has FPU and NEON SIMD. This function enables
 * access by setting CPACR_EL1.FPEN to allow both EL0 and EL1.
 * ========================================================================= */

void fpu_init(void)
{
	uint64_t cpacr;

	__asm__ volatile("mrs %0, cpacr_el1" : "=r"(cpacr));
	cpacr &= ~CPACR_EL1_FPEN_MASK;
	cpacr |= CPACR_EL1_FPEN_NONE;   /* No FP/SIMD traps at any EL */
	__asm__ volatile("msr cpacr_el1, %0" : : "r"(cpacr));
	isb();
}

/* =========================================================================
 * save_fpu — Save FPU state for a process (no-op)
 * ========================================================================= */

void save_fpu(struct proc *pr)
{
	(void)pr;
	/* TODO (Phase 5+): Save FP/SIMD registers (V0-V31, FPSR, FPCR)
	 * to proc->p_fpu_state when lazy FPU switching is implemented. */
}

/* =========================================================================
 * save_local_fpu — Save local CPU FPU state (no-op)
 * ========================================================================= */

void save_local_fpu(struct proc *pr, int retain)
{
	(void)pr;
	(void)retain;
	/* TODO (Phase 5+): Save current CPU FPU context. */
}

/* =========================================================================
 * restore_fpu — Restore FPU state for a process (no-op)
 * ========================================================================= */

int restore_fpu(struct proc *pr)
{
	(void)pr;
	/* TODO (Phase 5+): Restore FP/SIMD registers from proc->p_fpu_state. */
	return 0;
}

/* =========================================================================
 * fpu_sigcontext — Fill in FPU part of signal context (no-op)
 * ========================================================================= */

void fpu_sigcontext(struct proc *pr, struct sigframe_sigcontext *fr,
		    struct sigcontext *sc)
{
	(void)pr;
	(void)fr;
	(void)sc;
	/* TODO (Phase 5+): Save FP/SIMD state into mcontext_t/fpregset_t. */
}

/* =========================================================================
 * arch_proc_reset — Reset process context to initial state
 *
 * Clears all GPRs and sets initial PSTATE.
 * Kernel tasks start in EL1h mode with interrupts masked.
 * User processes start in EL0t mode (interrupts auto-masked by CPU).
 * ========================================================================= */

void arch_proc_reset(struct proc *pr)
{
	assert(pr->p_nr < NR_PROCS);

	/* Clear all process registers */
	memset(&pr->p_reg, 0, sizeof(pr->p_reg));

	/* Set initial PSTATE */
	if (iskerneln(pr->p_nr)) {
		pr->p_reg.spsr_el1 = ARM64_INIT_TASK_PSR;
	} else {
		pr->p_reg.spsr_el1 = ARM64_INIT_USR_PSR;
	}
}

/* =========================================================================
 * arch_proc_setcontext — Set full process context from stackframe_s
 * ========================================================================= */

void arch_proc_setcontext(struct proc *p, struct stackframe_s *state,
			  int isuser, int trapstyle)
{
	assert(sizeof(p->p_reg) == sizeof(*state));

	if (state != &p->p_reg) {
		memcpy(&p->p_reg, state, sizeof(*state));
	}

	/* Mark that context has been explicitly set */
	p->p_misc_flags |= MF_CONTEXT_SET;

	if (!(p->p_rts_flags)) {
		printf("WARNING: setting full context of runnable process\n");
		print_proc(p);
		util_stacktrace();
	}

	(void)isuser;
	(void)trapstyle;
}

/* =========================================================================
 * arch_set_secondary_ipc_return — Set IPC return value for secondary process
 *
 * On ARM64, the return value for the secondary IPC participant is
 * stored in x1 (per MINIX convention for ARM64 syscall ABI).
 * ========================================================================= */

void arch_set_secondary_ipc_return(struct proc *p, u32_t val)
{
	p->p_reg.gpr[1] = val;
}

/* =========================================================================
 * cpu_identify — Read CPU identification from MIDR_EL1
 * ========================================================================= */

void cpu_identify(void)
{
	uint64_t midr;
	unsigned cpu = cpuid;

	__asm__ volatile("mrs %0, midr_el1" : "=r"(midr));

	cpu_info[cpu].implementer = (unsigned)(midr >> 24) & 0xFF;
	cpu_info[cpu].variant     = (unsigned)(midr >> 20) & 0xF;
	cpu_info[cpu].arch        = (unsigned)(midr >> 16) & 0xF;
	cpu_info[cpu].part        = (unsigned)(midr >> 4)  & 0xFFF;
	cpu_info[cpu].revision    = (unsigned)(midr)       & 0xF;
	cpu_info[cpu].freq        = 0;   /* Will be calibrated by timer init */
}

/* =========================================================================
 * arch_do_syscall — Handle a system call from user space
 *
 * Called from the SVC handler when a user process makes a syscall.
 * The syscall number is in gpr[8] (ARM64 convention: x8 = syscall number).
 * Arguments are in gpr[0..2] (ipc_dest, msg_ptr, flags).
 *
 * For MINIX IPC:
 *   gpr[0] = destination endpoint
 *   gpr[1] = message pointer
 *   gpr[2] = flags/status
 *   Returns: gpr[0] = result
 * ========================================================================= */

void arch_do_syscall(struct proc *proc)
{
	/* do_ipc assumes it's running because of the current process */
	assert(proc == get_cpulocal_var(proc_ptr));

	/* Make the system call.
	 * ARM64 convention: x0=dest, x1=msg_ptr, x2=flags
	 *                   return value in x0 */
	proc->p_reg.gpr[0] = do_ipc(
		(reg_t)proc->p_reg.gpr[0],    /* destination endpoint */
		(reg_t)proc->p_reg.gpr[1],    /* message pointer */
		(reg_t)proc->p_reg.gpr[2]     /* flags */
	);
}

/* =========================================================================
 * arch_finish_switch_to_user — Finish switch to user mode
 *
 * Prepares the kernel stack for the return to user mode.
 * Returns the process pointer that will be restored by restore_user_context().
 *
 * On ARM64, the kernel stack contains the saved exception frame
 * (SPSR_EL1, ELR_EL1, SP_EL0, and GPRs). The ERET instruction
 * restores from these registers.
 * ========================================================================= */

struct proc *arch_finish_switch_to_user(void)
{
	struct proc *p;

	/* Get the process to run */
	p = get_cpulocal_var(proc_ptr);

	/* Clear interrupt masks in PSTATE so the process receives
	 * interrupts when it runs in user mode.
	 * SPSR_EL1.DAIF bits: clear I (bit 7) and F (bit 6) */
	p->p_reg.spsr_el1 &= ~(0x3C0);  /* Clear A, I, F bits */

	return p;
}

/* =========================================================================
 * arch_get_sp — Get current stack pointer of a process
 *
 * Returns the SP_EL0 (user stack pointer) from the process's saved context.
 * ========================================================================= */

reg_t arch_get_sp(struct proc *p)
{
	return (reg_t)p->p_reg.sp_el0;
}

/* =========================================================================
 * do_ser_debug — Serial debug hook (no-op on ARM64)
 * ========================================================================= */

void do_ser_debug(void)
{
	/* No-op: serial debug not implemented. */
}

/* =========================================================================
 * arch_init_profile_clock — Initialize profiling clock (no-op)
 * ========================================================================= */

int arch_init_profile_clock(u32_t freq)
{
	(void)freq;
	/* TODO (Phase 5+): Use ARM PMU for profiling. */
	return 0;
}

/* =========================================================================
 * arch_stop_profile_clock — Stop profiling clock (no-op)
 * ========================================================================= */

void arch_stop_profile_clock(void)
{
	/* No-op */
}

/* =========================================================================
 * arch_ack_profile_clock — Acknowledge profiling clock interrupt (no-op)
 * ========================================================================= */

void arch_ack_profile_clock(void)
{
	/* No-op */
}

/* =========================================================================
 * get_randomness — Accumulate randomness from interrupt sources
 *
 * Called from do_irqctl.c when an IRQ hook fires.
 * ARM64: uses generic timer (CNTPCT_EL0) as entropy source.
 * Pattern matches x86_64 version (read_tsc → CNTPCT_EL0).
 * ========================================================================= */

void get_randomness(struct k_randomness *rand, int source)
{
	int r_next;
	uint64_t tsc;

	source %= RANDOM_SOURCES;
	if (rand->bin[source].r_size >= RANDOM_ELEMENTS)
		return;
	r_next = rand->bin[source].r_next;

	__asm__ volatile("mrs %0, cntpct_el0" : "=r"(tsc));
	rand->bin[source].r_buf[r_next] = tsc;

	if (rand->bin[source].r_size < RANDOM_ELEMENTS)
		rand->bin[source].r_size++;
	rand->bin[source].r_next = (r_next + 1) % RANDOM_ELEMENTS;
}

/* =========================================================================
 * arch_get_params — Get kernel boot parameters (stub)
 * ========================================================================= */

int arch_get_params(char *parm, int max)
{
	(void)parm;
	(void)max;
	/* TODO (Phase 2+): Parse device tree /cmdline for bootargs. */
	return 0;
}

