/*	$NetBSD: ansi.h,v 1.17 2014/02/24 16:57:57 christos Exp $	*/

#ifndef _AARCH64_ANSI_H_
#define _AARCH64_ANSI_H_

#include <sys/cdefs.h>
#include <machine/int_types.h>

/* Fundamental types for AArch64 ILP32 / LP64 */
#define	_BSD_CLOCK_T_		unsigned int	/* clock() */
#ifdef __PTRDIFF_TYPE__
#define	_BSD_PTRDIFF_T_		__PTRDIFF_TYPE__
#define	_BSD_SSIZE_T_		__PTRDIFF_TYPE__
#else
#define	_BSD_PTRDIFF_T_		long int
#define	_BSD_SSIZE_T_		long int
#endif
#ifdef __SIZE_TYPE__
#define	_BSD_SIZE_T_		__SIZE_TYPE__
#else
#define	_BSD_SIZE_T_		unsigned long int
#endif
#define	_BSD_TIME_T_		__int64_t	/* time() */
#define	_BSD_CLOCKID_T_		int		/* clockid_t */
#define	_BSD_TIMER_T_		int		/* timer_t */
#define	_BSD_SUSECONDS_T_	int		/* suseconds_t */
#define	_BSD_USECONDS_T_	unsigned int	/* useconds_t */
#ifdef __WCHAR_TYPE__
#define	_BSD_WCHAR_T_		__WCHAR_TYPE__
#else
#define	_BSD_WCHAR_T_		int
#endif
#ifdef __WINT_TYPE__
#define	_BSD_WINT_T_		__WINT_TYPE__
#else
#define	_BSD_WINT_T_		int
#endif

#endif /* !_AARCH64_ANSI_H_ */
