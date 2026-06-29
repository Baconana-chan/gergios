/*	machine/int_types.h -> arch/${MACHINE_ARCH}/include/int_types.h	*/
#ifndef _MACHINE_INT_TYPES_H_
#define _MACHINE_INT_TYPES_H_

#ifndef _MSC_VER
#include_next <int_types.h>
#else
/* MSVC fallback for host tools */
#include <stdint.h>
typedef signed char		__int8_t;
typedef unsigned char		__uint8_t;
typedef short int		__int16_t;
typedef unsigned short int	__uint16_t;
typedef int			__int32_t;
typedef unsigned int		__uint32_t;
typedef long long		__int64_t;
typedef unsigned long long	__uint64_t;
typedef long long		__intptr_t;
typedef unsigned long long	__uintptr_t;
#endif

#endif /* !_MACHINE_INT_TYPES_H_ */
