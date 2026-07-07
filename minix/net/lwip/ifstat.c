/* LWIP service - ifstat.c - per-interface statistics */
/*
 * This module exposes per-interface packet/byte/error counters via the
 * minix.lwip.ifaces sysctl MIB node.  A single RMIB_FUNC handler returns
 * an array of struct if_stat structures, one per active interface.
 *
 * Usage:
 *   sysctl minix.lwip.ifaces  — returns all interface stats as binary data
 *
 * The struct if_stat format (for parsing with a C program):
 *   char     name[IFNAMSIZ]  — interface name
 *   uint64_t type            — interface type (IFT_*)
 *   uint64_t mtu             — MTU
 *   uint64_t link_state      — link state (LINK_STATE_*)
 *   uint64_t ipackets        — input packets
 *   uint64_t ierrors         — input errors
 *   uint64_t opackets        — output packets
 *   uint64_t oerrors         — output errors
 *   uint64_t ibytes          — input bytes
 *   uint64_t obytes          — output bytes
 *   uint64_t imcasts         — input multicast packets
 *   uint64_t omcasts         — output multicast packets
 *   uint64_t iqdrops         — input queue drops
 *   uint64_t collisions      — collisions
 */

#include "lwip.h"

/*
 * Per-interface statistics structure exposed via sysctl.
 * Fixed-size for easy array copyout.
 */
struct if_stat {
	char		ifs_name[IFNAMSIZ];
	uint64_t	ifs_type;
	uint64_t	ifs_mtu;
	uint64_t	ifs_link_state;
	uint64_t	ifs_ipackets;
	uint64_t	ifs_ierrors;
	uint64_t	ifs_opackets;
	uint64_t	ifs_oerrors;
	uint64_t	ifs_ibytes;
	uint64_t	ifs_obytes;
	uint64_t	ifs_imcasts;
	uint64_t	ifs_omcasts;
	uint64_t	ifs_iqdrops;
	uint64_t	ifs_collisions;
};

/*
 * Maximum number of interfaces we can report.  Must be at least as large
 * as the actual interface count.  UINT8_MAX is safe since lwIP uses u8_t
 * for the interface index.
 */
/* Max interfaces: NR_NDEV (8) + NR_LOOPIF (2) + safety margin. */
#define IFSTAT_MAX_IFACES	32

/*
 * Snapshot all active interfaces into the given array.
 * Returns the number of interfaces captured.
 */
static unsigned int
ifstat_snapshot(struct if_stat * entries, unsigned int max)
{
	struct ifdev *ifdev;
	struct if_data *ifdata;
	unsigned int count;

	count = 0;

	for (ifdev = ifdev_enum(NULL); ifdev != NULL && count < max;
	    ifdev = ifdev_enum(ifdev)) {
		struct if_stat *ent = &entries[count];

		memset(ent, 0, sizeof(*ent));

		strlcpy(ent->ifs_name, ifdev_get_name(ifdev),
		    sizeof(ent->ifs_name));
		ifdata = ifdev_get_ifdata(ifdev);

		ent->ifs_type = ifdev_get_iftype(ifdev);
		ent->ifs_mtu = ifdev_get_mtu(ifdev);
		ent->ifs_link_state = ifdev_get_link(ifdev);
		ent->ifs_ipackets = ifdata->ifi_ipackets;
		ent->ifs_ierrors = ifdata->ifi_ierrors;
		ent->ifs_opackets = ifdata->ifi_opackets;
		ent->ifs_oerrors = ifdata->ifi_oerrors;
		ent->ifs_ibytes = ifdata->ifi_ibytes;
		ent->ifs_obytes = ifdata->ifi_obytes;
		ent->ifs_imcasts = ifdata->ifi_imcasts;
		ent->ifs_omcasts = ifdata->ifi_omcasts;
		ent->ifs_iqdrops = ifdata->ifi_iqdrops;
		ent->ifs_collisions = ifdata->ifi_collisions;

		count++;
	}

	return count;
}

/*
 * Sysctl handler for minix.lwip.ifaces.
 * When oldp is NULL, returns the total size for all interfaces.
 * When oldp is non-NULL, copies out the array of if_stat structures.
 */
static ssize_t
ifstat_handler(struct rmib_call * call __unused,
	struct rmib_node * node __unused, struct rmib_oldp * oldp,
	struct rmib_newp * newp __unused)
{
	struct if_stat entries[IFSTAT_MAX_IFACES];
	unsigned int count;
	ssize_t off;
	int r;

	count = ifstat_snapshot(entries, IFSTAT_MAX_IFACES);

	if (oldp == NULL) {
		/* Return total size needed. */
		return (ssize_t)count * (ssize_t)sizeof(struct if_stat);
	}

	off = 0;

	if (count > 0) {
		if ((r = rmib_copyout(oldp, 0, entries,
		    (size_t)count * sizeof(struct if_stat))) < 0)
			return r;
		off = (ssize_t)count * (ssize_t)sizeof(struct if_stat);
	}

	return off;
}

/* Root of the minix.lwip.ifaces tree — single FUNC node. */
static struct rmib_node minix_lwip_ifaces_node =
    RMIB_FUNC(RMIB_RO, sizeof(struct if_stat) * IFSTAT_MAX_IFACES,
	ifstat_handler, "ifaces", "Per-interface statistics");

/*
 * Initialize the interface statistics module.
 */
void
ifstat_init(void)
{

	mibtree_register_lwip(&minix_lwip_ifaces_node);
}
