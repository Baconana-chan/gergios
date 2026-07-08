/*
 * MINIX 3 specific rate limiting for lwIP.
 *
 * Token-bucket rate limiters for ICMP error messages, ARP/NDP packets,
 * and IP fragment reassembly.  Protects against DoS attacks by limiting
 * the rate at which these packets are processed.
 *
 * When the rate limit is exceeded, the packet is silently dropped.
 * This is consistent with how other OSes (Linux, FreeBSD) handle
 * rate-limited packets.
 */

#include "lwip/opt.h"

#if LWIP_RATELIMIT /* do not build unless configured */

#include "lwip_ratelimit.h"

#include <string.h>

/* ------------------------------------------------------------------ */
/* Global rate limiters                                                */
/* ------------------------------------------------------------------ */

struct lwip_rate_limiter lwip_rl_icmp_errors;
struct lwip_rate_limiter lwip_rl_arp;
struct lwip_rate_limiter lwip_rl_ndp;

/* Last tick counter (incremented by lwip_ratelimit_tick). */
static u32_t ratelimit_global_tick;

/* Optional callback for rate-limit drop notifications. */
static lwip_ratelimit_alert_cb_t ratelimit_alert_cb;

/* ------------------------------------------------------------------ */
/* Token bucket implementation                                         */
/* ------------------------------------------------------------------ */

void
lwip_rate_limiter_init(struct lwip_rate_limiter *rl,
                       u32_t rate_per_sec, u32_t burst)
{
    memset(rl, 0, sizeof(*rl));
    rl->max_tokens = burst;
    rl->rate       = rate_per_sec;
    rl->tokens     = burst;  /* start full */
    rl->last_tick  = 0;
}

int
lwip_rate_limiter_check(struct lwip_rate_limiter *rl)
{
    u32_t ticks_elapsed;
    u32_t tokens_to_add;

    /*
     * Add tokens for elapsed ticks since last check.
     * This allows the limiter to recover even if tick() is not
     * called frequently.
     */
    ticks_elapsed = ratelimit_global_tick - rl->last_tick;
    if (ticks_elapsed > 0) {
        /*
         * Prevent u32_t overflow when computing tokens_to_add.
         * If enough ticks have elapsed to fill the bucket regardless,
         * just set tokens to max and skip the multiplication.
         */
        if (ticks_elapsed >= (rl->max_tokens / rl->rate)) {
            rl->tokens = rl->max_tokens;
        } else {
            rl->tokens += rl->rate * ticks_elapsed;
            if (rl->tokens > rl->max_tokens) {
                rl->tokens = rl->max_tokens;
            }
        }
        rl->last_tick = ratelimit_global_tick;
    }

    /* Try to consume a token. */
    if (rl->tokens > 0) {
        rl->tokens--;
        return 1; /* allowed */
    }

    return 0; /* rate-limited */
}

/* ------------------------------------------------------------------ */
/* Public API                                                          */
/* ------------------------------------------------------------------ */

void
lwip_ratelimit_set_alert_cb(lwip_ratelimit_alert_cb_t cb)
{
    ratelimit_alert_cb = cb;
}

void
lwip_ratelimit_init(void)
{
    ratelimit_global_tick = 0;
    ratelimit_alert_cb = NULL;

    lwip_rate_limiter_init(&lwip_rl_icmp_errors,
        LWIP_RATELIMIT_ICMP_ERRORS_PER_SEC,
        LWIP_RATELIMIT_ICMP_ERROR_BURST);

    lwip_rate_limiter_init(&lwip_rl_arp,
        LWIP_RATELIMIT_ARP_PER_SEC,
        LWIP_RATELIMIT_ARP_BURST);

    lwip_rate_limiter_init(&lwip_rl_ndp,
        LWIP_RATELIMIT_NDP_PER_SEC,
        LWIP_RATELIMIT_NDP_BURST);
}

void
lwip_ratelimit_tick(void)
{
    ratelimit_global_tick++;
}

int
lwip_ratelimit_icmp_error(void)
{
    if (!lwip_rate_limiter_check(&lwip_rl_icmp_errors)) {
        if (ratelimit_alert_cb != NULL)
            ratelimit_alert_cb(0);
        return 0;
    }
    return 1;
}

int
lwip_ratelimit_arp(void)
{
    if (!lwip_rate_limiter_check(&lwip_rl_arp)) {
        if (ratelimit_alert_cb != NULL)
            ratelimit_alert_cb(1);
        return 0;
    }
    return 1;
}

int
lwip_ratelimit_ndp(void)
{
    if (!lwip_rate_limiter_check(&lwip_rl_ndp)) {
        if (ratelimit_alert_cb != NULL)
            ratelimit_alert_cb(2);
        return 0;
    }
    return 1;
}

#endif /* LWIP_RATELIMIT */
