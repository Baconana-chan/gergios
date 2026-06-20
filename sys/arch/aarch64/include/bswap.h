/*	AArch64 bswap.h — compiler builtins for byte swap	*/

#ifndef _AARCH64_BSWAP_H_
#define _AARCH64_BSWAP_H_

#include <sys/cdefs.h>

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

#endif /* !_AARCH64_BSWAP_H_ */
