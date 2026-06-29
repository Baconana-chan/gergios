/*	machine/types.h -> arch/${MACHINE_ARCH}/include/types.h	*/
#ifndef _MACHINE_TYPES_H_
#define _MACHINE_TYPES_H_

#ifndef _MSC_VER
#include_next <types.h>
#else
/* MSVC fallback for host tools — arch-specific types.h not available */
#include <machine/int_types.h>
typedef unsigned long	paddr_t;
typedef unsigned long	psize_t;
typedef unsigned long	vaddr_t;
typedef unsigned long	vsize_t;
typedef long		register_t;
typedef int		register32_t;
typedef unsigned char	__cpu_simple_lock_nv_t;
#define	__SIMPLELOCK_LOCKED	1
#define	__SIMPLELOCK_UNLOCKED	0
#endif

#endif /* !_MACHINE_TYPES_H_ */
