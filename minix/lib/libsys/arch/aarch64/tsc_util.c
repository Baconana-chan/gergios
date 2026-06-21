/* Timestamp counter utility functions for AArch64.
 *
 * On AArch64, the timestamp counter is the Physical Count Register
 * (CNTPCT_EL0) of the ARM Generic Timer. It is a 64-bit monotonic
 * counter driven by the system counter at the frequency reported
 * by CNTFRQ_EL0.
 *
 * Unlike x86's TSC, CNTPCT_EL0 is:
 *   - Always available (no CPUID check needed)
 *   - Always monotonic (no frequency scaling)
 *   - Synchronized across CPUs (in SMP systems)
 *   - Accessible from EL0 (user mode)
 *
 * Reference: ARM DDI 0487, Chapter D10
 */

#include <stdio.h>
#include <time.h>
#include <sys/times.h>
#include <sys/types.h>
#include <minix/u64.h>
#include <minix/config.h>
#include <minix/const.h>
#include <minix/minlib.h>
#include <machine/archtypes.h>

#ifndef CONFIG_MAX_CPUS
#define CONFIG_MAX_CPUS 1
#endif

#define MICROHZ		1000000		/* number of micros per second */
#define MICROSPERTICK(h)	(MICROHZ/(h))	/* number of micros per HZ tick */

/*
 * Read the ARM Generic Timer counter frequency (CNTFRQ_EL0).
 * This is used to convert counter ticks to microseconds.
 */
static inline u64_t
read_cntfrq(void)
{
	u64_t val;
	__asm__ __volatile__("mrs %0, CNTFRQ_EL0" : "=r"(val));
	return val;
}

/*
 * tsc_64_to_micros — Convert 64-bit timestamp counter to microseconds.
 *
 * Uses the actual CNTFRQ_EL0 frequency rather than a hardcoded calibration.
 * This ensures correct results across different ARM64 platforms.
 */
u32_t
tsc_64_to_micros(u64_t tsc)
{
	u64_t freq_hz;

	freq_hz = read_cntfrq();
	if (freq_hz < MICROHZ)
		freq_hz = MICROHZ;
	return (u32_t)(tsc / (freq_hz / MICROHZ));
}

/*
 * tsc_to_micros — Convert 32-bit pair (low, high) to microseconds.
 *
 * Provided for API compatibility. New code should use tsc_64_to_micros()
 * with a single 64-bit value.
 */
u32_t
tsc_to_micros(u32_t low, u32_t high)
{
	return tsc_64_to_micros(make64(low, high));
}
