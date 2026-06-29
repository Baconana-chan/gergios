/*	$NetBSD$	*/
/* AArch64 bswap.h — compiler builtins for byte swap. */

#ifndef _MACHINE_BSWAP_H_
#define _MACHINE_BSWAP_H_

#ifdef _KERNEL
#include <sys/types.h>
#else
#include <sys/types.h>
#endif

static __inline uint16_t
bswap16(uint16_t x)
{
	return __builtin_bswap16(x);
}

static __inline uint32_t
bswap32(uint32_t x)
{
	return __builtin_bswap32(x);
}

static __inline uint64_t
bswap64(uint64_t x)
{
	return __builtin_bswap64(x);
}

/* NOTE: When bswap64.c includes this header and then defines its own
 * non-static bswap64, the static inline below is harmless in C99+:
 * the static inline has internal linkage, so it doesn't conflict with
 * the external function definition from bswap64.c at link time.
 * However, some compilers may warn about duplicate definitions in the
 * same translation unit. On bare-metal, bswap64.c is redundant since
 * __builtin_bswap64 handles everything — if you get a redefinition
 * error, exclude bswap64.c from the build or add -D__BSWAP64_INLINE
 * before including this header to guard the inline definition below.
 */

#endif /* !_MACHINE_BSWAP_H_ */
