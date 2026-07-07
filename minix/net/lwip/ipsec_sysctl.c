/* LWIP service - ipsec_sysctl.c - IPsec sysctl interface */
/*
 * Provides runtime enable/disable toggle and statistics readout for the
 * minimal IPsec ESP Transport + AH implementation (RFC 4301/4302/4303).
 *
 * Tree: minix.lwip.ipsec
 *   [0] enabled  - RW int: 1 = IPsec processing enabled, 0 = disabled
 *   [1] stats    - RO func: returns struct ipsec_stats as binary blob
 */
#include "lwip.h"

#if LWIP_IPSEC

#include "lwip_ipsec.h"

#include <string.h>

/*
 * Handler for the minix.lwip.ipsec.stats node.
 * On read, copies the current IPsec statistics to the caller.
 */
static ssize_t
ipsec_sysctl_stats(struct rmib_call *call __unused,
    struct rmib_node *node __unused,
    struct rmib_oldp *oldp, struct rmib_newp *newp __unused)
{

	if (oldp == NULL)
		return sizeof(struct ipsec_stats);

	return rmib_copyout(oldp, 0, &lwip_ipsec_stats,
	    sizeof(struct ipsec_stats));
}

/* The minix.lwip.ipsec RMIB subtree. */
static struct rmib_node minix_lwip_ipsec_table[] = {
	[0] = RMIB_INTPTR(RMIB_RW, &lwip_ipsec_enabled,
	    "enabled",
	    "Enable IPsec ESP/AH packet processing"),
	[1] = RMIB_FUNC(RMIB_RO, sizeof(struct ipsec_stats),
	    ipsec_sysctl_stats, "stats",
	    "IPsec statistics (struct ipsec_stats)"),
};

static struct rmib_node minix_lwip_ipsec_node =
    RMIB_NODE(RMIB_RO, minix_lwip_ipsec_table, "ipsec",
    "IPsec ESP/AH settings (RFC 4301/4302/4303)");

/* ------------------------------------------------------------------ */
/*  Initialisation                                                     */
/* ------------------------------------------------------------------ */

void
ipsec_sysctl_init(void)
{

	mibtree_register_lwip(&minix_lwip_ipsec_node);
}

#else /* !LWIP_IPSEC */

void
ipsec_sysctl_init(void)
{
}

#endif /* LWIP_IPSEC */
