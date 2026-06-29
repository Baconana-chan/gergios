/* ============================================================
 * arch_clock.h — x86_64 architecture clock interface
 *
 * Declares the architecture-specific timer interrupt handler
 * called from the generic kernel timer code (clock.c).
 *
 * On x86_64, the timer subsystem handles i8253 (PIT), APIC
 * timer, and TSC, depending on the detected hardware.
 * ============================================================ */

#ifndef _X86_64_ARCH_CLOCK_H_
#define _X86_64_ARCH_CLOCK_H_

/* Called from timer_int_handler() in clock.c after generic
 * timer tick processing. Arch-specific hook for any additional
 * timer maintenance. On x86_64, typically a no-op when APIC
 * is used, but can be used for i8253-specific handling. */
void arch_timer_int_handler(void);

#endif /* _X86_64_ARCH_CLOCK_H_ */
