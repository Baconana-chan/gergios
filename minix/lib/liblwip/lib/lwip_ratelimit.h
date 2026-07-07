/*
 * MINIX 3 specific rate limiting for lwIP.
 *
 * Implements token-bucket rate limiters for ICMP error messages,
 * ARP/NDP packets, and IP fragment reassembly.  This protects against
 * various network-based denial-of-service attacks.
 *
 * The token bucket algorithm allows bursts up to the bucket size
 * while enforcing a long-term average rate.  Each "tick" adds tokens
 * to the bucket up to the maximum capacity.
 */
#ifndef LWIP_LWIP_RATELIMIT_H
#define LWIP_LWIP_RATELIMIT_H

#include "lwip/opt.h"

/* ------------------------------------------------------------------ */
/* Configuration defaults                                              */
/* ------------------------------------------------------------------ */

/* ICMP error rate limit: max ICMP error responses per second. */
#ifndef LWIP_RATELIMIT_ICMP_ERRORS_PER_SEC
#define LWIP_RATELIMIT_ICMP_ERRORS_PER_SEC   10
#endif

/* ICMP error burst size: max ICMP errors in a short burst. */
#ifndef LWIP_RATELIMIT_ICMP_ERROR_BURST
#define LWIP_RATELIMIT_ICMP_ERROR_BURST      20
#endif

/* ARP rate limit: max ARP packets (requests + replies) per second. */
#ifndef LWIP_RATELIMIT_ARP_PER_SEC
#define LWIP_RATELIMIT_ARP_PER_SEC           50
#endif

/* ARP burst size: max ARP packets in a short burst. */
#ifndef LWIP_RATELIMIT_ARP_BURST
#define LWIP_RATELIMIT_ARP_BURST            100
#endif

/* NDP rate limit: max NDP packets (NS, NA, RS, RA) per second. */
#ifndef LWIP_RATELIMIT_NDP_PER_SEC
#define LWIP_RATELIMIT_NDP_PER_SEC           50
#endif

/* NDP burst size: max NDP packets in a short burst. */
#ifndef LWIP_RATELIMIT_NDP_BURST
#define LWIP_RATELIMIT_NDP_BURST            100
#endif

/* IP fragment limit: max concurrent fragment reassembly datagrams. */
#ifndef LWIP_RATELIMIT_MAX_REASS_DATAGRAMS
#define LWIP_RATELIMIT_MAX_REASS_DATAGRAMS  16
#endif

/* IP fragment limit: max pbufs across all reassemblies. */
#ifndef LWIP_RATELIMIT_MAX_REASS_PBUFS
#define LWIP_RATELIMIT_MAX_REASS_PBUFS      256
#endif

/* ------------------------------------------------------------------ */
/* Rate limiter state (token bucket)                                   */
/* ------------------------------------------------------------------ */

struct lwip_rate_limiter {
    u32_t tokens;        /* current token count */
    u32_t max_tokens;    /* bucket capacity (burst size) */
    u32_t rate;          /* tokens added per tick (per-second rate / ticks_per_sec) */
    u32_t last_tick;     /* last tick when tokens were added */
};

/* ------------------------------------------------------------------ */
/* API                                                                 */
/* ------------------------------------------------------------------ */

#if LWIP_RATELIMIT

/**
 * Initialize a rate limiter with the given parameters.
 * Call once during service initialization.
 */
void lwip_rate_limiter_init(struct lwip_rate_limiter *rl,
                            u32_t rate_per_sec, u32_t burst);

/**
 * Check if an operation is allowed under the rate limit.
 * Returns 1 if allowed, 0 if rate-limited (dropped).
 */
int lwip_rate_limiter_check(struct lwip_rate_limiter *rl);

/**
 * Initialize all rate limiters.  Called during lwIP service init.
 */
void lwip_ratelimit_init(void);

/**
 * Tick all rate limiters.  Called every second from the timer.
 */
void lwip_ratelimit_tick(void);

/* Global rate limiters (defined in lwip_ratelimit.c). */
extern struct lwip_rate_limiter lwip_rl_icmp_errors;
extern struct lwip_rate_limiter lwip_rl_arp;
extern struct lwip_rate_limiter lwip_rl_ndp;

/*
 * Per-protocol rate-limit check helpers.
 * Returns 1 if the packet should be processed, 0 if it should be dropped.
 */

/** Check ICMP error rate limit. */
int lwip_ratelimit_icmp_error(void);

/** Check ARP packet rate limit. */
int lwip_ratelimit_arp(void);

/** Check NDP packet rate limit. */
int lwip_ratelimit_ndp(void);

#else /* !LWIP_RATELIMIT */

/* Stubs when rate limiting is disabled. */
#define lwip_ratelimit_init()
#define lwip_ratelimit_tick()
#define lwip_ratelimit_icmp_error()       1
#define lwip_ratelimit_arp()              1
#define lwip_ratelimit_ndp()              1

#endif /* LWIP_RATELIMIT */

#endif /* !LWIP_LWIP_RATELIMIT_H */
