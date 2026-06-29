/*	x86_64 cdefs.h	*/

#ifndef _X86_64_CDEFS_H_
#define _X86_64_CDEFS_H_

/* x86_64 requires GCC 4.1+ or Clang (any) */
#ifndef __lint__
#if (__GNUC__ == 4 && __GNUC_MINOR__ < 1) || (__GNUC__ < 4 && !defined(__clang__))
#error GCC 4.1 or compatible (Clang) required.
#endif
#endif

/* x86_64 is always 64-bit, 8-byte aligned */
#define __ALIGNBYTES		(sizeof(long) - 1)

#endif /* !_X86_64_CDEFS_H_ */
