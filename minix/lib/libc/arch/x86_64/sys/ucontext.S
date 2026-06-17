#include <machine/asm.h>
#include <ucontextoffsets.h>

IMPORT(getuctx)
IMPORT(setuctx)
IMPORT(resumecontext)

	.globl	_C_LABEL(__errno)

/* int getcontext(ucontext_t *ucp) - x86_64 version
 *	Initialise the structure pointed to by ucp to the current user context
 *	of the calling thread. */
ENTRY(getcontext)
ENTRY(_getcontext)
	mov	%rdi, %rdx		/* edx = ucp */
	/* Check null pointer */
	test	%rdx, %rdx
	jnz	3f			/* Not null, continue */
	PIC_PROLOGUE
	call	PIC_PLT(_C_LABEL(__errno))
	PIC_EPILOGUE
	movl	$EFAULT, (%rax)
	xor	%eax, %eax
	dec	%eax			/* return -1 */
	ret

3:	/* Check flags */
	mov	UC_FLAGS(%rdx), %eax	/* eax = ucp->uc_flags */
	and	$[_UC_IGNFPU|_UC_IGNSIGM], %eax
	cmp	$[_UC_IGNFPU|_UC_IGNSIGM], %eax
	jz	5f			/* Ignore both, skip getuctx */
	PIC_PROLOGUE
	push	%rdx			/* push ucp as argument */
	call	PIC_PLT(_C_LABEL(getuctx))	/* getuctx(ucp) */
	pop	%rdx			/* restore ucp */
	PIC_EPILOGUE

5:
	/* Save the context */
	pop	PC(%rdx)		/* Save real RTA in mcp struct */
	mov	%rsp, SP(%rdx)		/* Save stack pointer */
	/* Save GP registers (except RAX and RDX) */
	mov	%rbp, BP(%rdx)		/* Save RBP */
	mov	%rsi, SI(%rdx)		/* Save RSI */
	mov	%rdi, DI(%rdx)		/* Save RDI */
	mov	%rbx, BX(%rdx)		/* Save RBX */
	mov	%rcx, CX(%rdx)		/* Save RCX */
	movl	$MCF_MAGIC, MAGIC(%rdx)	/* Set magic value */
	xor	%eax, %eax		/* Return 0 */
	jmp	*PC(%rdx)		/* Return to return address */


/* int setcontext(const ucontext_t *ucp) - x86_64 version
 *	Restore the user context pointed to by ucp. */
ENTRY(setcontext)
	mov	%rdi, %rdx		/* edx = ucp */

	/* Check null pointer */
	test	%rdx, %rdx
	jnz	3f			/* Not null, continue */
	movl	$EFAULT, %edx
0:	push	%rdx			/* preserve errno */
	PIC_PROLOGUE
	call	PIC_PLT(_C_LABEL(__errno))
	PIC_EPILOGUE
	pop	%rdx
	movl	%edx, (%rax)
	xor	%eax, %eax
	dec	%eax			/* return -1 */
	ret

3:	/* Check flags */
	cmpl	$MCF_MAGIC, MAGIC(%rdx)	/* is the magic value set? */
	jz	4f			/* yes, proceed */
	movl	$EINVAL, %edx		/* not set, return error */
	jmp	0b

4:	mov	UC_FLAGS(%rdx), %eax	/* eax = ucp->uc_flags */
	and	$[_UC_IGNFPU|_UC_IGNSIGM], %eax
	cmp	$[_UC_IGNFPU|_UC_IGNSIGM], %eax
	jz	5f			/* Ignore both */

	PIC_PROLOGUE
	push	%rdx			/* push ucp */
	call	PIC_PLT(_C_LABEL(setuctx))	/* setuctx(ucp) */
	pop	%rdx			/* restore ucp */
	PIC_EPILOGUE

5:	/* Restore the registers (except RAX and RDX) */
	mov	CX(%rdx), %rcx		/* Restore RCX */
	mov	BX(%rdx), %rbx		/* Restore RBX */
	mov	DI(%rdx), %rdi		/* Restore RDI */
	mov	SI(%rdx), %rsi		/* Restore RSI */
	mov	BP(%rdx), %rbp		/* Restore RBP */
	mov	SP(%rdx), %rsp		/* Restore stack pointer */
	xor	%eax, %eax		/* Return 0 */
	jmp	*PC(%rdx)		/* Return to RTA */

/* void ctx_start((void *func)(int arg1, ..., argn), arg1, ..., argn,
 *		  ucontext_t *ucp) - x86_64 version
 *	A wrapper to start function `func'. R12 register contains a pointer
 *	to ucp on the stack. By setting RSP to R12, we effectively 'remove' all
 *	arguments to `func' from the stack. Finally, a call to resumecontext
 *	will start the next context in the linked list.
 *
 * In the x86_64 ABI, the first 6 args are passed in registers (RDI, RSI,
 * RDX, RCX, R8, R9) and additional args on the stack. The func address
 * is at the top of the stack (popped by the first `pop`).
 */
ENTRY(ctx_start)
	/* Stack layout on entry:
	 *   [RSP+0] = func (return address for 'call')
	 *   [RSP+8] = ucp (stored by makecontext)
	 *
	 * Makecontext sets up:
	 *   RDI-R9 = arguments 1-6
	 *   Stack has any additional arguments (beyond 6)
	 *   R12 = pointer to ucp location on stack
	 *
	 * We need to:
	 * 1. Pop func from stack and call it
	 * 2. After func returns, call resumecontext(ucp) using R12
	 */
	pop	%rax			/* RAX = func */
	call	*%rax			/* func(args...) */
	PIC_PROLOGUE
	mov	%r12, %rdi		/* RDI = ucp (from R12, callee-saved) */
	call	PIC_PLT(_C_LABEL(resumecontext))	/* resumecontext(ucp) */
	ret				/* never reached */
