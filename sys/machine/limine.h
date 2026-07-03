/* machine/limine.h -> arch/${MACHINE_ARCH}/include/limine.h */
#ifndef _MACHINE_LIMINE_H_
#define _MACHINE_LIMINE_H_

#ifndef _MSC_VER
#include_next <limine.h>
#else
/* MSVC: #include_next not supported — include x86_64 limine.h directly */
#include <arch/x86_64/include/limine.h>
#endif

#endif /* !_MACHINE_LIMINE_H_ */
