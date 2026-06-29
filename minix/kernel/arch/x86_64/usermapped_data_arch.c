/* x86_64 usermapped IPC data definitions.
 *
 * Provides the IPC vector tables (struct minix_ipcvecs) for each
 * supported IPC dispatch mechanism: softint and syscall.
 * SYSENTER is not available on x86_64 (SYSCALL/SYSRET is used instead).
 * minix_ipcvecs_sysenter is defined as null (the feature flag
 * MKF_I386_INTEL_SYSENTER is never set on x86_64, so this table
 * is never used, but the symbol must exist to satisfy the link).
 *
 * These tables are placed in the .usermapped section so they are
 * accessible from user-space after VM maps them.
 */

#include "kernel/kernel.h"
#include "arch_proto.h"

struct minix_ipcvecs minix_ipcvecs_softint __section(".usermapped") = {
	.send		= usermapped_send_softint,
	.receive	= usermapped_receive_softint,
	.sendrec	= usermapped_sendrec_softint,
	.sendnb		= usermapped_sendnb_softint,
	.notify		= usermapped_notify_softint,
	.do_kernel_call	= usermapped_do_kernel_call_softint,
	.senda		= usermapped_senda_softint
};

struct minix_ipcvecs minix_ipcvecs_syscall __section(".usermapped") = {
	.send		= usermapped_send_syscall,
	.receive	= usermapped_receive_syscall,
	.sendrec	= usermapped_sendrec_syscall,
	.sendnb		= usermapped_sendnb_syscall,
	.notify		= usermapped_notify_syscall,
	.do_kernel_call	= usermapped_do_kernel_call_syscall,
	.senda		= usermapped_senda_syscall
};

/* SYSENTER is never used on x86_64, but memory.c references this
 * symbol under a conditional that is never true on x86_64.
 * Define it as null to satisfy the linker.
 */
struct minix_ipcvecs minix_ipcvecs_sysenter __section(".usermapped") = {
	.send		= NULL,
	.receive	= NULL,
	.sendrec	= NULL,
	.sendnb		= NULL,
	.notify		= NULL,
	.do_kernel_call	= NULL,
	.senda		= NULL
};
