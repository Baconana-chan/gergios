/* ============================================================
 * exception.c — ARM64 Exception Handler
 *
 * Handles synchronous exceptions (page faults, alignment faults,
 * undefined instructions, breakpoints) from both kernel and user
 * mode. Implements the C-level logic for:
 *
 *   1. Page fault dispatch to VM server (VM_PAGEFAULT)
 *   2. phys_copy / phys_memset fault recovery (catch_pagefaults)
 *   3. copy_msg_from_user / copy_msg_to_user fault recovery
 *   4. Unhandled exceptions → SIGSEGV/SIGBUS for user, panic for kernel
 *   5. Process stack tracing
 *
 * Called from mpx.S via:
 *   exception_handler(is_nested, frame_ptr, esr)
 * where:
 *   is_nested != 0 if exception occurred in kernel mode
 *   frame_ptr = SP_EL1 pointing to saved context on kernel stack
 *   esr = ESR_EL1 (Exception Syndrome Register) value
 *
 * Stack frame layout (from SAVE_FULL_CONTEXT in sconst.h):
 *   frame_ptr[0] = SPSR_EL1
 *   frame_ptr[1] = SP_EL0
 *   frame_ptr[2] = ELR_EL1 (PC at exception)
 *   frame_ptr[3] = x30 ... frame_ptr[33] = x0
 *
 * ESR_EL1 encoding:
 *   Bits[31:26] = Exception Class (EC)
 *   Bit  25     = IL (Instruction Length)
 *   Bits[24:0]  = ISS (Instruction Specific Syndrome)
 *
 * Reference: ARM DDI 0487 (ARMv8-A Architecture Reference Manual)
 * ============================================================ */

#include "kernel/kernel.h"
#include "arch_proto.h"
#include <signal.h>
#include <string.h>
#include <assert.h>
#include "kernel/proc.h"
#include "kernel/proto.h"
#include <machine/vm.h>

/* =========================================================================
 * Exception class descriptions and signal mapping
 *
 * Maps ARM64 ESR_EL1 exception classes to human-readable names and
 * POSIX signals for user-mode process delivery.
 * ========================================================================= */

struct ex_s {
	char *msg;
	int signum;
};

static struct ex_s ex_data[] = {
	{ "Unknown exception",           SIGILL },
	{ "WFI/WFE trap",                0      },
	{ "MCR/MRC (AArch32)",           0      },
	{ "MCRR/MRRC (AArch32)",         0      },
	{ "MCR (AArch32 coproc)",        0      },
	{ "LDC/STC (AArch32)",           0      },
	{ "FP/SIMD/SVE (AArch64)",       SIGFPE },
	{ "SVE/SIMD (AArch64)",          SIGFPE },
	{ "VMRS (AArch64)",              0      },
	{ "PAC",                         SIGILL },
	{ "BRB",                         0      },
	{ "0x0B",                        0      },
	{ "0x0C",                        0      },
	{ "0x0D",                        0      },
	{ "0x0E",                        0      },
	{ "IMPLEMENTATION DEFINED",      0      },
	{ "SVC (AArch32)",               SIGILL },
	{ "HVC (AArch32/64)",            0      },
	{ "SMC (AArch32/64)",            0      },
	{ "SYS (AArch64 MSR/MRS)",       0      },
	{ "0x13",                        0      },
	{ "0x14",                        0      },
	{ "SVC (AArch64)",               SIGILL },
	{ "HVC (AArch64)",               0      },
	{ "SMC (AArch64)",               0      },
	{ "SYS (AArch64 MSR/MRS)",       0      },
	{ "0x19",                        0      },
	{ "0x1A",                        0      },
	{ "0x1B",                        0      },
	{ "0x1C",                        0      },
	{ "0x1D",                        0      },
	{ "0x1E",                        0      },
	{ "0x1F",                        0      },
	{ "Instruction Abort (EL0)",     SIGSEGV },
	{ "Instruction Abort (EL1)",     0      },
	{ "0x22",                        0      },
	{ "0x23",                        0      },
	{ "Data Abort (EL0)",            SIGSEGV },
	{ "Data Abort (EL1)",            0      },
	{ "SP Alignment fault",          SIGBUS },
	{ "0x27",                        0      },
	{ "FP (AArch32)",                0      },
	{ "0x29",                        0      },
	{ "0x2A",                        0      },
	{ "0x2B",                        0      },
	{ "FP/SIMD (AArch64)",           0      },
	{ "SVE (AArch64)",               0      },
	{ "0x2E",                        0      },
	{ "SError / Asynchronous Abort", 0      },
	{ "Breakpoint (EL0)",            SIGTRAP },
	{ "Breakpoint (EL1)",            0      },
	{ "Software Step (EL0)",         SIGTRAP },
	{ "Software Step (EL1)",         0      },
	{ "Watchpoint (EL0)",            SIGTRAP },
	{ "Watchpoint (EL1)",            0      },
	{ "0x34",                        0      },
	{ "BKPT (AArch32)",              0      },
	{ "0x36",                        0      },
	{ "0x37",                        0      },
	{ "BRK (AArch64)",               SIGTRAP },
	{ "0x39",                        0      },
	{ "0x3A",                        0      },
	{ "0x3B",                        0      },
	{ "0x3C",                        0      },
	{ "0x3D",                        0      },
	{ "0x3E",                        0      },
	{ "0x3F",                        0      },
};

