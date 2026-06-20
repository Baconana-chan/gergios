/* get_bp.S */
/* return fp (x29) in x0 */

#include <machine/asm.h>

ENTRY(get_bp)
	mov	x0, x29
	ret
END(get_bp)
