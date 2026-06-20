#include <machine/asm.h>
#include <ucontextoffsets.h>

IMPORT(getuctx)
IMPORT(setuctx)
IMPORT(resumecontext)

/* int getcontext(ucontext_t *ucp)
 *	Initialise the structure pointed to by ucp to the current user context
 *	of the calling thread.
 *
 *   ARM64 AAPCS64 callee-saved registers: x19-x28, x29(FP), x30(LR)
 *   Also saves: SP, PC (return address) */
ENTRY(getcontext)
ENTRY(_getcontext)
	/* Save return address: when setcontext or swapcontext is called,
	 * we jump to this address and continue running. */
	mov	x3, x30			/* Save return address */

	/* x0 = ucp */

	/* Check null pointer */
	cmp	x0, #0			/* ucp == NULL? */
	b.ne	3f			/* Not null, continue */
	adrp	x2, errno
	add	x2, x2, :lo12:errno
	mov	x1, #EFAULT
	str	w1, [x2]		/* errno = EFAULT */
	mov	x0, #-1			/* return -1 */
	ret

3:	/* Check flags */
	ldr	w1, [x0, #UC_FLAGS]	/* w1 = ucp->uc_flags */
	and	w1, w1, #(_UC_IGNFPU|_UC_IGNSIGM)
	cmp	w1, #(_UC_IGNFPU|_UC_IGNSIGM) /* Allowed to ignore both? */
	b.eq	1f			/* If so, skip getuctx */

0:	stp	x0, x3, [sp, #-16]!
	bl	_C_LABEL(getuctx)	/* getuctx(ucp) */
	ldp	x0, x3, [sp], #16

1:	/* Save the context */
	mov	x30, x3			/* Restore lr */

	/* Save callee-saved registers (x19-x28) */
	str	x19, [x0, #REG19]
	str	x20, [x0, #REG20]
	str	x21, [x0, #REG21]
	str	x22, [x0, #REG22]
	str	x23, [x0, #REG23]
	str	x24, [x0, #REG24]
	str	x25, [x0, #REG25]
	str	x26, [x0, #REG26]
	str	x27, [x0, #REG27]
	str	x28, [x0, #REG28]

	/* Save FP (x29), LR (x30), SP, PC */
	str	x29, [x0, #FPREG]	/* Save frame pointer */
	str	x30, [x0, #LRREG]	/* Save link register */
	mov	x1, sp
	str	x1, [x0, #SPREG]	/* Save stack pointer */
	str	x30, [x0, #PCREG]	/* Save real RTA as PC */

	/* Set magic value */
	ldr	x1, =MCF_MAGIC
	str	x1, [x0, #MAGIC]

	/* Save return value (0) */
	str	xzr, [x0, #REG0]	/* Return 0 in saved x0 */
	mov	x0, #0			/* Return 0 */

2:	ret


/* int setcontext(const ucontext_t *ucp)
 *	Restore the user context pointed to by ucp. A successful call to
 *	setcontext does not return; program execution resumes at the point
 *	specified by the ucp argument. */
ENTRY(setcontext)
	/* x0 = ucp */

	/* Check null pointer */
	cmp	x0, #0			/* ucp == NULL? */
	b.ne	3f			/* Not null, continue */
	adrp	x2, errno
	add	x2, x2, :lo12:errno
	mov	x1, #EFAULT
	str	w1, [x2]		/* errno = EFAULT */
	mov	x0, #-1			/* return -1 */
	ret

3:	/* Check magic value */
	ldr	x1, [x0, #MAGIC]	/* x1 = ucp->mc_magic */
	adrp	x2, MCF_MAGIC
	add	x2, x2, :lo12:MCF_MAGIC
	ldr	x2, [x2]
	cmp	x1, x2			/* Is the context valid? */
	b.eq	4f			/* Valid, proceed */
	adrp	x2, errno
	add	x2, x2, :lo12:errno
	mov	x1, #EINVAL
	str	w1, [x2]		/* errno = EINVAL */
	mov	x0, #-1			/* return -1 */
	ret

4:	/* Check flags */
	ldr	w1, [x0, #UC_FLAGS]	/* w1 = ucp->uc_flags */
	and	w1, w1, #(_UC_IGNFPU|_UC_IGNSIGM)
	cmp	w1, #(_UC_IGNFPU|_UC_IGNSIGM) /* Allowed to ignore both? */
	b.eq	1f			/* Skip setuctx if allowed */

	stp	x0, x3, [sp, #-16]!
0:	bl	_C_LABEL(setuctx)	/* setuctx(ucp) */
	ldp	x0, x3, [sp], #16

1:	/* Restore the registers */
	/* Restore callee-saved registers (x19-x28) */
	ldr	x19, [x0, #REG19]
	ldr	x20, [x0, #REG20]
	ldr	x21, [x0, #REG21]
	ldr	x22, [x0, #REG22]
	ldr	x23, [x0, #REG23]
	ldr	x24, [x0, #REG24]
	ldr	x25, [x0, #REG25]
	ldr	x26, [x0, #REG26]
	ldr	x27, [x0, #REG27]
	ldr	x28, [x0, #REG28]

	/* Restore FP (x29), LR (x30), SP */
	ldr	x29, [x0, #FPREG]	/* Restore frame pointer */
	ldr	x30, [x0, #LRREG]	/* Restore link register */
	ldr	x2,  [x0, #SPREG]	/* Restore stack pointer */
	mov	sp, x2

	/* Load return value (x0) and jump to PC */
	ldr	x1,  [x0, #PCREG]	/* x1 = return address (PC) */
	ldr	x0,  [x0, #REG0]	/* Restore x0 (return value) */
2:	br	x1			/* Jump to PC */


/* void ctx_start()
 *	A wrapper to call resumecontext. Makecontext puts the ucp in x19.
 *	This function moves the ucp into x0 so that the ucp is the first
 *	parameter for resumecontext. The call to resumecontext will start
 *	the next context in the linked list (or exit the program if there
 *	is no context). */
ENTRY(ctx_start)
	mov	x0, x19
	b	_C_LABEL(resumecontext)
END(ctx_start)
