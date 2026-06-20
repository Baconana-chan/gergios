/*	$NetBSD$	*/
/* AArch64 int_const.h — macros for integer constants.
 * Uses #ifndef guards to avoid conflicts with Clang's built-in <stdint.h>. */

#ifndef _MACHINE_INT_CONST_H_
#define _MACHINE_INT_CONST_H_

/*
 * 7.18.4 Macros for integer constants
 * (only define if Clang's built-in <stdint.h> hasn't already)
 */

/* 7.18.4.1 Macros for minimum-width integer constants */
#ifndef INT8_C
#define	INT8_C(c)	c
#endif
#ifndef INT16_C
#define	INT16_C(c)	c
#endif
#ifndef INT32_C
#define	INT32_C(c)	c
#endif
#ifndef INT64_C
#define	INT64_C(c)	c ## LL
#endif

#ifndef UINT8_C
#define	UINT8_C(c)	c
#endif
#ifndef UINT16_C
#define	UINT16_C(c)	c
#endif
#ifndef UINT32_C
#define	UINT32_C(c)	c ## U
#endif
#ifndef UINT64_C
#define	UINT64_C(c)	c ## ULL
#endif

/* 7.18.4.2 Macros for greatest-width integer constants */
#ifndef INTMAX_C
#define	INTMAX_C(c)	c ## LL
#endif
#ifndef UINTMAX_C
#define	UINTMAX_C(c)	c ## ULL
#endif

#endif /* !_MACHINE_INT_CONST_H_ */
