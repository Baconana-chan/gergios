#include <minix/ipcconst.h>
#include <machine/asm.h>

/* _do_kernel_call_intr(message *m_ptr) - x86_64 version
 * Pass the message pointer to the kernel via SYSCALL.
 *
 * Kernel convention from mpx.S SYSCALL handler:
 *   RAX = KERNEL_CALL (syscall number for kernel calls)
 *   RDX = message pointer
 */
ENTRY(_do_kernel_call_intr)
	mov	%rdi, %rdx	/* RDX = message pointer */
	movl	$KERNEL_CALL, %eax
	syscall
	ret
