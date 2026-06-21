/* Exception handler for x86_64.
 * Exceptions in user processes → signals.
 * Exceptions in kernel tasks → panic.
 */

#include "kernel/kernel.h"
#include "arch_proto.h"
#include <signal.h>
#include <string.h>
#include <assert.h>
#include <machine/vm.h>

/* Compatibility: x86_64 vm.h defines X86_64_CR0_TS */
#ifndef I386_CR0_TS
#define I386_CR0_TS X86_64_CR0_TS
#endif

struct ex_s {
	char *msg;
	int signum;
	int minprocessor;
};

static struct ex_s ex_data[] = {
	{ "Divide error", SIGFPE, 86 },
	{ "Debug exception", SIGTRAP, 86 },
	{ "Nonmaskable interrupt", SIGBUS, 86 },
	{ "Breakpoint", SIGEMT, 86 },
	{ "Overflow", SIGFPE, 86 },
	{ "Bounds check", SIGFPE, 186 },
	{ "Invalid opcode", SIGILL, 186 },
	{ "Coprocessor not available", SIGFPE, 186 },
	{ "Double fault", SIGBUS, 286 },
	{ "Coprocessor segment overrun", SIGSEGV, 286 },
	{ "Invalid TSS", SIGSEGV, 286 },
	{ "Segment not present", SIGSEGV, 286 },
	{ "Stack exception", SIGSEGV, 286 },
	{ "General protection", SIGSEGV, 286 },
	{ "Page fault", SIGSEGV, 386 },
	{ NULL, SIGILL, 0 },
	{ "Coprocessor error", SIGFPE, 386 },
	{ "Alignment check", SIGBUS, 386 },
	{ "Machine check", SIGBUS, 386 },
	{ "SIMD exception", SIGFPE, 386 },
};

static void inkernel_disaster(struct proc *saved_proc,
	struct exception_frame *frame, struct ex_s *ep, int is_nested);

extern int catch_pagefaults;

static void proc_stacktrace_execute(struct proc *whichproc, reg_t v_bp,
	reg_t pc);

static void pagefault(struct proc *pr, struct exception_frame * frame,
	int is_nested)
{
	int in_physcopy = 0, in_memset = 0;

	reg_t pagefaultcr2;
	message m_pagefault;
	int err;

	pagefaultcr2 = read_cr2();

	in_physcopy = (frame->eip > (vir_bytes) phys_copy) &&
	   (frame->eip < (vir_bytes) phys_copy_fault);

	in_memset = (frame->eip > (vir_bytes) phys_memset) &&
	   (frame->eip < (vir_bytes) memset_fault);

	if((is_nested || iskernelp(pr)) &&
		catch_pagefaults && (in_physcopy || in_memset)) {
		if (is_nested) {
			if(in_physcopy) {
				assert(!in_memset);
				frame->eip = (reg_t) phys_copy_fault_in_kernel;
			} else {
				frame->eip = (reg_t) memset_fault_in_kernel;
			}
		}
		else {
			pr->p_reg.pc = (reg_t) phys_copy_fault;
			pr->p_reg.retreg = pagefaultcr2;
		}
		return;
	}

	if(is_nested) {
		printf("pagefault in kernel at pc 0x%lx address 0x%lx\n",
			frame->eip, pagefaultcr2);
		inkernel_disaster(pr, frame, NULL, is_nested);
	}

	if(pr->p_endpoint == VM_PROC_NR) {
		printf("pagefault for VM on CPU %d, "
			"pc = 0x%lx, addr = 0x%lx, flags = 0x%lx, is_nested %d\n",
			cpuid, pr->p_reg.pc, pagefaultcr2, frame->errcode,
			is_nested);
		proc_stacktrace(pr);
		printf("pc of pagefault: 0x%lx\n", frame->eip);
		panic("pagefault in VM");
		return;
	}

	RTS_SET(pr, RTS_PAGEFAULT);

	m_pagefault.m_source = pr->p_endpoint;
	m_pagefault.m_type   = VM_PAGEFAULT;
	m_pagefault.VPF_ADDR = pagefaultcr2;
	m_pagefault.VPF_FLAGS = frame->errcode;

	if ((err = mini_send(pr, VM_PROC_NR,
					&m_pagefault, FROM_KERNEL))) {
		panic("WARNING: pagefault: mini_send returned %d\n", err);
	}
	return;
}

static void inkernel_disaster(struct proc *saved_proc,
	struct exception_frame * frame, struct ex_s *ep,
	int is_nested)
{
#if USE_SYSDEBUG
  if(ep) {
	if (ep->msg == NULL)
		printf("\nIntel-reserved exception %d\n", frame->vector);
	  else
		printf("\n%s\n", ep->msg);
  }

  printf("cpu %d is_nested = %d ", cpuid, is_nested);

  printf("vec_nr= %d, trap_errno= 0x%lx, eip= 0x%lx, "
	"cs= 0x%lx, eflags= 0x%lx\n",
	frame->vector, frame->errcode, frame->eip,
	frame->cs, frame->eflags);
  printf("KERNEL registers :\n");
#define REG(n) (((u32_t *)frame)[-n])
  printf(
	"%%rax 0x%08lx %%rbx 0x%08lx %%rcx 0x%08lx %%rdx 0x%08lx\n"
	"%%rsp 0x%08lx %%rbp 0x%08lx %%rsi 0x%08lx %%rdi 0x%08lx\n"
	"%%r8 0x%08lx %%r9 0x%08lx %%r10 0x%08lx %%r11 0x%08lx\n"
	"%%r12 0x%08lx %%r13 0x%08lx %%r14 0x%08lx %%r15 0x%08lx\n",
	REG(1), REG(2), REG(3), REG(4),
	REG(5), REG(6), REG(7), REG(8),
	REG(9), REG(10), REG(11), REG(12),
	REG(13), REG(14), REG(15), REG(16));

  {
  	reg_t k_ebp = REG(6);
  	printf("KERNEL stacktrace, starting with rbp = 0x%lx:\n", k_ebp);
  	proc_stacktrace_execute(proc_addr(SYSTEM), k_ebp, frame->eip);
  }

  if (saved_proc) {
	  printf("scheduled was: process %d (%s), ",
		saved_proc->p_endpoint, saved_proc->p_name);
	  printf("pc = 0x%lx\n", (unsigned long) saved_proc->p_reg.pc);
	  proc_stacktrace(saved_proc);
	  panic("Unhandled kernel exception");
  }

  panic("exception in kernel while booting, no saved_proc yet");
#endif
}

