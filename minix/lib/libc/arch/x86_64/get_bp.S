/* get_bp.S - x86_64 version */
/* return RBP in RAX */

#include <machine/asm.h>

ENTRY(get_bp)
	movq	%rbp, %rax
	ret
