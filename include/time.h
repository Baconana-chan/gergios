/*	$NetBSD: time.h,v 1.32 2005/12/26 18:43:46 perry Exp $	*/

/*
 * Copyright (c) 1989, 1993
 *	The Regents of the University of California.  All rights reserved.
 * (c) UNIX System Laboratories, Inc.
 *
 * Minimal <time.h> for bare-metal x86_64-elf MINIX builds.
 * Wraps existing MINIX type definitions from <sys/types.h>
 * and <sys/timespec.h> to provide a standard libc <time.h> 
 * interface for wolfSSL and other libc-dependent code.
 *
 * NOTE: Function declarations are provided for compilation only.
 * wolfSSL uses XTIME() macro which can be overridden. On bare-metal
 * targets where time() is not available, define XTIME(t)=((time_t)0)
 * in wolfSSL compile definitions to avoid runtime crashes.
 */

#ifndef _TIME_H_
#define	_TIME_H_

#include <sys/cdefs.h>

/* Get time_t, clock_t, clockid_t from MINIX's standard types.
 * These are defined via _BSD_TIME_T_ / _BSD_CLOCK_T_ / _BSD_CLOCKID_T_
 * from <machine/ansi.h> included transitively through <sys/types.h>. */
#include <sys/types.h>

/* Get struct timespec and struct itimerspec from MINIX's time header.
 * sys/sys/time.h defines both timespec and itimerspec. */
#include <sys/time.h>

/* struct tm — standard broken-down time structure.
 * Not provided by MINIX system headers; define here for wolfSSL. */
struct tm {
	int	tm_sec;		/* seconds after the minute [0-61] */
	int	tm_min;		/* minutes after the hour [0-59] */
	int	tm_hour;	/* hours since midnight [0-23] */
	int	tm_mday;	/* day of the month [1-31] */
	int	tm_mon;		/* months since January [0-11] */
	int	tm_year;	/* years since 1900 */
	int	tm_wday;	/* days since Sunday [0-6] */
	int	tm_yday;	/* days since January 1 [0-365] */
	int	tm_isdst;	/* Daylight Saving Time flag */
	long	tm_gmtoff;	/* offset from UTC in seconds */
	const char *tm_zone;	/* timezone abbreviation */
};

__BEGIN_DECLS
/* Standard time functions — declarations only (no weak attribute).
 * wolfSSL uses XTIME() macro which can be overridden. On bare-metal
 * x86_64-elf, define -DXTIME(t)=((time_t)0) in wolfSSL config to
 * avoid link errors. */
time_t	time(time_t *);
double	difftime(time_t, time_t);
time_t	mktime(struct tm *);
struct tm *gmtime(const time_t *);
struct tm *localtime(const time_t *);
char	*asctime(const struct tm *);
char	*ctime(const time_t *);
size_t	strftime(char *, size_t, const char *, const struct tm *);
clock_t	clock(void);
int	clock_gettime(clockid_t, struct timespec *);
int	clock_settime(clockid_t, const struct timespec *);
int	clock_getres(clockid_t, struct timespec *);
int	nanosleep(const struct timespec *, struct timespec *);
__END_DECLS

#endif /* !_TIME_H_ */
