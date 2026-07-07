/*
 * MINIX 3 specific SYN Cookie hooks for lwIP.
 *
 * SYN Cookies (RFC 4987) protect against SYN flood attacks by encoding
 * connection state in the Initial Sequence Number (ISN) of the SYN-ACK
 * segment.  This allows the server to avoid allocating state for
 * half-open connections when the listen backlog is exhausted or when
 * memory for a new PCB cannot be allocated.
 *
 * Cookie encoding (32-bit ISN):
 *   bits [31:27] — timestamp counter (5 bit, mod 32, ~2s tick, ~64s rotation)
 *   bits [26:24] — MSS index (3 bit, 8 values: 1460, 1440, 1360, 1024,
 *                               576, 536, 256, 0)
 *   bits [23:0]  — SHA256(4-tuple + server secret) truncated to 24 bits
 */
#ifndef LWIP_LWIPSYNCOOKIE_H
#define LWIP_LWIPSYNCOOKIE_H

#include "lwip/opt.h"
#include "lwip/ip_addr.h"
#include "lwip/tcp.h"
#include "lwip/netif.h"

/* ------------------------------------------------------------------ */
/* Configuration                                                      */
/* ------------------------------------------------------------------ */

#define SYN_COOKIE_NUM_SECRETS  2    /* current + previous */
#define SYN_COOKIE_SECRET_LEN   16   /* 128 bits entropy */
#define SYN_COOKIE_TS_BITS      5
#define SYN_COOKIE_TS_MASK      ((1u << SYN_COOKIE_TS_BITS) - 1)
#define SYN_COOKIE_MSS_BITS     3
#define SYN_COOKIE_MSS_MASK     ((1u << SYN_COOKIE_MSS_BITS) - 1)
#define SYN_COOKIE_HASH_BITS    24
#define SYN_COOKIE_HASH_MASK    ((1u << SYN_COOKIE_HASH_BITS) - 1)
#define SYN_COOKIE_TS_SECONDS   2
#define SYN_COOKIE_NUM_MSS      (1u << SYN_COOKIE_MSS_BITS)

/* ------------------------------------------------------------------ */
/* API                                                                 */
/* ------------------------------------------------------------------ */

#if LWIP_TCP_SYNCOOKIE

extern int lwip_syn_cookie_enabled;

void lwip_syn_cookie_init(void);
void lwip_syn_cookie_tick(void);

int  lwip_syn_cookie_is_enabled(void);
void lwip_syn_cookie_set_enabled(int enabled);

/**
 * Send a SYN-ACK with a cookie ISN (called when backlog is full).
 *
 * @param pcb       listening PCB
 * @param netif     netif on which the SYN arrived
 * @param src_ip    source IP address of the SYN
 * @param src_port  source TCP port of the SYN
 * @param dst_ip    dest IP address (our address)
 * @param dst_port  dest TCP port (our port)
 * @return 1 if SYN-ACK was sent, 0 on failure
 */
int lwip_syn_cookie_send_synack(struct tcp_pcb_listen *pcb,
                                struct netif *netif,
                                const ip_addr_t *src_ip, u16_t src_port,
                                const ip_addr_t *dst_ip, u16_t dst_port,
                                u32_t syn_seqno);

/**
 * Handle an incoming ACK on a listening connection.
 * Validates the SYN cookie from the ACK's acknowledge number.
 *
 * @param pcb       listening PCB
 * @param ackno     acknowledgement number from the TCP header
 * @param seqno     sequence number from the TCP header
 * @param src_ip    source IP address of the ACK
 * @param src_port  source TCP port of the ACK
 * @param dst_ip    dest IP address (our address)
 * @param dst_port  dest TCP port (our port)
 * @param wnd       TCP window from the ACK
 * @return 1 if a valid cookie was found and connection created, 0 otherwise
 */
int lwip_syn_cookie_handle_ack(struct tcp_pcb_listen *pcb,
                               u32_t ackno, u32_t seqno,
                               const ip_addr_t *src_ip, u16_t src_port,
                               const ip_addr_t *dst_ip, u16_t dst_port,
                               u16_t wnd);

#else /* !LWIP_TCP_SYNCOOKIE */

#define lwip_syn_cookie_init()
#define lwip_syn_cookie_tick()
#define lwip_syn_cookie_enabled                0
#define lwip_syn_cookie_is_enabled()           0
#define lwip_syn_cookie_set_enabled(enabled)
#define lwip_syn_cookie_send_synack(p,n,sip,sp,dip,dp,sq)    0
#define lwip_syn_cookie_handle_ack(p,a,s,sp,sp2,dp,dp2,w)     0

#endif /* LWIP_TCP_SYNCOOKIE */

#endif /* !LWIP_LWIPSYNCOOKIE_H */
