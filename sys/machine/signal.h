/*	machine/signal.h -> arch/${MACHINE_ARCH}/include/signal.h	*/
#ifndef _MACHINE_SIGNAL_H_
#define _MACHINE_SIGNAL_H_

#ifndef _MSC_VER
/* Use __has_include_next to safely detect whether there's a next
 * <signal.h> to include. On bare-metal targets (--target=x86_64-elf,
 * --target=aarch64-elf), there is no system <signal.h>, so the
 * #include_next would fail. __has_include_next is supported by
 * Clang and GCC 5+. */
#ifdef __has_include_next
#  if __has_include_next(<signal.h>)
#    include_next <signal.h>
#  endif
#else
/* Fallback for compilers without __has_include_next:
 * skip the include_next on bare-metal targets. */
#if !defined(__UINT_FAST64_TYPE__)
#include_next <signal.h>
#endif
#endif
#endif

#endif /* !_MACHINE_SIGNAL_H_ */
