/*	$NetBSD$	*/
/* AArch64 watchdog stub — no hardware watchdog on QEMU virt. */

#ifndef _AARCH64_ARCH_WATCHDOG_H_
#define _AARCH64_ARCH_WATCHDOG_H_

/* NMI watchdog not supported on ARM64. */
static inline int nmi_watchdog_start_profiling(unsigned freq)
{
	(void)freq;
	return -1; /* not supported */
}

static inline void nmi_watchdog_stop_profiling(void)
{
	/* no-op */
}

#endif /* _AARCH64_ARCH_WATCHDOG_H_ */
