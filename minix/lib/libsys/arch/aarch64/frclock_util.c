/* Utility functions around the ARM Generic Timer (free-running counter).
 *
 * On AArch64, the free-running clock is the Physical Count Register
 * (CNTPCT_EL0), which is a 64-bit monotonic counter driven by the
 * system counter. The counter frequency is reported by CNTFRQ_EL0.
 *
 * Unlike ARM32 (earm), AArch64 has a true 64-bit counter, so no
 * wrapping logic is needed for the 64-bit variants.
 *
 * Reference: ARM DDI 0487, Chapter D10 — System counter and timer
 */

#include <minix/minlib.h>
#include <minix/sysutil.h>
#include <sys/errno.h>
#include <sys/types.h>
#include <lib.h>
#include <assert.h>

#define MICROHZ         1000000ULL	/* number of micros per second */
#define MICROSPERTICK(h)	(MICROHZ/(h)) /* number of micros per HZ tick */

/*
 * Read the ARM Generic Timer counter frequency (CNTFRQ_EL0).
 * This is the frequency of the system counter, which determines
 * the number of ticks per second.
 */
static inline u64_t
read_cntfrq(void)
{
	u64_t val;
	__asm__ __volatile__("mrs %0, CNTFRQ_EL0" : "=r"(val));
	return val;
}

/*
 * Read the ARM Generic Timer physical count (CNTPCT_EL0).
 * This is a 64-bit counter that increments at the frequency
 * reported by CNTFRQ_EL0.
 */
static inline u64_t
read_cntpct(void)
{
	u64_t val;
	__asm__ __volatile__("mrs %0, CNTPCT_EL0" : "=r"(val));
	return val;
}

/*
 * micro_delay — Busy-wait for the given number of microseconds.
 *
 * Uses a combination of tickdelay (voluntary sleep) and busy-waiting
 * on CNTPCT_EL0 for the remainder.
 */
int
micro_delay(u32_t micros)
{
	u64_t start, delta, delta_end;
	u64_t freq_hz;

	freq_hz = read_cntfrq();
	if (freq_hz < MICROHZ)
		freq_hz = MICROHZ;
	start = read_cntpct();
	delta_end = (freq_hz * micros) / MICROHZ;

	/* If we have to wait for at least one HZ tick, use the regular
	 * tickdelay first. Round downwards to compensate for overhead.
	 */
	if (micros >= MICROSPERTICK(sys_hz()))
		tickdelay(micros * sys_hz() / MICROHZ);

	/* Busy-wait for the (remaining) delay. */
	do {
		delta = read_cntpct();
	} while ((delta - start) < delta_end);

	return 0;
}

/*
 * read_frclock — Read the low 32 bits of the free-running counter.
 *
 * Provided for API compatibility with ARM32 (earm). New code should
 * use read_frclock_64() instead.
 */
void
read_frclock(u32_t *frclk)
{
	u64_t val;

	assert(frclk);
	val = read_cntpct();
	*frclk = (u32_t)(val & 0xFFFFFFFFULL);
}

/*
 * delta_frclock — Compute difference between two 32-bit counter values.
 *
 * Handles 32-bit wrap-around (once). Provided for API compatibility.
 * New code should use delta_frclock_64() instead.
 */
u32_t
delta_frclock(u32_t base, u32_t cur)
{
	u32_t delta;

	if (cur < base)
		delta = (UINT_MAX - base) + cur;
	else
		delta = cur - base;

	return delta;
}

/*
 * read_frclock_64 — Read the full 64-bit free-running counter.
 *
 * On AArch64, this reads CNTPCT_EL0 directly. The counter is
 * guaranteed to be monotonic and 64-bit.
 */
void
read_frclock_64(u64_t *frclk)
{
	assert(frclk);
	*frclk = read_cntpct();
}

/*
 * delta_frclock_64 — Compute difference between two 64-bit counter values.
 *
 * No wrap-around needed for 64-bit counters in practice.
 */
u64_t
delta_frclock_64(u64_t base, u64_t cur)
{
	return cur - base;
}

/*
 * frclock_64_to_micros — Convert free-running clock ticks to microseconds.
 *
 * Uses CNTFRQ_EL0 to get the counter frequency.
 */
u32_t
frclock_64_to_micros(u64_t tsc)
{
	u64_t freq_hz;

	freq_hz = read_cntfrq();
	if (freq_hz < MICROHZ)
		freq_hz = MICROHZ;
	return (u32_t)(tsc / (freq_hz / MICROHZ));
}
