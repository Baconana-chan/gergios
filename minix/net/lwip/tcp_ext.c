/* LWIP service - tcp_ext.c - TCP extended per-connection metrics */
/*
 * This module exposes extended TCP per-connection metrics via the
 * minix.lwip.tcp_ext sysctl MIB tree.  Metrics include:
 *   - snd_cwnd: sender congestion window (segments)
 *   - snd_wnd: sender window (bytes)
 *   - rto: retransmission timeout (ms)
 *   - rtt: smoothed round-trip time (ms)
 *   - rttvar: round-trip time variance (ms)
 *   - nrtx: number of retransmissions
 *   - mss: maximum segment size
 *   - state: TCP state (TCPS_*)
 *
 * Usage:
 *   sysctl minix.lwip.tcp_ext          — list all entries
 *   sysctl minix.lwip.tcp_ext.0        — first connection metrics
 *   sysctl minix.lwip.tcp_ext.0.cwnd   — congestion window
 *   ... (32 entries indexed by connection)
 *
 * Currently supports up to 32 concurrent TCP connections for reporting.
 * Only ESTABLISHED and higher-state connections are listed.
 */

#include "lwip.h"

#include <netinet/tcp.h>
#include <netinet/tcp_var.h>
#include <netinet/tcp_fsm.h>

#include "lwip/tcp.h"
#include "lwip/priv/tcp_priv.h"

#define TCP_EXT_ARRAY_COUNT(x) (sizeof(x) / sizeof((x)[0]))

#define TCP_EXT_MAX_ENTRIES	32

/*
 * A snapshot of TCP per-connection metrics.
 */
struct tcp_ext_entry {
	uint64_t tee_sockaddr;		/* socket address */
	uint32_t tee_state;		/* TCP state (TCPS_*) */
	uint32_t tee_snd_cwnd;		/* congestion window (segments) */
	uint32_t tee_snd_wnd;		/* sender window (bytes) */
	uint32_t tee_rcv_wnd;		/* receiver window (bytes) */
	uint32_t tee_rto;		/* retransmission timeout (ms) */
	uint32_t tee_rtt;		/* smoothed RTT (ms) */
	uint32_t tee_rttvar;		/* RTT variance (ms) */
	uint32_t tee_nrtx;		/* number of retransmissions */
	uint32_t tee_mss;		/* maximum segment size */
	uint32_t tee_snd_buf;		/* send buffer size */
	uint32_t tee_unsent;		/* unsent bytes */
	uint32_t tee_unacked;		/* unacknowledged bytes */
	uint8_t  tee_local_ip[16];	/* local IP address (IPv4 or IPv6) */
	uint8_t  tee_remote_ip[16];	/* remote IP address */
	uint16_t tee_local_port;	/* local port */
	uint16_t tee_remote_port;	/* remote port */
	uint8_t  tee_is_v6;		/* IPv6 flag */
};

/*
 * TCP state mapping from lwIP to NetBSD TCPS_* constants.
 */
static const int tcp_ext_statemap[] = {
	[CLOSED]	= TCPS_CLOSED,
	[LISTEN]	= TCPS_LISTEN,
	[SYN_SENT]	= TCPS_SYN_SENT,
	[SYN_RCVD]	= TCPS_SYN_RECEIVED,
	[ESTABLISHED]	= TCPS_ESTABLISHED,
	[FIN_WAIT_1]	= TCPS_FIN_WAIT_1,
	[FIN_WAIT_2]	= TCPS_FIN_WAIT_2,
	[CLOSE_WAIT]	= TCPS_CLOSE_WAIT,
	[CLOSING]	= TCPS_CLOSING,
	[LAST_ACK]	= TCPS_LAST_ACK,
	[TIME_WAIT]	= TCPS_TIME_WAIT,
};

/*
 * Snapshot current TCP connections into an array of tcp_ext_entry structs.
 */
