#include <sys/types.h>
#include <minix/minlib.h>

void
read_tsc(u32_t *hi, u32_t *lo)
{
	/*
	 * Read the ARM Generic Timer physical count register (CNTPCT_EL0).
	 * This is a 64-bit counter available at EL0 on all AArch64 systems.
	 * Returns the low and high 32-bit parts.
	 */
	u64_t cntpct;

	__asm __volatile("mrs %0, CNTPCT_EL0" : "=r" (cntpct));

	*hi = (u32_t)(cntpct >> 32);
	*lo = (u32_t)(cntpct & 0xFFFFFFFF);
}
