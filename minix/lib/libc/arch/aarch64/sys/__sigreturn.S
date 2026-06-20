/* This routine is the low-level code for returning from signals.   */
/* It calls _sigreturn, which is the normal "system call" routine. */
/* Both __sigreturn and _sigreturn are needed. */
#include <machine/asm.h>

IMPORT(sigreturn)
ENTRY(__sigreturn)
	ldr	x0, [sp], #8		/* load sigframe.sf_scp into x0 as parameter */
	b	_C_LABEL(sigreturn)	/* _sigreturn(struct sigcontext *sf_scpcopy) */
END(__sigreturn)