/* =========================================================================
 * Forward declarations
 * ========================================================================= */

static void inkernel_disaster(struct proc *saved_proc,
	reg_t *saved_lr, struct ex_s *ep, int is_nested);

extern int catch_pagefaults;

/* =========================================================================
 * ESR_EL1 field extraction helpers
 * ========================================================================= */

/* Exception Class (EC): bits [31:26] */
#define ESR_EC_SHIFT		26
#define ESR_EC_MASK		(0x3F << 26)
#define ESR_EC(esr)		(((esr) >> ESR_EC_SHIFT) & 0x3F)

/* Data/Instruction Abort ISS fields */
#define DA_ISS_WNR		(1 << 6)	/* Write not Read */
#define DA_ISS_FSC_MASK		0x3F		/* Fault Status Code */
#define DA_ISS_FSC(esr)		((esr) & DA_ISS_FSC_MASK)

/* =========================================================================
 * Page fault decoding
 *
 * ARM64 FSC values for data/instruction aborts:
 *   0b0001xx (0x04-0x07): Translation fault (level xx)
 *   0b0010xx (0x08-0x0B): Access flag fault (level xx)
 *   0b0011xx (0x0C-0x0F): Permission fault (level xx)
 *   0b0100xx (0x10-0x13): Domain fault (level xx)
 *   0b0101xx (0x14-0x17): Address size fault (level xx)
 *   0b100001 (0x21): Alignment fault
 * ========================================================================= */

#define FSC_TRANS_BASE		0x04	/* Translation fault (level 0) */
#define FSC_ACCESS_BASE		0x08	/* Access flag fault (level 0) */
#define FSC_PERM_BASE		0x0C	/* Permission fault (level 0) */
#define FSC_ALIGN		0x21	/* Alignment fault */

#define is_trans_fault(fsc)	\
	((fsc) >= FSC_TRANS_BASE && (fsc) < (FSC_TRANS_BASE + 4))

#define is_perm_fault(fsc)	\
	(((fsc) >= FSC_ACCESS_BASE && (fsc) < (FSC_ACCESS_BASE + 4)) || \
	 ((fsc) >= FSC_PERM_BASE && (fsc) < (FSC_PERM_BASE + 4)))

#define is_align_fault(fsc)	\
	((fsc) == FSC_ALIGN)

/* =========================================================================
 * Exception class handlers
 * ========================================================================= */

/* -----------------------------------------------------------------------
 * Stack frame access
 *
 * After SAVE_FULL_CONTEXT in sconst.h, the kernel stack contains:
 *   frame[0]  = SPSR_EL1
 *   frame[1]  = SP_EL0
 *   frame[2]  = ELR_EL1 (PC at exception)
 *   frame[3]  = x30
 *   ...
 *   frame[33] = x0
 * ----------------------------------------------------------------------- */

/* Get PC at exception from saved frame */
#define FRAME_PC(frame)		((reg_t)(frame)[2])
#define FRAME_SPSR(frame)	((reg_t)(frame)[0])
#define FRAME_SP_EL0(frame)	((reg_t)(frame)[1])

/*
 * Access x0 from the saved frame.
 * After SAVE_GPRS (15 pairs + x30 = 31 regs = 0..240 bytes),
 * then SAVE_EXTRA_STATE (SP_EL0, ELR_EL1, SPSR_EL1 = 241..272),
 * x0 is at the highest offset: frame[33].
 *
 * frame[0]=SPSR, frame[1]=SP_EL0, frame[2]=ELR_EL1,
 * frame[3]=x30, ..., frame[33]=x0.
 */
#define FRAME_X0(frame)		(frame)[33]

/* Memory barrier: instruction synchronization barrier for ARM64 */
#define isb() __asm__ __volatile__("isb" : : : "memory")

