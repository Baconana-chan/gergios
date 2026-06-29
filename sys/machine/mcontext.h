/*	machine/mcontext.h -> arch/${MACHINE_ARCH}/include/mcontext.h	*/
#ifndef _MACHINE_MCONTEXT_H_
#define _MACHINE_MCONTEXT_H_

#ifndef _MSC_VER
#include_next <mcontext.h>
#else
/* MSVC fallback for host tools — mcontext not used at runtime */
typedef struct { long long __mc_pad[(776 - 4 - 8) / 8]; } mcontext_t;
#endif

#endif /* !_MACHINE_MCONTEXT_H_ */
