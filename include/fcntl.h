/*	$NetBSD: fcntl.h,v 1.34 2009/04/12 04:14:32 lukem Exp $	*/

/*
 * Minimal fcntl.h for bare-metal builds (x86_64-elf / aarch64-elf).
 *
 * Provides the basic POSIX file control constants needed by
 * wolfSSL at compile time. On bare-metal targets, these are
 * only used for #ifdef consistency — actual file I/O is stubbed.
 */

#ifndef _FCNTL_H_
#define _FCNTL_H_

/* File access modes */
#ifndef O_RDONLY
#define	O_RDONLY	0x0000		/* open for reading only */
#endif
#ifndef	O_WRONLY
#define	O_WRONLY	0x0001		/* open for writing only */
#endif
#ifndef	O_RDWR
#define	O_RDWR		0x0002		/* open for reading and writing */
#endif
#ifndef	O_ACCMODE
#define	O_ACCMODE	0x0003		/* mask for file access modes */
#endif

/* File status flags and open flags */
#ifndef	O_CREAT
#define	O_CREAT		0x0200		/* create if nonexistent */
#endif
#ifndef	O_TRUNC
#define	O_TRUNC		0x0400		/* truncate to zero length */
#endif
#ifndef	O_EXCL
#define	O_EXCL		0x0800		/* error if already exists */
#endif
#ifndef	O_APPEND
#define	O_APPEND	0x0008		/* set append mode */
#endif
#ifndef	O_NONBLOCK
#define	O_NONBLOCK	0x0004		/* no delay */
#endif
#ifndef	O_SYNC
#define	O_SYNC		0x0080		/* synchronous writes */
#endif
#ifndef	O_DSYNC
#define	O_DSYNC		0x0100		/* synchronous data writes */
#endif
#ifndef	O_RSYNC
#define	O_RSYNC		0x0200		/* synchronous reads */
#endif
#ifndef	O_NOCTTY
#define	O_NOCTTY	0x8000		/* don't assign controlling terminal */
#endif
#ifndef	O_CLOEXEC
#define	O_CLOEXEC	0x4000		/* set close_on_exec */
#endif
#ifndef	O_DIRECTORY
#define	O_DIRECTORY	0x2000		/* must be a directory */
#endif
#ifndef	O_NOFOLLOW
#define	O_NOFOLLOW	0x0100		/* don't follow symlinks */
#endif

/* File creation flags */
#ifndef	O_SHLOCK
#define	O_SHLOCK	0x0010		/* open with shared file lock */
#endif
#ifndef	O_EXLOCK
#define	O_EXLOCK	0x0020		/* open with exclusive file lock */
#endif

/* Advisory file locking */
#ifndef	F_DUPFD
#define	F_DUPFD		0		/* duplicate fd */
#endif
#ifndef	F_GETFD
#define	F_GETFD		1		/* get fd flags */
#endif
#ifndef	F_SETFD
#define	F_SETFD		2		/* set fd flags */
#endif
#ifndef	F_GETFL
#define	F_GETFL		3		/* get file status flags */
#endif
#ifndef	F_SETFL
#define	F_SETFL		4		/* set file status flags */
#endif
#ifndef	F_GETOWN
#define	F_GETOWN		5		/* get SIGIO/SIGURG owner */
#endif
#ifndef	F_SETOWN
#define	F_SETOWN		6		/* set SIGIO/SIGURG owner */
#endif
#ifndef	F_GETLK
#define	F_GETLK		7		/* get record locking information */
#endif
#ifndef	F_SETLK
#define	F_SETLK		8		/* set record locking information */
#endif
#ifndef	F_SETLKW
#define	F_SETLKW	9		/* F_SETLK; wait if blocked */
#endif

/* Close-on-exec flag */
#ifndef	FD_CLOEXEC
#define	FD_CLOEXEC	1		/* close on exec */
#endif

/* Functions */
#ifndef __BAREMETAL_NO_FCNTL
int	open(const char *, int, ...);
int	fcntl(int, int, ...);
int	creat(const char *, int);
#endif

#endif /* !_FCNTL_H_ */
