/*	machine/ports.h -> arch/${MACHINE_ARCH}/include/ports.h	*/
#ifndef _MACHINE_PORTS_H_
#define _MACHINE_PORTS_H_

#ifndef _MSC_VER
#include_next <ports.h>
#else
/* MSVC: direct include of arch-specific ports.h (TIMER0, TIMER_MODE) */
#include <ports.h>
#endif

#endif /* !_MACHINE_PORTS_H_ */