/* =========================================================================
 * pagefault — Handle page faults from user or kernel mode
 *
 * Called for translation faults, access flag faults, and permission
 * faults (both data and instruction aborts).
 *
 * @param pr         Process that caused the fault
 * @param saved_lr   Pointer to saved frame on kernel stack
 * @param is_nested  1 if fault in kernel mode, 0 if in user mode
 * @param fault_addr Fault address (FAR_EL1)
 * @param fault_st   Fault status (FSC value from ESR_EL1)
 * @param write_fault Non-zero if write fault (WnR bit from ESR_EL1)
 * ========================================================================= */

static void pagefault(struct proc *pr,
		      reg_t *saved_lr,
		      int is_nested,
		      uint64_t fault_addr,
		      uint64_t fault_st,
		      int write_fault)
{
	int in_physcopy = 0, in_memset = 0;
	message m_pagefault;
	int err;
	reg_t pc = FRAME_PC(saved_lr);

	/* Check if fault occurred during phys_copy or phys_memset */
	in_physcopy = ((uint64_t)pc > (uint64_t)&phys_copy) &&
		      ((uint64_t)pc < (uint64_t)&phys_copy_fault);

	in_memset = ((uint64_t)pc > (uint64_t)&phys_memset) &&
		    ((uint64_t)pc < (uint64_t)&memset_fault);

	if ((is_nested || iskernelp(pr)) &&
	    catch_pagefaults && (in_physcopy || in_memset)) {
		if (is_nested) {
			if (in_physcopy) {
				assert(!in_memset);
				saved_lr[2] = (reg_t)phys_copy_fault_in_kernel;
			} else {
				saved_lr[2] = (reg_t)memset_fault_in_kernel;
			}
		} else {
			/*
			 * For non-nested kernel faults (shouldn't normally
			 * happen for phys_copy), redirect to user-return
			 * fault path. ARM64: redirect ELR_EL1 to fault
			 * handler and set return value (x0) to fault addr.
			 */
			saved_lr[2] = (reg_t)phys_copy_fault;
		/* x0 is at frame[33] (after SAVE_GPRS, x0 last) */
		FRAME_X0(saved_lr) = fault_addr;
		}
		return;
	}

	/* Nested kernel faults that aren't recoverable: panic */
	if (is_nested) {
		printf("pagefault in kernel at pc 0x%lx address 0x%lx\n",
			(unsigned long)pc, (unsigned long)fault_addr);
		inkernel_disaster(pr, saved_lr, NULL, is_nested);
		return;
	}

	/* VM can't handle page faults */
	if (pr->p_endpoint == VM_PROC_NR) {
		printf("pagefault for VM on CPU %d, "
			"pc = 0x%lx, addr = 0x%lx, flags = 0x%lx, "
			"is_nested %d\n",
			cpuid,
			(unsigned long)pc,
			(unsigned long)fault_addr,
			(unsigned long)fault_st,
			is_nested);
		proc_stacktrace(pr);
		printf("pc of pagefault: 0x%lx\n", (unsigned long)pc);
		panic("pagefault in VM");
		return;
	}

	/* Don't schedule this process until pagefault is handled */
	RTS_SET(pr, RTS_PAGEFAULT);

	/* Tell VM about the pagefault */
	m_pagefault.m_source = pr->p_endpoint;
	m_pagefault.m_type   = VM_PAGEFAULT;
	m_pagefault.VPF_ADDR = fault_addr;
	m_pagefault.VPF_FLAGS = (write_fault ? AARCH64_VM_PFE_W : 0) |
				(fault_st & AARCH64_VM_PFE_FSC_MASK);

	if ((err = mini_send(pr, VM_PROC_NR,
			     &m_pagefault, FROM_KERNEL))) {
		panic("WARNING: pagefault: mini_send returned %d\n", err);
	}
}

/* =========================================================================
 * data_abort — Handle data abort exception
 *
 * ARM64 data abort comes with:
 *   - FAR_EL1: fault address
 *   - ESR_EL1[5:0]: FSC (fault status code)
 *   - ESR_EL1[6]: WnR (1 = write, 0 = read)
 * ========================================================================= */

