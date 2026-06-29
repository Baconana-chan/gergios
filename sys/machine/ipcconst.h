/*	$NetBSD$	*/
/* Machine-dependent IPC constants. */

#ifndef _MACHINE_IPCCONST_H_
#define _MACHINE_IPCCONST_H_

/* Syscall vector numbers */
/* On x86_64, these are IDT vector indices for user-interrupt dispatch.
 * On AArch64, these are SVC immediate values (32/33). */
#define KERVEC_INTR		32	/* syscall trap to kernel */
#define IPCVEC_INTR		33	/* ipc trap to kernel */

/* IPC status register location in stackframe_s.
 * x86_64 uses named fields (retreg = RAX at offset 0 in SAVE_GP_REGS).
 * ARM/AArch64 uses gpr[] array (x1 = gpr[1]). */
#if defined(__x86_64__)
#define IPC_STATUS_REG		retreg
#else
#define IPC_STATUS_REG		gpr[1]
#endif

#endif /* _MACHINE_IPCCONST_H_ */
