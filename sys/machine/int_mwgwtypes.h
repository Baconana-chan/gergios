/*	$NetBSD$	*/
/* AArch64 int_mwgwtypes.h — minimum-width and fastest minimum-width integer types.
 * Uses __UINT_FAST64_TYPE__ as guard — when Clang predefines it, Clang's
 * built-in <stdint.h> already provides these types and typedefs. */

#ifndef _MACHINE_INT_MWGWTYPES_H_
#define _MACHINE_INT_MWGWTYPES_H_

#ifndef __UINT_FAST64_TYPE__

/*
 * 7.18.1 Integer types
 * Only reachable on compilers that don't predefine __UINT_FAST64_TYPE__
 * (modern Clang and GCC always define it).
 */

/* 7.18.1.2 Minimum-width integer types */
typedef signed char		 int_least8_t;
typedef unsigned char		uint_least8_t;
typedef short int		 int_least16_t;
typedef unsigned short int	uint_least16_t;
typedef int			 int_least32_t;
typedef unsigned int		uint_least32_t;

/* On LP64, __int64_t is long, not long long — use the same type */
#ifdef _LP64
typedef long int		 int_least64_t;
typedef unsigned long int	uint_least64_t;
#else
typedef long long int		 int_least64_t;
typedef unsigned long long int	uint_least64_t;
#endif

/* 7.18.1.3 Fastest minimum-width integer types */
typedef int			 int_fast8_t;
typedef unsigned int		uint_fast8_t;
typedef int			 int_fast16_t;
typedef unsigned int		uint_fast16_t;
typedef int			 int_fast32_t;
typedef unsigned int		uint_fast32_t;

#ifdef _LP64
typedef long int		 int_fast64_t;
typedef unsigned long int	uint_fast64_t;
#else
typedef long long int		 int_fast64_t;
typedef unsigned long long int	uint_fast64_t;
#endif

/* 7.18.1.5 Greatest-width integer types */
typedef long long int		 intmax_t;
typedef unsigned long long int	uintmax_t;

#endif /* !__UINT_FAST64_TYPE__ */

#endif /* !_MACHINE_INT_MWGWTYPES_H_ */
