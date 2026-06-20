/* ============================================================
 * arch_clock.h — ARM64 architecture clock interface
 *
 * Declares the architecture-specific timer interrupt handler
 * called from the generic kernel timer code (clock.c).
 *
 * Following the same pattern as arch/earm/include/arch_clock.h.
 * ============================================================ */

#ifndef _AARCH64_ARCH_CLOCK_H_
#define _AARCH64_ARCH_CLOCK_H_

/* Called from timer_int_handler() in clock.c after generic
 * timer tick processing. Arch-specific hook for any additional
 * timer maintenance. Typically a no-op on ARM64. */
void arch_timer_int_handler(void);

#endif /* _AARCH64_ARCH_CLOCK_H_ */
