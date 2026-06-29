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

/* ===========================================================================
 * Bare-metal fallback: Clang with --target=x86_64-elf or --target=aarch64-elf
 * predefines __UINT_FAST64_TYPE__ as part of its built-in <stdint.h>, which
 * causes the #ifndef block above to be skipped. However, Clang's built-in
 * stdint.h does NOT get included through MINIX's header chain (which goes
 * through include/inttypes.h → sys/sys/inttypes.h → sys/sys/stdint.h),
 * so intmax_t/uintmax_t are never actually defined.
 *
 * When the built-in stdint.h IS included (e.g., via #include <stdint.h>
 * from MINIX code), Clang defines intmax_t as 'long' on LP64 (x86_64,
 * aarch64). When it's NOT included, we need to provide the typdef here.
 * There's no reliable preprocessor check to detect whether intmax_t was
 * already typedef'd. The safest approach: use 'long' on LP64 to match
 * Clang's choice, which makes repeated typedefs harmless (C11 allows
 * identical repeated typedefs). On non-LP64, use 'long long'.
 * ========================================================================= */
#ifndef intmax_t
/* Note: #ifndef only checks MACRO namespace. But Clang predefines intmax_t
 * as a typedef, not a macro, so this check always passes. The typedef below
 * will either be the first definition (no Clang built-in) or a repeat of
 * the same type (Clang built-in included first). In both cases, using
 * 'long' on LP64 matches Clang's choice, avoiding redefinition errors. */
#ifdef __LP64__
typedef long int intmax_t;
typedef unsigned long int uintmax_t;
#else
typedef long long int intmax_t;
typedef unsigned long long int uintmax_t;
#endif
#endif

#endif /* !_MACHINE_INT_MWGWTYPES_H_ */
