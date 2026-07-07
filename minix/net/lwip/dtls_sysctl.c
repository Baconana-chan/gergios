/* LWIP service - dtls_sysctl.c - DTLS sysctl interface */
/*
 * Provides runtime enable/disable toggle and statistics readout for the
 * DTLS (Datagram TLS) over UDP implementation (RFC 6347/9147).
 *
 * Tree: minix.lwip.dtls
 *   [0] enabled  - RW int: 1 = DTLS processing enabled, 0 = disabled
 *   [1] stats    - RO func: returns struct lwip_dtls_stats as binary blob
 */
#include "lwip.h"

#if LWIP_DTLS

#include "lwip_dtls.h"

#include <string.h>

/*
 * Handler for the minix.lwip.dtls.stats node.
 * On read, copies the current DTLS statistics to the caller.
 */
static ssize_t
dtls_sysctl_stats(struct rmib_call *call __unused,
    struct rmib_node *node __unused,
    struct rmib_oldp *oldp, struct rmib_newp *newp __unused)
{

	if (oldp == NULL)
		return sizeof(struct lwip_dtls_stats);

	return rmib_copyout(oldp, 0, &lwip_dtls_stats,
	    sizeof(struct lwip_dtls_stats));
}

/* The minix.lwip.dtls RMIB subtree. */
static struct rmib_node minix_lwip_dtls_table[] = {
	[0] = RMIB_INTPTR(RMIB_RW, &lwip_dtls_enabled,
	    "enabled",
	    "Enable DTLS over UDP handshake and crypto processing"),
	[1] = RMIB_FUNC(RMIB_RO, sizeof(struct lwip_dtls_stats),
	    dtls_sysctl_stats, "stats",
	    "DTLS statistics (struct lwip_dtls_stats)"),
};

static struct rmib_node minix_lwip_dtls_node =
    RMIB_NODE(RMIB_RO, minix_lwip_dtls_table, "dtls",
    "DTLS settings (RFC 6347/9147)");

/* ------------------------------------------------------------------ */
/*  Initialisation                                                     */
/* ------------------------------------------------------------------ */

void
dtls_sysctl_init(void)
{

	mibtree_register_lwip(&minix_lwip_dtls_node);
}

#else /* !LWIP_DTLS */

void
dtls_sysctl_init(void)
{
}

#endif /* LWIP_DTLS */