static unsigned int
tcp_ext_snapshot(struct tcp_ext_entry * entries, unsigned int max)
{
	const struct tcp_pcb_lists *list;
	const struct tcp_pcb *pcb;
	unsigned int i, count;

	count = 0;

	/* Iterate over ALL lwIP TCP PCB lists (active, TIME-WAIT, etc). */
	for (i = 0; i < TCP_EXT_ARRAY_COUNT(tcp_pcb_lists) && count < max; i++) {
		list = tcp_pcb_lists[i];

		if (list == NULL)
			continue;

		for (pcb = *list; pcb != NULL && count < max; pcb = pcb->next) {
			struct tcp_ext_entry *ent = &entries[count];

			memset(ent, 0, sizeof(*ent));

			/* Skip closed sockets */
			if (pcb->state == CLOSED)
				continue;

			/* Store state */
			if ((unsigned int)pcb->state <
			    TCP_EXT_ARRAY_COUNT(tcp_ext_statemap))
				ent->tee_state = (uint32_t)tcp_ext_statemap[pcb->state];
			else
				ent->tee_state = TCPS_CLOSED;

			/* Store IP addresses */
			if (IP_IS_V6(&pcb->local_ip)) {
				ent->tee_is_v6 = 1;
				memcpy(ent->tee_local_ip,
				    ip_2_ip6(&pcb->local_ip)->addr, 16);
				memcpy(ent->tee_remote_ip,
				    ip_2_ip6(&pcb->remote_ip)->addr, 16);
			} else {
				uint32_t addr4;

				ent->tee_is_v6 = 0;
				addr4 = ip_addr_get_ip4_u32(&pcb->local_ip);
				memcpy(ent->tee_local_ip, &addr4, 4);
				addr4 = ip_addr_get_ip4_u32(&pcb->remote_ip);
				memcpy(ent->tee_remote_ip, &addr4, 4);
			}

			/* Store ports */
			ent->tee_local_port = pcb->local_port;
			ent->tee_remote_port = pcb->remote_port;

			/*
			 * The following fields are only available in the
			 * full TCP PCB (not the smaller LISTEN PCB).
			 */
			if (pcb->state != LISTEN) {
				ent->tee_snd_cwnd = pcb->cwnd;
				ent->tee_snd_wnd = pcb->snd_wnd;
				ent->tee_rcv_wnd = pcb->rcv_wnd;
				ent->tee_rto = pcb->rto;
				ent->tee_nrtx = pcb->nrtx;
				ent->tee_mss = pcb->mss;

				/* Send buffer space available */
				ent->tee_snd_buf = tcp_sndbuf(pcb);

				/* Track unsent/unacked byte counts */
				ent->tee_unsent = (pcb->unsent != NULL) ?
				    pcb->unsent->tot_len : 0;
				ent->tee_unacked = (pcb->unacked != NULL) ?
				    pcb->unacked->tot_len : 0;

				/*
				 * RTT estimation (optional).
				 * lwIP's smoothed RTT (sa, units of 500ms)
				 * and RTT variance (sv, units of 250ms).
				 * May be zero if not yet measured.
				 */
				if (pcb->sa != 0) {
					/* sa in 500ms units, convert to ms */
					ent->tee_rtt =
					    ((uint32_t)pcb->sa * 500) >> 3;
				}
				if (pcb->sv != 0) {
					/* sv in 250ms units, convert to ms */
					ent->tee_rttvar =
					    ((uint32_t)pcb->sv * 250) >> 2;
				}
			}

			/* Store socket address from callback arg */
			if (pcb->callback_arg != NULL &&
			    pcb->state != SYN_RCVD) {
				ent->tee_sockaddr =
				    (uint64_t)(uintptr_t)pcb->callback_arg;
			}

			count++;
		}
	}

	return count;
}

/*
 * Handler for the tcp_ext sysctl node.
 * When oldp is NULL, returns the size of one entry times the number of
 * entries.  When oldp is non-NULL, copies out the entry array.
 */
static ssize_t
tcp_ext_handler(struct rmib_call * call __unused,
	struct rmib_node * node __unused, struct rmib_oldp * oldp,
	struct rmib_newp * newp __unused)
{
	struct tcp_ext_entry entries[TCP_EXT_MAX_ENTRIES];
	unsigned int count;
	ssize_t off;
	int r;

	count = tcp_ext_snapshot(entries, TCP_EXT_MAX_ENTRIES);
	off = 0;

	if (oldp != NULL) {
		unsigned int i;

		for (i = 0; i < count; i++) {
			if ((r = rmib_copyout(oldp, off,
			    &entries[i], sizeof(entries[i]))) < 0)
				return r;
			off += sizeof(entries[i]);
		}
	} else {
		off = (ssize_t)count * (ssize_t)sizeof(struct tcp_ext_entry);
	}

	return off;
}

/* The minix.lwip.tcp_ext RMIB node. */
static struct rmib_node minix_lwip_tcp_ext_table[] = {
	RMIB_FUNC(RMIB_RO, sizeof(struct tcp_ext_entry),
	    tcp_ext_handler, "tcp_ext",
	    "Extended TCP per-connection metrics"),
};

static struct rmib_node minix_lwip_tcp_ext_node =
    RMIB_NODE(RMIB_RO, minix_lwip_tcp_ext_table, "tcp_ext",
	"Extended TCP connection metrics (cwnd, rtt, rto, nrtx)");

/*
 * Initialize the TCP extended info module.
 */
void
tcp_ext_init(void)
{

	mibtree_register_lwip(&minix_lwip_tcp_ext_node);
}
