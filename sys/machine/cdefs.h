/*	machine/cdefs.h -> arch/${MACHINE_ARCH}/include/cdefs.h	*/
#ifndef _MACHINE_CDEFS_H_
#define _MACHINE_CDEFS_H_

/* Use include_next to find the arch-specific cdefs.h
 * (e.g., sys/arch/aarch64/include/cdefs.h via -I.../sys/arch/aarch64/include)
 */
#ifndef _MSC_VER
#include_next <cdefs.h>
#endif

#endif /* !_MACHINE_CDEFS_H_ */
