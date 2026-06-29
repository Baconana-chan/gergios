/*	machine/endian.h -> arch/${MACHINE_ARCH}/include/endian.h	*/
#ifndef _MACHINE_ENDIAN_H_
#define _MACHINE_ENDIAN_H_

#ifndef _MSC_VER
#include_next <endian.h>
#else
/* MSVC fallback for host tools — x86_64 is little-endian */
#ifndef _LITTLE_ENDIAN
#define _LITTLE_ENDIAN	1234
#endif
#ifndef _BIG_ENDIAN
#define _BIG_ENDIAN	4321
#endif
#define	_BYTE_ORDER		_LITTLE_ENDIAN
#endif

#endif /* !_MACHINE_ENDIAN_H_ */
