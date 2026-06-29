/* x86_64 NMI watchdog stubs.
 *
 * The NMI watchdog is not yet implemented for x86_64.
 * These stubs satisfy the linker for kernel/watchdog.c.
 * The functions return "not available" so the watchdog
 * gracefully disables itself at runtime.
 */

#include "kernel/kernel.h"
#include "arch_watchdog.h"

int arch_watchdog_init(void)
{
	/* No watchdog available — return 0 to indicate no hardware found. */
	return 0;
}

void arch_watchdog_stop(void)
{
	/* Nothing to stop. */
}

void arch_watchdog_lockup(const struct nmi_frame * frame)
{
	/* Should never be called (watchdog not initialized). */
}