static void data_abort(int is_nested, struct proc *pr,
		       reg_t *saved_lr, struct ex_s *ep,
		       uint64_t far, uint64_t esr_fsc, int write_fault)
{
	/* Translation and permission faults → pagefault */
	if (is_trans_fault(esr_fsc) || is_perm_fault(esr_fsc)) {
		pagefault(pr, saved_lr, is_nested, far, esr_fsc, write_fault);
	} else if (!is_nested) {
		/* User process caused some other kind of data abort */
		int signum = SIGSEGV;

		if (is_align_fault(esr_fsc)) {
			signum = SIGBUS;
		} else {
			printf("KERNEL: unknown data abort by proc %d, "
			       "sending SIGSEGV "
			       "(far=0x%lx esr_fsc=0x%lx)\n",
			       proc_nr(pr),
			       (unsigned long)far,
			       (unsigned long)esr_fsc);
		}
		cause_sig(proc_nr(pr), signum);
	} else {
		/* Nested data abort in kernel */
		printf("KERNEL: inkernel data abort - disaster "
		       "(far=0x%lx esr_fsc=0x%lx)\n",
		       (unsigned long)far, (unsigned long)esr_fsc);
		inkernel_disaster(pr, saved_lr, ep, is_nested);
	}
}

/* =========================================================================
 * inkernel_disaster — Handle unrecoverable kernel exception
 *
 * Called when an exception occurs in kernel mode that cannot be
 * handled (e.g., page fault in interrupt handler, or fault in
 * non-phys_copy/memset code). Prints diagnostic info and panics.
 * ========================================================================= */

static void inkernel_disaster(struct proc *saved_proc,
	reg_t *saved_lr, struct ex_s *ep, int is_nested)
{
#if USE_SYSDEBUG
	if (ep)
		printf("\n%s\n", ep->msg);

	printf("cpu %d is_nested = %d ", cpuid, is_nested);

	if (saved_proc) {
		printf("scheduled was: process %d (%s), ",
		       saved_proc->p_endpoint, saved_proc->p_name);
		printf("pc = 0x%lx\n",
		       (unsigned long)FRAME_PC(saved_lr));
		proc_stacktrace(saved_proc);
		panic("Unhandled kernel exception");
	}

	/* In an early stage of boot we don't have processes yet */
	panic("exception in kernel while booting, no saved_proc yet");
#endif /* USE_SYSDEBUG */
}

/* =========================================================================
 * exception_handler — Main exception handler entry point
 *
 * Called from arm64_exception_entry_from_user and
 * arm64_exception_entry_from_kernel in mpx.S.
 *
 * @param is_nested  Non-zero if exception occurred in kernel mode
 * @param saved_lr   Pointer to register save frame on kernel stack
 * @param vector     ESR_EL1 value (exception syndrome register)
 *
 * ARM64 exception classes (EC in ESR_EL1[31:26]):
 *   0x20-0x21: Instruction Abort (EL0/EL1)
 *   0x24-0x25: Data Abort (EL0/EL1)
 *   0x26:      SP Alignment Fault
 *   0x2C:      Floating-point Exception (AArch64)
 *   0x30-0x35: Debug exceptions
 *   0x38:      BRK (AArch64)
 * ========================================================================= */

