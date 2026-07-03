/*	machine/setjmp.h -> arch/${MACHINE_ARCH}/include/setjmp.h	*/
#ifndef _MACHINE_SETJMP_H_
#define _MACHINE_SETJMP_H_

#ifndef _MSC_VER
#include_next <setjmp.h>
#else
/* MSVC: #include_next not supported — _JBLEN needed by <setjmp.h> */
#include <arch/x86_64/include/setjmp.h>
#endif

#endif /* !_MACHINE_SETJMP_H_ */
