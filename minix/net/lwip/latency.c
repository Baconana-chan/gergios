/* LWIP service - latency.c - network latency histogram */
/*
 * This module implements latency histogram tracking for network operations.
 * Latencies are binned into configurable buckets (in microseconds):
 *   0-100us, 100-500us, 500us-1ms, 1-5ms, 5-20ms, 20-100ms, >100ms
 *
 * Per-protocol statistics are exposed via minix.lwip.latency sysctl.
 *
 * To record a latency sample from elsewhere in the lwIP service:
 *   uint32_t start = sys_now() * 1000;  // or use getticks()
 *   ... do operation ...
 *   uint32_t end = sys_now() * 1000;
 *   latency_record(&latency_tcp_connect, end - start);
 */

#include "lwip.h"
#include "latency.h"

#define LATENCY_ARRAY_COUNT(x) (sizeof(x) / sizeof((x)[0]))

/* Bucket boundaries in microseconds. */
const uint32_t latency_buckets[LATENCY_NUM_BUCKETS] = {
	100,		/* 0-100 us */
	500,		/* 100-500 us */
	1000,		/* 500 us - 1 ms */
	5000,		/* 1-5 ms */
	20000,		/* 5-20 ms */
	100000,		/* 20-100 ms */
	/* >100ms falls into the last bucket */
};

/* Global statistics counters. */
struct latency_stats latency_udp_send;
struct latency_stats latency_tcp_connect;
struct latency_stats latency_tcp_send;

/*
 * Record a latency sample into the given statistics structure.
 */
void
latency_record(struct latency_stats * stats, uint32_t duration_us)
{
	unsigned int i;

	stats->ls_count++;

	/* Find the appropriate bucket. */
	for (i = 0; i < LATENCY_NUM_BUCKETS - 1; i++) {
		if (duration_us <= latency_buckets[i]) {
			stats->ls_buckets[i]++;
			return;
		}
	}

	/* Last bucket catches everything above the last boundary. */
	stats->ls_buckets[LATENCY_NUM_BUCKETS - 1]++;
}

/*
 * Sysctl handler for UDP send latency (child node 0).
 */
static ssize_t
latency_udp_handler(struct rmib_call * call __unused,
	struct rmib_node * node __unused, struct rmib_oldp * oldp,
	struct rmib_newp * newp __unused)
{
	int r;

	if (oldp == NULL)
		return sizeof(struct latency_stats);

	if ((r = rmib_copyout(oldp, 0, &latency_udp_send,
	    sizeof(struct latency_stats))) < 0)
		return r;
	return sizeof(struct latency_stats);
}

/*
 * Sysctl handler for TCP connect latency (child node 1).
 */
static ssize_t
latency_tcp_connect_handler(struct rmib_call * call __unused,
	struct rmib_node * node __unused, struct rmib_oldp * oldp,
	struct rmib_newp * newp __unused)
{
	int r;

	if (oldp == NULL)
		return sizeof(struct latency_stats);

	if ((r = rmib_copyout(oldp, 0, &latency_tcp_connect,
	    sizeof(struct latency_stats))) < 0)
		return r;
	return sizeof(struct latency_stats);
}

/*
 * Sysctl handler for TCP send latency (child node 2).
 */
static ssize_t
latency_tcp_send_handler(struct rmib_call * call __unused,
	struct rmib_node * node __unused, struct rmib_oldp * oldp,
	struct rmib_newp * newp __unused)
{
	int r;

	if (oldp == NULL)
		return sizeof(struct latency_stats);

	if ((r = rmib_copyout(oldp, 0, &latency_tcp_send,
	    sizeof(struct latency_stats))) < 0)
		return r;
	return sizeof(struct latency_stats);
}

/* The minix.lwip.latency RMIB nodes (separate nodes per protocol). */
static struct rmib_node minix_lwip_latency_table[] = {
	[0] = RMIB_FUNC(RMIB_RO, sizeof(struct latency_stats),
	    latency_udp_handler, "udp_send",
	    "UDP send latency histogram"),
	[1] = RMIB_FUNC(RMIB_RO, sizeof(struct latency_stats),
	    latency_tcp_connect_handler, "tcp_connect",
	    "TCP connect latency histogram"),
	[2] = RMIB_FUNC(RMIB_RO, sizeof(struct latency_stats),
	    latency_tcp_send_handler, "tcp_send",
	    "TCP send latency histogram"),
};

static struct rmib_node minix_lwip_latency_node =
    RMIB_NODE(RMIB_RO, minix_lwip_latency_table, "latency",
	"Network latency histograms");

/*
 * Initialize the latency histogram module.
 */
void
latency_init(void)
{

	memset(&latency_udp_send, 0, sizeof(latency_udp_send));
	memset(&latency_tcp_connect, 0, sizeof(latency_tcp_connect));
	memset(&latency_tcp_send, 0, sizeof(latency_tcp_send));

	mibtree_register_lwip(&minix_lwip_latency_node);
}
