/*	$NetBSD: endian.h,v 1.1 2003/04/26 18:39:44 fvdl Exp $	*/

/*
 * x86_64 is always little-endian.
 */

#ifndef _X86_64_ENDIAN_H_
#define _X86_64_ENDIAN_H_

#include <sys/endian.h>

#ifndef _POSIX_SOURCE
/* Define byte order: little-endian */
#define	_BYTE_ORDER	_LITTLE_ENDIAN
#endif /* !_POSIX_SOURCE */

#endif /* _X86_64_ENDIAN_H_ */
