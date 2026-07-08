/*
 * perf_alerts.h — Network performance alert thresholds and API.
 *
 * Provides rate-limited syslog-based alerts for network performance
 * anomalies: packet drops, TCP resets, out-of-memory conditions,
 * high-latency events, and rate-limiter activations.
 *
 * Each alert type uses a configurable threshold (events per tick) and
 * a cooldown period (default 30 seconds) to prevent log flooding.
 *
 * Usage:
 *   perf_alerts_init();        // one-time init
 *   perf_alerts_tick();        // called periodically to reset counters
 *   perf_alerts_drop(ifdev);   // packet drop on an interface
 *   perf_alerts_tcp_rst();     // TCP RST event
 *   perf_alerts_oom();         // out-of-memory
 *   perf_alerts_latency(us);   // high-latency event
 *   perf_alerts_rate_limit();  // rate limiter triggered
 */

#ifndef MINIX_NET_LWIP_PERF_ALERTS_H
#define MINIX_NET_LWIP_PERF_ALERTS_H

#include <stdint.h>

/* ── Default thresholds ──────────────────────────────────────────── */
#define PERF_ALERTS_DROP_THRESH_DEF	100	/* drops per tick */
#define PERF_ALERTS_RST_THRESH_DEF	50	/* RSTs per tick */
#define PERF_ALERTS_OOM_THRESH_DEF	10	/* OOMs per tick */
#define PERF_ALERTS_LATENCY_US_DEF	100000	/* 100 ms */
#define PERF_ALERTS_RATELIMIT_THRESH_DEF 100	/* rate-limit hits per tick */

#define PERF_ALERTS_COOLDOWN_S		30	/* seconds between same alerts */

/* ── Public API ──────────────────────────────────────────────────── */
void perf_alerts_init(void);
void perf_alerts_tick(void);

void perf_alerts_drop(const char *ifname);
void perf_alerts_tcp_rst(void);
void perf_alerts_oom(void);
void perf_alerts_latency(uint32_t duration_us);
void perf_alerts_rate_limit(void);

#endif /* MINIX_NET_LWIP_PERF_ALERTS_H */
