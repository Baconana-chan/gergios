/*	$NetBSD$	*/
/* AArch64 machine limits.h */

#ifndef _MACHINE_LIMITS_H_
#define _MACHINE_LIMITS_H_

#include <sys/featuretest.h>

#define	CHAR_BIT	8

#define	UCHAR_MAX	0xff
#define	SCHAR_MAX	0x7f
#define	SCHAR_MIN	(-0x7f-1)

#define	USHRT_MAX	0xffff
#define	SHRT_MAX	0x7fff
#define	SHRT_MIN	(-0x7fff-1)

#define	UINT_MAX	0xffffffffU
#define	INT_MAX		0x7fffffff
#define	INT_MIN		(-0x7fffffff-1)

#ifdef _LP64
#define	ULONG_MAX	0xffffffffffffffffUL
#define	LONG_MAX	0x7fffffffffffffffL
#define	LONG_MIN	(-0x7fffffffffffffffL-1)
#else
#define	ULONG_MAX	0xffffffffUL
#define	LONG_MAX	0x7fffffffL
#define	LONG_MIN	(-0x7fffffffL-1)
#endif

#if defined(_POSIX_C_SOURCE) || defined(_XOPEN_SOURCE) || \
    defined(_NETBSD_SOURCE)
#define	SSIZE_MAX	LONG_MAX

#if defined(_ISOC99_SOURCE) || (__STDC_VERSION__ - 0) >= 199901L || \
    defined(_NETBSD_SOURCE)
#define	ULLONG_MAX	0xffffffffffffffffULL
#define	LLONG_MAX	0x7fffffffffffffffLL
#define	LLONG_MIN	(-0x7fffffffffffffffLL-1)
#endif

#if defined(_NETBSD_SOURCE)
#define	SSIZE_MIN	LONG_MIN
#define	SIZE_T_MAX	ULONG_MAX

#define	UQUAD_MAX	0xffffffffffffffffULL
#define	QUAD_MAX	0x7fffffffffffffffLL
#define	QUAD_MIN	(-0x7fffffffffffffffLL-1)
#endif /* _NETBSD_SOURCE */
#endif /* _POSIX_C_SOURCE || _XOPEN_SOURCE || _NETBSD_SOURCE */

#if defined(_XOPEN_SOURCE) || defined(_NETBSD_SOURCE)
#ifdef _LP64
#define LONG_BIT	64
#else
#define LONG_BIT	32
#endif
#define WORD_BIT	32

#define DBL_DIG		__DBL_DIG__
#define DBL_MAX		__DBL_MAX__
#define DBL_MIN		__DBL_MIN__

#define FLT_DIG		__FLT_DIG__
#define FLT_MAX		__FLT_MAX__
#define FLT_MIN		__FLT_MIN__

#define LDBL_DIG	__LDBL_DIG__
#define LDBL_MAX	__LDBL_MAX__
#define LDBL_MIN	__LDBL_MIN__
#endif /* _XOPEN_SOURCE || _NETBSD_SOURCE */

#endif	/* _MACHINE_LIMITS_H_ */
