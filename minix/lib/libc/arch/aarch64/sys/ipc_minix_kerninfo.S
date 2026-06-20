#include <minix/ipcconst.h>
#include <machine/asm.h>

ENTRY(ipc_minix_kerninfo)
	stp	x29, x30, [sp, #-16]!
	mov	x29, sp
	str	x0, [sp, #-16]!		/* Save return struct ptr */

	mov	x1, #0
	mov	x2, #0
	mov	x0, #MINIX_KERNINFO	/* _ipc_minix_kerninfo() */
	mov	x3, #IPCVEC_INTR	/* x3 determines the SVC type */
	svc	#0			/* trap to kernel */

	ldr	x2, [sp], #16		/* x2 = return struct ptr (was x0) */
	str	x1, [x2]		/* Store kernel info pointer */
	ldp	x29, x30, [sp], #16
	ret
END(ipc_minix_kerninfo)
