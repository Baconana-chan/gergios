/*	getprocessor() - determine processor type for x86_64	*/
#include <machine/asm.h>

/* int getprocessor(void); */
/*	Return 386, 486, 586, ... (x86_64 always >= 586) */
ENTRY(getprocessor)
	push	%rbp
	movq	%rsp, %rbp

	/* On x86_64, CPUID is always available. */
	push	%rbx
	movl	$1, %eax
	cpuid
	andb	$0x0F, %ah	/* Extract the family (5, 6, ...) */
	movzbl	%ah, %eax
	cmpl	$15, %eax	/* 15: extended family */
	jne	direct
	movl	$6, %eax	/* Make it 686 */
direct:
	imull	$100, %eax	/* 500, 600, ... */
	addl	$86, %eax	/* 586, 686, ... */
	pop	%rbx

	leave
	ret
