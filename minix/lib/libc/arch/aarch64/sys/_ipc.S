#include <minix/ipcconst.h>
#include <machine/asm.h>

/**========================================================================* */
/*                           IPC assembly routines                          */
/**========================================================================* */

ENTRY(_ipc_send_intr)
	stp	x29, x30, [sp, #-16]!
	mov	x29, sp
	mov	x2, x1			/* x2 = msg ptr */
	mov	x1, x0			/* x1 = src_dest */
	mov	x0, #SEND		/* _ipc_send(dest, ptr) */
	mov	x3, #IPCVEC_INTR	/* x3 determines the SVC type */
	svc	#0			/* trap to kernel */
	ldp	x29, x30, [sp], #16
	ret
END(_ipc_send_intr)

ENTRY(_ipc_receive_intr)
	stp	x29, x30, [sp, #-16]!
	mov	x29, sp
	str	x2, [sp, #-16]!		/* Save status ptr */
	mov	x2, x1			/* x2 = msg ptr */
	mov	x1, x0			/* x1 = src_dest */
	mov	x0, #RECEIVE		/* _ipc_receive(src, ptr) */
	mov	x3, #IPCVEC_INTR	/* x3 determines the SVC type */
	svc	#0			/* trap to kernel */
	ldr	x2, [sp], #16		/* Restore status ptr */
	str	x1, [x2]		/* Store status */
	ldp	x29, x30, [sp], #16
	ret
END(_ipc_receive_intr)

ENTRY(_ipc_sendrec_intr)
	stp	x29, x30, [sp, #-16]!
	mov	x29, sp
	mov	x2, x1			/* x2 = msg ptr */
	mov	x1, x0			/* x1 = src_dest */
	mov	x0, #SENDREC		/* _ipc_sendrec(srcdest, ptr) */
	mov	x3, #IPCVEC_INTR	/* x3 determines the SVC type */
	svc	#0			/* trap to kernel */
	ldp	x29, x30, [sp], #16
	ret
END(_ipc_sendrec_intr)

ENTRY(_ipc_notify_intr)
	stp	x29, x30, [sp, #-16]!
	mov	x29, sp
	mov	x1, x0			/* x1 = src_dest */
	mov	x0, #NOTIFY		/* _ipc_notify(srcdst) */
	mov	x3, #IPCVEC_INTR	/* x3 determines the SVC type */
	svc	#0			/* trap to kernel */
	ldp	x29, x30, [sp], #16
	ret
END(_ipc_notify_intr)

ENTRY(_ipc_sendnb_intr)
	stp	x29, x30, [sp, #-16]!
	mov	x29, sp
	mov	x2, x1			/* x2 = msg ptr */
	mov	x1, x0			/* x1 = src_dest */
	mov	x0, #SENDNB		/* _ipc_sendnb(dest, ptr) */
	mov	x3, #IPCVEC_INTR	/* x3 determines the SVC type */
	svc	#0			/* trap to kernel */
	ldp	x29, x30, [sp], #16
	ret
END(_ipc_sendnb_intr)

ENTRY(_ipc_senda_intr)
	stp	x29, x30, [sp, #-16]!
	mov	x29, sp
	mov	x2, x0			/* x2 = table */
	/* x1 already holds count */
	mov	x0, #SENDA		/* _ipc_senda(table, count) */
	mov	x3, #IPCVEC_INTR	/* x3 determines the SVC type */
	svc	#0			/* trap to kernel */
	ldp	x29, x30, [sp], #16
	ret
END(_ipc_senda_intr)
