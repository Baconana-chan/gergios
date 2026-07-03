/*	machine/mcontext.h -> arch/${MACHINE_ARCH}/include/mcontext.h	*/
#ifndef _MACHINE_MCONTEXT_H_
#define _MACHINE_MCONTEXT_H_

#ifndef _MSC_VER
#include_next <mcontext.h>
#else
/* MSVC: #include_next not supported — include x86_64 mcontext.h directly */
#include <arch/x86_64/include/mcontext.h>
#endif

#endif /* !_MACHINE_MCONTEXT_H_ */
