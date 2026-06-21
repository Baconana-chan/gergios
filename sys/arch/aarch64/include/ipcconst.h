/*	AArch64 IPC constants.	*/

#ifndef _AARCH64_IPCCONST_H_
#define _AARCH64_IPCCONST_H_

/* Syscall vector numbers (SVC immediate values) */
#define KERVEC_INTR		32	/* syscall trap to kernel */
#define IPCVEC_INTR		33	/* ipc trap to kernel */

/* IPC status is stored in x1 (gpr[1] in stackframe_s array) */
#define IPC_STATUS_REG		gpr[1]

#endif /* !_AARCH64_IPCCONST_H_ */
