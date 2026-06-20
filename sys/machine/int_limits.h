/*	$NetBSD$	*/
/* AArch64 int_limits.h — limits of specified-width integer types.
 * Uses #ifndef guards to avoid conflicts with Clang's built-in <stdint.h>. */

#ifndef _MACHINE_INT_LIMITS_H_
#define _MACHINE_INT_LIMITS_H_

/*
 * 7.18.2 Limits of specified-width integer types
 * (only define if Clang's built-in <stdint.h> hasn't already)
 */

/* 7.18.2.1 Limits of exact-width integer types */
#ifndef INT8_MIN
#define	INT8_MIN	(-0x7f-1)
#endif
#ifndef INT16_MIN
#define	INT16_MIN	(-0x7fff-1)
#endif
#ifndef INT32_MIN
#define	INT32_MIN	(-0x7fffffff-1)
#endif
#ifndef INT64_MIN
#define	INT64_MIN	(-0x7fffffffffffffffLL-1)
#endif

#ifndef INT8_MAX
#define	INT8_MAX	0x7f
#endif
#ifndef INT16_MAX
#define	INT16_MAX	0x7fff
#endif
#ifndef INT32_MAX
#define	INT32_MAX	0x7fffffff
#endif
#ifndef INT64_MAX
#define	INT64_MAX	0x7fffffffffffffffLL
#endif

#ifndef UINT8_MAX
#define	UINT8_MAX	0xff
#endif
#ifndef UINT16_MAX
#define	UINT16_MAX	0xffff
#endif
#ifndef UINT32_MAX
#define	UINT32_MAX	0xffffffffU
#endif
#ifndef UINT64_MAX
#define	UINT64_MAX	0xffffffffffffffffULL
#endif

/* 7.18.2.2 Limits of minimum-width integer types */
#ifndef INT_LEAST8_MIN
#define	INT_LEAST8_MIN	(-0x7f-1)
#endif
#ifndef INT_LEAST16_MIN
#define	INT_LEAST16_MIN	(-0x7fff-1)
#endif
#ifndef INT_LEAST32_MIN
#define	INT_LEAST32_MIN	(-0x7fffffff-1)
#endif
#ifndef INT_LEAST64_MIN
#define	INT_LEAST64_MIN	(-0x7fffffffffffffffLL-1)
#endif

#ifndef INT_LEAST8_MAX
#define	INT_LEAST8_MAX	0x7f
#endif
#ifndef INT_LEAST16_MAX
#define	INT_LEAST16_MAX	0x7fff
#endif
#ifndef INT_LEAST32_MAX
#define	INT_LEAST32_MAX	0x7fffffff
#endif
#ifndef INT_LEAST64_MAX
#define	INT_LEAST64_MAX	0x7fffffffffffffffLL
#endif

#ifndef UINT_LEAST8_MAX
#define	UINT_LEAST8_MAX	 0xff
#endif
#ifndef UINT_LEAST16_MAX
#define	UINT_LEAST16_MAX 0xffff
#endif
#ifndef UINT_LEAST32_MAX
#define	UINT_LEAST32_MAX 0xffffffffU
#endif
#ifndef UINT_LEAST64_MAX
#define	UINT_LEAST64_MAX 0xffffffffffffffffULL
#endif

/* 7.18.2.3 Limits of fastest minimum-width integer types */
#ifndef INT_FAST8_MIN
#define	INT_FAST8_MIN	(-0x7fffffff-1)
#endif
#ifndef INT_FAST16_MIN
#define	INT_FAST16_MIN	(-0x7fffffff-1)
#endif
#ifndef INT_FAST32_MIN
#define	INT_FAST32_MIN	(-0x7fffffff-1)
#endif
#ifndef INT_FAST64_MIN
#define	INT_FAST64_MIN	(-0x7fffffffffffffffLL-1)
#endif

#ifndef INT_FAST8_MAX
#define	INT_FAST8_MAX	0x7fffffff
#endif
#ifndef INT_FAST16_MAX
#define	INT_FAST16_MAX	0x7fffffff
#endif
#ifndef INT_FAST32_MAX
#define	INT_FAST32_MAX	0x7fffffff
#endif
#ifndef INT_FAST64_MAX
#define	INT_FAST64_MAX	0x7fffffffffffffffLL
#endif

#ifndef UINT_FAST8_MAX
#define	UINT_FAST8_MAX	0xffffffffU
#endif
#ifndef UINT_FAST16_MAX
#define	UINT_FAST16_MAX	0xffffffffU
#endif
#ifndef UINT_FAST32_MAX
#define	UINT_FAST32_MAX	0xffffffffU
#endif
#ifndef UINT_FAST64_MAX
#define	UINT_FAST64_MAX	0xffffffffffffffffULL
#endif

/* 7.18.2.4 Limits of integer types capable of holding object pointers */
#ifndef INTPTR_MIN
#ifdef _LP64
#define	INTPTR_MIN	(-0x7fffffffffffffffL-1)
#else
#define	INTPTR_MIN	(-0x7fffffffL-1)
#endif
#endif
#ifndef INTPTR_MAX
#ifdef _LP64
#define	INTPTR_MAX	0x7fffffffffffffffL
#else
#define	INTPTR_MAX	0x7fffffffL
#endif
#endif
#ifndef UINTPTR_MAX
#ifdef _LP64
#define	UINTPTR_MAX	0xffffffffffffffffUL
#else
#define	UINTPTR_MAX	0xffffffffUL
#endif
#endif

/* 7.18.2.5 Limits of greatest-width integer types */
#ifndef INTMAX_MIN
#define	INTMAX_MIN	(-0x7fffffffffffffffLL-1)
#endif
#ifndef INTMAX_MAX
#define	INTMAX_MAX	0x7fffffffffffffffLL
#endif
#ifndef UINTMAX_MAX
#define	UINTMAX_MAX	0xffffffffffffffffULL
#endif

/* 7.18.3 Limits of other integer types */
#ifndef PTRDIFF_MIN
#ifdef _LP64
#define	PTRDIFF_MIN	(-0x7fffffffffffffffL-1)
#else
#define	PTRDIFF_MIN	(-0x7fffffffL-1)
#endif
#endif
#ifndef PTRDIFF_MAX
#ifdef _LP64
#define	PTRDIFF_MAX	0x7fffffffffffffffL
#else
#define	PTRDIFF_MAX	0x7fffffffL
#endif
#endif

#ifndef SIG_ATOMIC_MIN
#define	SIG_ATOMIC_MIN	(-0x7fffffff-1)
#endif
#ifndef SIG_ATOMIC_MAX
#define	SIG_ATOMIC_MAX	0x7fffffff
#endif

#ifndef SIZE_MAX
#ifdef _LP64
#define	SIZE_MAX	0xffffffffffffffffUL
#else
#define	SIZE_MAX	0xffffffffUL
#endif
#endif

#endif /* !_MACHINE_INT_LIMITS_H_ */
