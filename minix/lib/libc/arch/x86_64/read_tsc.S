/* read_tsc.S - x86_64 version */

#include <machine/asm.h>

/* PUBLIC void read_tsc(unsigned long *high, unsigned long *low); */
/* Read the cycle counter of the CPU. */
ENTRY(read_tsc)
	/* RDI = high pointer, RSI = low pointer */
	rdtsc			/* EDX:EAX = TSC */
	movl	%edx, (%rdi)	/* *high = EDX */
	movl	%eax, (%rsi)	/* *low = EAX */
	ret

/* PUBLIC void read_host_time_ns(unsigned long *high, unsigned long *low); */
/* Access real time in ns from host in VMware. */
ENTRY(read_host_time_ns)
	push	%rcx
	movl	$0x10001, %ecx
	rdtsc			/* EDX:EAX = TSC value in VMware */
	movl	%edx, (%rdi)	/* *high = EDX */
	movl	%eax, (%rsi)	/* *low = EAX */
	pop	%rcx
	ret
