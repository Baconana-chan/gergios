/* Kernel MAC (Mandatory Access Control) hooks.
 *
 * This file implements the kernel-side MAC hook infrastructure.
 * It provides a simple hook chain mechanism for kernel-internal
 * MAC checks at IPC send, privilege control, and other points.
 *
 * The hook chain is initially empty (all operations allowed).
 * A hook function registered via mac_register_kernel_hook() is
 * called for each securable operation. All hooks in the chain
 * must return MAC_ALLOW for the operation to proceed.
 */

#include "kernel/system.h"
#include <minix/mac.h>

/* The kernel MAC hook chain — a simple array of function pointers.
 * Currently supports up to 4 concurrent hooks; easily extended. */
#define MAX_KERNEL_MAC_HOOKS 4

static mac_hook_fn_t mac_hooks[MAX_KERNEL_MAC_HOOKS];
static int num_mac_hooks;

/*===========================================================================*
 *				mac_register_kernel_hook			     *
 *===========================================================================*/
void mac_register_kernel_hook(mac_hook_fn_t hook)
{
	int i;

	if (hook == NULL) {
		/* Clear the entire chain. */
		num_mac_hooks = 0;
		return;
	}

	if (num_mac_hooks >= MAX_KERNEL_MAC_HOOKS) {
		printf("MAC: hook chain full (%d), cannot register more\n",
		    MAX_KERNEL_MAC_HOOKS);
		return;
	}

	/* Check if already registered. */
	for (i = 0; i < num_mac_hooks; i++) {
		if (mac_hooks[i] == hook)
			return;
	}

	mac_hooks[num_mac_hooks++] = hook;
	printf("MAC: registered kernel hook %d (total %d)\n",
	    num_mac_hooks, num_mac_hooks);
}

/*===========================================================================*
 *				mac_kernel_check				     *
 *===========================================================================*/
int mac_kernel_check(int what, mac_context_t *ctx)
{
	int i, r;

	/* If no hooks registered, allow by default. */
	if (num_mac_hooks == 0)
		return MAC_ALLOW;

	/* Call each hook in the chain. All must allow. */
	for (i = 0; i < num_mac_hooks; i++) {
		r = mac_hooks[i](what, ctx);
		if (r != MAC_ALLOW) {
			return MAC_DENY;
		}
	}

	return MAC_ALLOW;
}

/*===========================================================================*
 *				mac_hook_init					     *
 *===========================================================================*/
void mac_hook_init(void)
{
	num_mac_hooks = 0;
	/* Hook chain is empty → all operations allowed.
	 * A MAC module can register its hook later during boot. */
}
