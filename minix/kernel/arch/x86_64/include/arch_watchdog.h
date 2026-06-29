/* ============================================================
 * arch_watchdog.h — x86_64 architecture NMI watchdog interface
 *
 * Declares the NMI frame structure and watchdog interface for x86_64.
 * The x86_64 NMI frame mirrors the exception frame layout pushed by
 * the CPU on NMI entry (iretq-style frame + error code).
 *
 * NOTE: nmi_watchdog_start_profiling() and nmi_watchdog_stop_profiling()
 * are defined in kernel/watchdog.c (shared), not as inline stubs here.
 * ============================================================ */

#ifndef _X86_64_ARCH_WATCHDOG_H_
#define _X86_64_ARCH_WATCHDOG_H_

#include "kernel/kernel.h"

/* NMI frame — registers saved by CPU on NMI entry (iretq frame) */
struct nmi_frame {
	reg_t	eax;
	reg_t	ecx;
	reg_t	edx;
	reg_t	ebx;
	reg_t	esp;
	reg_t	ebp;
	reg_t	esi;
	reg_t	edi;
	u16_t	gs;
	u16_t	fs;
	u16_t	es;
	u16_t	ds;
	reg_t	pc;		/* program counter (RIP from interrupt) */
	reg_t	cs;
	reg_t	eflags;
};

/* Helper: was the NMI taken while in kernel mode? */
#define nmi_in_kernel(f)	((f)->cs == KERN_CS_SELECTOR)

#endif /* _X86_64_ARCH_WATCHDOG_H_ */
