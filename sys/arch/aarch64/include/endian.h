/*	$NetBSD$	*/

/*
 * ARM64 is always little-endian in AArch64 execution state.
 * (AArch64 supports both LE and BE, but MINIX targets LE.)
 */

#ifndef _AARCH64_ENDIAN_H_
#define _AARCH64_ENDIAN_H_

#include <sys/endian.h>

#ifndef _POSIX_SOURCE
/* Define byte order: little-endian */
#define	_BYTE_ORDER	_LITTLE_ENDIAN
#endif /* !_POSIX_SOURCE */

#endif /* _AARCH64_ENDIAN_H_ */
