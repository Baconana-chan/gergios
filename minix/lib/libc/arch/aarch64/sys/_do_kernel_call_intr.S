#include <minix/ipcconst.h>
#include <machine/asm.h>

ENTRY(_do_kernel_call_intr)
	/* x0 already holds msg ptr */
	mov	x3, #KERVEC_INTR	/* x3 determines the SVC type */
	svc	#0			/* trap to kernel */
	ret
END(_do_kernel_call_intr)
