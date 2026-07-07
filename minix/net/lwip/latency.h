#ifndef MINIX_NET_LWIP_LATENCY_H
#define MINIX_NET_LWIP_LATENCY_H

#include <stdint.h>

/*
 * Latency histogram bucket boundaries (in microseconds).
 * Each bucket counts the number of operations whose latency falls
 * within its range.  The last bucket counts all latencies above the
 * last boundary.
 */
#define LATENCY_NUM_BUCKETS	7

extern const uint32_t latency_buckets[LATENCY_NUM_BUCKETS];

/*
 * Per-protocol latency statistics.
 */
struct latency_stats {
	uint64_t ls_count;				/* total operations */
	uint64_t ls_buckets[LATENCY_NUM_BUCKETS];	/* histogram */
};

/*
 * Global latency tracking for network protocols.
 */
extern struct latency_stats latency_udp_send;
extern struct latency_stats latency_tcp_connect;
extern struct latency_stats latency_tcp_send;

void latency_init(void);
void latency_record(struct latency_stats * stats, uint32_t duration_us);

#endif /* !MINIX_NET_LWIP_LATENCY_H */
