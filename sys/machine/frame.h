/*	machine/frame.h -> arch/${MACHINE_ARCH}/include/frame.h	*/
#ifndef _MACHINE_FRAME_H_
#define _MACHINE_FRAME_H_

#ifndef _MSC_VER
#include_next <frame.h>
#else
/* MSVC: #include_next not supported — include x86_64 frame.h directly */
#include <arch/x86_64/include/frame.h>
#endif

#endif /* !_MACHINE_FRAME_H_ */
