/* This routine is the low-level code for returning from signals. */
/* It calls _sigreturn, which is the normal "system call" routine. */
#include <machine/asm.h>

IMPORT(sigreturn)
ENTRY(__sigreturn)
	addq	$16, %rsp		/* Skip past sigcontext struct */
#ifndef __PIC__
	jmp	_C_LABEL(sigreturn)
#else
	PIC_PROLOGUE			/* push %rbx, but we do not care */
	pop	%rax			/* discard pushed RBX */
	jmp	PIC_PLT(_C_LABEL(sigreturn))
#endif /* PIC */
