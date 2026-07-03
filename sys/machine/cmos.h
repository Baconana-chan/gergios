/*	machine/cmos.h -> arch/${MACHINE_ARCH}/include/cmos.h	*/
#ifndef _MACHINE_CMOS_H_
#define _MACHINE_CMOS_H_

#ifndef _MSC_VER
#include_next <cmos.h>
#else
/* MSVC: #include_next not supported — include x86_64 cmos.h directly */
#include <arch/x86_64/include/cmos.h>
#endif

#endif /* !_MACHINE_CMOS_H_ */
