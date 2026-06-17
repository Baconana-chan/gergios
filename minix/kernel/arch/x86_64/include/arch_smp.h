#ifndef __SMP_X86_64_H__
#define __SMP_X86_64_H__

#include "arch_proto.h" /* K_STACK_SIZE */

#define MAX_NR_INTERRUPT_ENTRIES	128

#ifndef __ASSEMBLY__

/*
 * cpuid macro for x86_64.
 * The kernel stack layout reserves the top X86_STACK_TOP_RESERVED bytes
 * for per-CPU data. On x86_64, reg_t is 8 bytes (64-bit).
 * The stack is K_STACK_SIZE bytes per CPU, and we store the CPU id at
 * the top of the kernel stack (as reg_t).
 *
 * get_stack_frame() returns the current RBP (64-bit frame pointer).
 * The kernel stack base is at (RBP & ~(K_STACK_SIZE - 1)).
 * The CPU id is stored at the LAST reg_t slot at the top of the stack:
 *   stack_base + K_STACK_SIZE - sizeof(reg_t)
 */
#define cpuid	\
	(((reg_t *)(((unsigned long)get_stack_frame() \
		+ (K_STACK_SIZE - 1)) \
		& ~(K_STACK_SIZE - 1)))[-1])

/*
 * In case APIC or SMP is disabled in boot monitor, we need to finish single
 * CPU boot using the legacy PIC.
 * x86_64: TSS uses the same selectors but tss_init now handles 64-bit TSS
 * format (16-byte entries with IST fields).
 */
#define smp_single_cpu_fallback() do {		\
	  tss_init(0, get_k_stack_top(0));	\
	  bsp_cpu_id = 0;			\
	  ncpus = 1;				\
	  bsp_finish_booting();			\
} while(0)

extern unsigned char cpuid2apicid[CONFIG_MAX_CPUS];

/* Memory barrier: use mfence on x86_64 (same as i386) */
#define barrier()	do { mfence(); } while(0)

#endif /* __ASSEMBLY__ */

#endif /* __SMP_X86_64_H__ */
