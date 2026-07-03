/*	machine/mutex.h -> arch/${MACHINE_ARCH}/include/mutex.h	*/
#ifndef _MACHINE_MUTEX_H_
#define _MACHINE_MUTEX_H_

#ifndef _MSC_VER
#include_next <mutex.h>
#else
/*
 * MSVC: #include_next is not supported.
 * Provide minimal struct kmutex definition.
 * MINIX does not use NetBSD's kmutex, but some NetBSD headers
 * (specificdata.h, mount.h, proc.h) embed kmutex_t which requires
 * a complete type.
 */
struct kmutex {
	void *mtx_opaque;
};
#endif

#endif /* !_MACHINE_MUTEX_H_ */
