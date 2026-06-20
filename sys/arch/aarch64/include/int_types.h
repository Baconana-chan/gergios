/*	$NetBSD: int_types.h,v 1.17 2014/07/25 21:43:13 joerg Exp $	*/

#ifndef _AARCH64_INT_TYPES_H_
#define _AARCH64_INT_TYPES_H_

/* AArch64 uses Clang built-in type macros for exact-width integer types */
typedef signed char			__int8_t;
typedef unsigned char			__uint8_t;
typedef short int			__int16_t;
typedef unsigned short int		__uint16_t;
typedef int				__int32_t;
typedef unsigned int			__uint32_t;
typedef long int			__int64_t;
typedef unsigned long int		__uint64_t;

#if !defined(__minix)
#define	__BIT_TYPES_DEFINED__
#endif /* !defined(__minix) */

/* Integer types for object pointers */
typedef long int			__intptr_t;
typedef unsigned long int		__uintptr_t;

#endif /* !_AARCH64_INT_TYPES_H_ */
