#include <minix/ipcconst.h>
#include <machine/asm.h>

/* x86_64 IPC syscall wrapper using SYSCALL instruction.
 *
 * Kernel convention (from mpx.S):
 *   RAX = syscall number (SEND/RECEIVE/SENDREC/NOTIFY/SENDNB/SENDA)
 *   RDX = destination/source endpoint
 *   RSI = message pointer (or table ptr for SENDA)
 *   Return: status in RBX
 *
 * SYSCALL clobbers RCX (saves RIP) and R11 (saves RFLAGS).
 *
 * C calling convention (x86_64 System V ABI):
 *   RDI = arg1, RSI = arg2, RDX = arg3, RCX = arg4, R8 = arg5, R9 = arg6
 *   Return: RAX
 *   Preserved: RBX, RBP, R12-R15
 */

/**========================================================================* */
/* _ipc_send_intr(endpoint_t dest, message *msg)                           * */
/**========================================================================* */
ENTRY(_ipc_send_intr)
	/* RDI = dest, RSI = msg => Need: RDX=dest, RSI=msg */
	mov	%rdi, %rdx	/* RDX = dest */
	movl	$SEND, %eax
	syscall
	ret

/**========================================================================* */
/* _ipc_receive_intr(endpoint_t src, message *msg, int *status)            * */
/**========================================================================* */
ENTRY(_ipc_receive_intr)
	/* RDI = src, RSI = msg, RDX = status */
	/* Save status ptr before using RDX for src */
	push	%rdx		/* save status ptr */
	mov	%rdi, %rdx	/* RDX = src */
	movl	$RECEIVE, %eax
	syscall
	pop	%rcx		/* RCX = status ptr */
	mov	%rbx, (%rcx)	/* *status = RBX */
	ret

/**========================================================================* */
/* _ipc_sendrec_intr(endpoint_t srcdest, message *msg)                     * */
/**========================================================================* */
ENTRY(_ipc_sendrec_intr)
	/* RDI = srcdest, RSI = msg => Need: RDX=srcdest, RSI=msg */
	mov	%rdi, %rdx	/* RDX = srcdest */
	movl	$SENDREC, %eax
	syscall
	ret

/**========================================================================* */
/* _ipc_notify_intr(endpoint_t dest)                                       * */
/**========================================================================* */
ENTRY(_ipc_notify_intr)
	/* RDI = dest => Need: RDX=dest */
	mov	%rdi, %rdx	/* RDX = dest */
	movl	$NOTIFY, %eax
	syscall
	ret

/**========================================================================* */
/* _ipc_sendnb_intr(endpoint_t dest, message *msg)                         * */
/**========================================================================* */
ENTRY(_ipc_sendnb_intr)
	/* RDI = dest, RSI = msg => Need: RDX=dest, RSI=msg */
	mov	%rdi, %rdx	/* RDX = dest */
	movl	$SENDNB, %eax
	syscall
	ret

/**========================================================================* */
/* _ipc_senda_intr(struct asynmsg *table, int count)                       * */
/**========================================================================* */
ENTRY(_ipc_senda_intr)
	/* RDI = table, RSI = count => Need: RSI=table, RDX=count */
	mov	%rsi, %rdx	/* RDX = count */
	mov	%rdi, %rsi	/* RSI = table */
	movl	$SENDA, %eax
	syscall
	ret
