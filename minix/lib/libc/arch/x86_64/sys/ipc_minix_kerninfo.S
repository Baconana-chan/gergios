#include <minix/ipcconst.h>
#include <machine/asm.h>

/* ipc_minix_kerninfo(struct minix_kerninfo **) - x86_64 version
 * Retrieves kernel info pointer.
 *
 * Kernel convention:
 *   RAX = MINIX_KERNINFO
 *   Return: RBX = kernel info pointer
 */
ENTRY(ipc_minix_kerninfo)
	/* RDI = return struct ptr */
	push	%rdi			/* save return ptr */
	movl	$MINIX_KERNINFO, %eax
	syscall
	pop	%rdi			/* restore return ptr */
	mov	%rbx, (%rdi)		/* *ptr = RBX (kerninfo) */
	ret