void exception_handler(int is_nested, reg_t *saved_lr, int vector)
{
	struct ex_s *ep;
	struct proc *saved_proc;
	uint64_t esr = (uint64_t)vector;
	unsigned int ec = ESR_EC(esr);
	reg_t pc = FRAME_PC(saved_lr);

	saved_proc = get_cpulocal_var(proc_ptr);

	/* Verify frame pointer is in kernel space */
	assert((vir_bytes)saved_lr >= kinfo.vir_kern_start);

	/* Get exception descriptor (EC index 0-63) */
	ep = (ec < 64) ? &ex_data[ec] : NULL;

	/*
	 * Handle special cases for nested faults:
	 * copy_msg_from_user / copy_msg_to_user pointer failure.
	 */
	if (is_nested) {
		/*
		 * If a fault occurred while copying a message from userspace
		 * because of a bad pointer, redirect to failure handler.
		 */
		if (((void *)pc >= (void *)copy_msg_to_user &&
		     (void *)pc <= (void *)__copy_msg_to_user_end) ||
		    ((void *)pc >= (void *)copy_msg_from_user &&
		     (void *)pc <= (void *)__copy_msg_from_user_end)) {
			switch (ec) {
			case 0x25: /* Data Abort from EL1 */
			case 0x21: /* Instruction Abort from EL1 */
				saved_lr[2] = (reg_t)__user_copy_msg_pointer_failure;
				return;
			default:
				panic("Copy involving a user pointer failed "
				      "unexpectedly!");
			}
		}
	}

	/* Data abort handling */
	if (ec == 0x24 || ec == 0x25) {
		uint64_t far = read_far();
		uint64_t fsc = DA_ISS_FSC(esr);
		int write_fault = (esr & DA_ISS_WNR) ? 1 : 0;

		data_abort(is_nested, saved_proc, saved_lr, ep,
			   far, fsc, write_fault);
		return;
	}

	/* Instruction abort handling */
	if (ec == 0x20 || ec == 0x21) {
		uint64_t far = read_far();
		uint64_t fsc = DA_ISS_FSC(esr);

		/*
		 * Instruction aborts: the fault address is in FAR_EL1.
		 * ARM64 also stores the VA that caused the fault in FAR_EL1
		 * for instruction aborts (unlike ARM32 which uses IFAR).
		 */
		reg_t ifar = far;

		/*
		 * On ARM64, ELR_EL1 points to the faulting instruction
		 * (for synchronous exceptions). FAR_EL1 contains the
		 * address that caused the fault.
		 */
		if (pc != ifar) {
			printf("KERNEL: instruction abort with differing "
			       "FAR and ELR\n");
			printf("KERNEL: FAR %"PRIx64" ELR %"PRIx64" in "
			       "%s/%d\n",
			       (uint64_t)ifar, (uint64_t)pc,
			       saved_proc->p_name,
			       saved_proc->p_endpoint);
		}

		pagefault(saved_proc, saved_lr, is_nested, ifar, fsc, 0);
		return;
	}

	/*
	 * If an exception occurs while running a user process (not nested),
	 * deliver the appropriate signal.
	 */
	if (is_nested == 0 && !iskernelp(saved_proc) && ep && ep->signum) {
		cause_sig(proc_nr(saved_proc), ep->signum);
		return;
	}

	/* Exception in system code — this should not happen */
	if (ep) {
		inkernel_disaster(saved_proc, saved_lr, ep, is_nested);
	} else {
		printf("KERNEL: unknown exception EC=%u at pc=0x%lx\n",
		       ec, (unsigned long)pc);
		inkernel_disaster(saved_proc, saved_lr, NULL, is_nested);
	}

	panic("return from inkernel_disaster");
}

/* =========================================================================
 * proc_stacktrace — Print process stack trace
 *
 * Shows the saved PC from the process's register context.
 * Full stack unwinding (frame pointer walk) is TBD for Phase 5+.
 * ========================================================================= */

void proc_stacktrace(struct proc *whichproc)
{
#if USE_SYSDEBUG
	reg_t pc = whichproc->p_reg.elr_el1;
	reg_t fp = whichproc->p_reg.gpr[29]; /* x29 = frame pointer */

	printf("%-8.8s %6d pc=0x%lx fp=0x%lx\n",
	       whichproc->p_name,
	       whichproc->p_endpoint,
	       (unsigned long)pc,
	       (unsigned long)fp);

	/*
	 * TODO (Phase 5+): Walk the frame pointer chain to produce
	 * a full stack trace. ARM64 frame layout:
	 *   [fp + 0]  = previous FP (x29)
	 *   [fp + 8]  = return address (x30)
	 */
	(void)fp; /* Unused until Phase 5+ */
#endif /* USE_SYSDEBUG */
}

/* =========================================================================
 * FPU exception control
 *
 * ARM64 FP/SIMD exceptions are enabled/disabled via the FPCR and
 * CPTR_EL2/CPACR_EL1 registers.
 * ========================================================================= */

/*
 * Enable FPU/SIMD exception traps.
 * ARM64: FP/SIMD access is controlled by CPACR_EL1.FPEN.
 *   CPACR_EL1[21:20] = FPEN:
 *     0b00: Traps EL0 and EL1 access
 *     0b01: Traps only EL0 access
 *     0b10: Traps only EL1 access (not used)
 *     0b11: No traps
 */
void enable_fpu_exception(void)
{
	uint64_t cpacr;

	/* Read CPACR_EL1, set FPEN[21:20] = 0b00 to trap all */
	__asm__ __volatile__("mrs %0, CPACR_EL1" : "=r"(cpacr));
	cpacr &= ~(3UL << 20);
	__asm__ __volatile__("msr CPACR_EL1, %0" : : "r"(cpacr));
	isb();
}

/*
 * Disable FPU/SIMD exception traps (allow access).
 * ARM64: Set CPACR_EL1.FPEN[21:20] = 0b11 to allow EL0/EL1 access.
 */
void disable_fpu_exception(void)
{
	uint64_t cpacr;

	__asm__ __volatile__("mrs %0, CPACR_EL1" : "=r"(cpacr));
	cpacr |= (3UL << 20);	/* FPEN = 0b11: no traps */
	__asm__ __volatile__("msr CPACR_EL1, %0" : : "r"(cpacr));
	isb();
}

/* =========================================================================
 * End of exception.c
 * ========================================================================= */
