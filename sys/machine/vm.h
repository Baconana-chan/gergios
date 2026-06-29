/*	machine/vm.h -> arch/${MACHINE_ARCH}/include/vm.h	*/
#ifndef _MACHINE_VM_H_
#define _MACHINE_VM_H_

/* Include arch-specific vm.h from the include paths.
 * Using #include <vm.h> instead of #include_next to work reliably
 * across all toolchains (Clang on Windows). */
#include <vm.h>

#endif /* !_MACHINE_VM_H_ */
