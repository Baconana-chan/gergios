/*	$NetBSD$	*/
/* AArch64 wchar_limits.h — limits of wchar_t and wint_t.
 * Uses #ifndef guards to avoid conflicts with Clang's built-in <stdint.h>. */

#ifndef _MACHINE_WCHAR_LIMITS_H_
#define _MACHINE_WCHAR_LIMITS_H_

/* 7.18.3 Limits of other integer types */

/* limits of wchar_t */
#ifndef WCHAR_MIN
#ifdef __WCHAR_MIN__
#define	WCHAR_MIN	__WCHAR_MIN__
#elif __WCHAR_UNSIGNED__
#define	WCHAR_MIN	0U
#else
#define	WCHAR_MIN	(-0x7fffffff-1)
#endif
#endif

#ifndef WCHAR_MAX
#ifdef __WCHAR_MAX__
#define	WCHAR_MAX	__WCHAR_MAX__
#elif __WCHAR_UNSIGNED__
#define	WCHAR_MAX	0xffffffffU
#else
#define	WCHAR_MAX	0x7fffffff
#endif
#endif

/* limits of wint_t */
#ifndef WINT_MIN
#ifdef __WINT_MIN__
#define	WINT_MIN	__WINT_MIN__
#elif __WINT_UNSIGNED__
#define	WINT_MIN	0U
#else
#define	WINT_MIN	(-0x7fffffff-1)
#endif
#endif

#ifndef WINT_MAX
#ifdef __WINT_MAX__
#define	WINT_MAX	__WINT_MAX__
#elif __WINT_UNSIGNED__
#define	WINT_MAX	0xffffffffU
#else
#define	WINT_MAX	0x7fffffff
#endif
#endif

#endif /* !_MACHINE_WCHAR_LIMITS_H_ */
