/*	machine/ansi.h -> arch/${MACHINE_ARCH}/include/ansi.h	*/
#ifndef _MACHINE_ANSI_H_
#define _MACHINE_ANSI_H_

#ifndef _MSC_VER
#include_next <ansi.h>
#else
/* MSVC fallback for host tools — arch-specific ansi.h not available */
/* Note: ptrdiff_t, size_t, wchar_t, wint_t are provided by MSVC CRT */
#define	_BSD_CLOCK_T_		unsigned int
#define	_BSD_SSIZE_T_		long long
#define	_BSD_TIME_T_		__int64_t
#define	_BSD_CLOCKID_T_		int
#define	_BSD_TIMER_T_		int
#define	_BSD_SUSECONDS_T_	int
#define	_BSD_USECONDS_T_	unsigned int
#endif

#endif /* !_MACHINE_ANSI_H_ */
