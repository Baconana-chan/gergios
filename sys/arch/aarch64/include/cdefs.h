/*	$NetBSD: cdefs.h,v 1.15 2014/06/23 03:40:57 christos Exp $	*/

#ifndef _AARCH64_CDEFS_H_
#define _AARCH64_CDEFS_H_

/* ARMv8-A 64-bit requires GCC 4.1+ or Clang (any) */
#ifndef __lint__
#if (__GNUC__ == 4 && __GNUC_MINOR__ < 1) || (__GNUC__ < 4 && !defined(__clang__))
#error GCC 4.1 or compatible (Clang) required.
#endif
#endif

/* AArch64 is always 64-bit, 8-byte aligned */
#define __ALIGNBYTES		(sizeof(long) - 1)

#endif /* !_AARCH64_CDEFS_H_ */