void exception_handler(int is_nested, struct exception_frame * frame)
{
  register struct ex_s *ep;
  struct proc *saved_proc;

  saved_proc = get_cpulocal_var(proc_ptr);

  ep = &ex_data[frame->vector];

  if (frame->vector == 2) {
	printf("got spurious NMI\n");
	return;
  }

  if (is_nested) {
	if (((void*)frame->eip >= (void*)copy_msg_to_user &&
			(void*)frame->eip <= (void*)__copy_msg_to_user_end) ||
			((void*)frame->eip >= (void*)copy_msg_from_user &&
			(void*)frame->eip <= (void*)__copy_msg_from_user_end)) {
		switch(frame->vector) {
		case PAGE_FAULT_VECTOR:
		case PROTECTION_VECTOR:
			frame->eip = (reg_t) __user_copy_msg_pointer_failure;
			return;
		default:
			panic("Copy involving a user pointer failed unexpectedly!");
		}
	}

	if (((void*)frame->eip >= (void*)fxrstor &&
			(void *)frame->eip <= (void*)__fxrstor_end) ||
			((void*)frame->eip >= (void*)frstor &&
			(void *)frame->eip <= (void*)__frstor_end)) {
		frame->eip = (reg_t) __frstor_failure;
		return;
	}

  	if(frame->vector == DEBUG_VECTOR
		&& (saved_proc->p_reg.psw & TRACEBIT)
		&& (saved_proc->p_seg.p_kern_trap_style == KTS_NONE)) {
		frame->eflags &= ~TRACEBIT;
		return;
	}
  }

  if(frame->vector == PAGE_FAULT_VECTOR) {
	pagefault(saved_proc, frame, is_nested);
	return;
  }

  if (is_nested == 0 && ! iskernelp(saved_proc)) {
	cause_sig(proc_nr(saved_proc), ep->signum);
	return;
  }

  inkernel_disaster(saved_proc, frame, ep, is_nested);

  panic("return from inkernel_disaster");
}

#if USE_SYSDEBUG
static void proc_stacktrace_execute(struct proc *whichproc, reg_t v_bp,
	reg_t pc)
{
	reg_t v_hbp;
	int iskernel;
	int n = 0;

	iskernel = iskernelp(whichproc);

	printf("%-8.8s %6d 0x%lx ",
		whichproc->p_name, whichproc->p_endpoint, pc);

	while(v_bp) {
		reg_t v_pc;

#define PRCOPY(pr, pv, v, n) \
  (iskernel ? (memcpy((char *) v, (char *) pv, n), OK) : \
     data_copy(pr->p_endpoint, pv, KERNEL, (vir_bytes) (v), n))

	        if(PRCOPY(whichproc, v_bp, &v_hbp, sizeof(v_hbp)) != OK) {
			printf("(v_bp 0x%lx ?)", v_bp);
			break;
		}
		if(PRCOPY(whichproc, v_bp + sizeof(v_pc), &v_pc,
			sizeof(v_pc)) != OK) {
			printf("(v_pc 0x%lx ?)", v_bp + sizeof(v_pc));
			break;
		}
		printf("0x%lx ", (unsigned long) v_pc);
		if(v_hbp != 0 && v_hbp <= v_bp) {
			printf("(hbp 0x%lx ?)", v_hbp);
			break;
		}
		v_bp = v_hbp;
		if(n++ > 50) {
			printf("(truncated after %d steps) ", n);
			break;
		}
	}
	printf("\n");
}
#endif /* USE_SYSDEBUG */

void proc_stacktrace(struct proc *whichproc)
{
	u32_t use_bp;

	if(whichproc->p_seg.p_kern_trap_style == KTS_NONE) {
		printf("WARNING: stacktrace of running process\n");
	}

	switch(whichproc->p_seg.p_kern_trap_style) {
		case KTS_SYSCALL:
		{
			u32_t sp = whichproc->p_reg.sp;
			if(data_copy(whichproc->p_endpoint, sp+16,
			  KERNEL, (vir_bytes) &use_bp,
				sizeof(use_bp)) != OK) {
				printf("stacktrace: aborting, copy failed\n");
				return;
			}
			break;
		}
		default:
			use_bp = whichproc->p_reg.fp;
			break;
	}

#if USE_SYSDEBUG
	proc_stacktrace_execute(whichproc, use_bp, whichproc->p_reg.pc);
#endif
}

void enable_fpu_exception(void)
{
	u32_t cr0 = read_cr0();
	if(!(cr0 & I386_CR0_TS))
		write_cr0(cr0 | I386_CR0_TS);
}

void disable_fpu_exception(void)
{
	clts();
}
