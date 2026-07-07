/*
 * MINIX 3 specific SYN Cookie implementation for lwIP.
 *
 * SYN Cookies (RFC 4987) allow the TCP stack to cope with SYN flood
 * attacks by not allocating any state for half-open connections.
 *
 * When the listen backlog is exhausted (or tcp_alloc fails), instead
 * of silently dropping the SYN, we send a SYN-ACK with a specially
 * crafted ISN that encodes connection parameters in a verifiable way.
 * The client must respond with an ACK containing (cookie_ISN + 1);
 * if the cookie is valid, the connection is established directly
 * (no SYN_RCVD state needed).
 *
 * Reference: FreeBSD syncache implementation simplified for lwIP.
 */

#include "lwip/opt.h"

#if LWIP_TCP_SYNCOOKIE /* do not build unless configured */

#include "lwipsyncookie.h"

#include "lwip/tcp.h"
#include "lwip/priv/tcp_priv.h"
#include "lwip/ip.h"
#include "lwip/inet_chksum.h"
#include "lwip/memp.h"
#include "lwip/stats.h"

#include <string.h>
#include <sys/sha2.h>

/* ------------------------------------------------------------------ */
/* Internal state                                                      */
/* ------------------------------------------------------------------ */

/* Runtime toggle (sysctl).  Non-static for RMIB_INTPTR access. */
int lwip_syn_cookie_enabled = 1;

/* Timestamp counter, incremented every 2 seconds. */
static u32_t syn_cookie_timestamp;

/* Server secrets: [0] = current, [1] = previous (for overlap). */
static u8_t syn_cookie_secrets[SYN_COOKIE_NUM_SECRETS][SYN_COOKIE_SECRET_LEN];

/*
 * MSS table indexed by 3-bit MSS index (0-7).
 * Ordered by frequency: ethernet first, fallbacks for smaller MTUs.
 */
static const u16_t syn_cookie_mss_table[SYN_COOKIE_NUM_MSS] = {
    1460, /* Ethernet (1500 - 40 = 1460)           */
    1440, /* Ethernet w/ VLAN (1500-4-40 = 1456)   */
    1360, /* PPPoE (1492 - 40 = 1452, rounded)      */
    1024, /* Common smaller MTU                     */
    576,  /* Minimum IPv4 MTU (576 - 40)            */
    536,  /* RFC 879 default MSS                    */
    256,  /* Very constrained MTU                   */
    0     /* Invalid/fallback                       */
};

/* ------------------------------------------------------------------ */
/* Internal helper: compute cookie hash                                */
/* ------------------------------------------------------------------ */

/**
 * Compute a 24-bit hash of (src_ip, dst_ip, src_port, dst_port, secret).
 *
 * Input layout (52 bytes in a 64-byte SHA256 block):
 *   0-15:  src IP (16-byte IPv6, or IPv4-mapped IPv6)
 *  16-31:  dst IP (same)
 *  32-33:  src port (big-endian)
 *  34-35:  dst port (big-endian)
 *  36-51:  server secret (16 bytes)
 *  52-63:  zeros (padding)
 *
 * Returns the first 24 bits of the SHA256 digest.
 */
static u32_t
syn_cookie_hash(const ip_addr_t *src_ip, u16_t src_port,
                const ip_addr_t *dst_ip, u16_t dst_port,
                const u8_t *secret)
{
    u8_t input[SHA256_BLOCK_LENGTH];
    u8_t output[SHA256_DIGEST_LENGTH];
    SHA256_CTX ctx;

    memset(input, 0, sizeof(input));

    /* Encode source address (always 16 bytes). */
    if (IP_IS_V6(src_ip)) {
        memcpy(&input[0], ip_2_ip6(src_ip)->addr, 16);
    } else {
        memset(&input[0], 0, 10);
        input[10] = 0xff;
        input[11] = 0xff;
        memcpy(&input[12], ip_2_ip4(src_ip)->addr, 4);
    }

    /* Encode destination address (always 16 bytes). */
    if (IP_IS_V6(dst_ip)) {
        memcpy(&input[16], ip_2_ip6(dst_ip)->addr, 16);
    } else {
        memset(&input[16], 0, 10);
        input[26] = 0xff;
        input[27] = 0xff;
        memcpy(&input[28], ip_2_ip4(dst_ip)->addr, 4);
    }

    /* Encode ports (big-endian). */
    input[32] = (u8_t)(src_port >> 8);
    input[33] = (u8_t)(src_port & 0xff);
    input[34] = (u8_t)(dst_port >> 8);
    input[35] = (u8_t)(dst_port & 0xff);

    /* Copy secret. */
    memcpy(&input[36], secret, SYN_COOKIE_SECRET_LEN);

    /* SHA256 hash. */
    SHA256_Init(&ctx);
    SHA256_Update(&ctx, input, 36 + SYN_COOKIE_SECRET_LEN);
    SHA256_Final(output, &ctx);

    /* Return first 24 bits. */
    return ((u32_t)output[0] << 16) |
           ((u32_t)output[1] << 8)  |
           ((u32_t)output[2]);
}

/* ------------------------------------------------------------------ */
/* ISN encoding / decoding helpers                                     */
/* ------------------------------------------------------------------ */

static u32_t
syn_cookie_encode_isn(u32_t ts, u32_t mss_idx, u32_t hash)
{
    return (ts << (SYN_COOKIE_HASH_BITS + SYN_COOKIE_MSS_BITS)) |
           ((mss_idx & SYN_COOKIE_MSS_MASK) << SYN_COOKIE_HASH_BITS) |
           (hash & SYN_COOKIE_HASH_MASK);
}

static void
syn_cookie_decode_isn(u32_t isn, u32_t *ts, u32_t *mss_idx, u32_t *hash)
{
    *ts      = (isn >> (SYN_COOKIE_HASH_BITS + SYN_COOKIE_MSS_BITS)) &
                SYN_COOKIE_TS_MASK;
    *mss_idx = (isn >> SYN_COOKIE_HASH_BITS) & SYN_COOKIE_MSS_MASK;
    *hash    = isn & SYN_COOKIE_HASH_MASK;
}

/* ------------------------------------------------------------------ */
/* Secret generation                                                    */
/* ------------------------------------------------------------------ */

static void
syn_cookie_generate_secret(u8_t *secret)
{
    u32_t entropy[SYN_COOKIE_SECRET_LEN / sizeof(u32_t)];
    int i;

    for (i = 0; i < (int)LWIP_ARRAYSIZE(entropy); i++) {
        /* Mix stack address + timestamp + RNG for basic unpredictability. */
        entropy[i] = LWIP_RAND() ^ syn_cookie_timestamp ^
                     (u32_t)(uintptr_t)secret;
    }
    memcpy(secret, entropy, SYN_COOKIE_SECRET_LEN);
}

/* Initial CWND calculation per RFC 2581 (defined locally because
   LWIP_TCP_CALC_INITIAL_CWND is static in tcp_in.c). */
#ifndef LWIP_TCP_CALC_INITIAL_CWND
#define LWIP_TCP_CALC_INITIAL_CWND(mss) ((tcpwnd_size_t)LWIP_MIN((4U * (mss)), LWIP_MAX((2U * (mss)), 4380U)))
#endif

/* ------------------------------------------------------------------ */
/* Public API                                                          */
/* ------------------------------------------------------------------ */

void
lwip_syn_cookie_init(void)
{
    int i;

    syn_cookie_timestamp = 0;
    lwip_syn_cookie_enabled = 1;

    for (i = 0; i < SYN_COOKIE_NUM_SECRETS; i++) {
        syn_cookie_generate_secret(syn_cookie_secrets[i]);
    }
}

void
lwip_syn_cookie_tick(void)
{
    static int counter = 0;

    counter++;
    if (counter < SYN_COOKIE_TS_SECONDS) {
        return;
    }
    counter = 0;

    u32_t old_ts = syn_cookie_timestamp;
    syn_cookie_timestamp = (syn_cookie_timestamp + 1) & SYN_COOKIE_TS_MASK;

    /* When the 5-bit counter wraps, rotate secrets. */
    if (syn_cookie_timestamp < old_ts) {
        memcpy(syn_cookie_secrets[1], syn_cookie_secrets[0],
               SYN_COOKIE_SECRET_LEN);
        syn_cookie_generate_secret(syn_cookie_secrets[0]);
    }
}

int
lwip_syn_cookie_is_enabled(void)
{
    return lwip_syn_cookie_enabled;
}

void
lwip_syn_cookie_set_enabled(int enabled)
{
    lwip_syn_cookie_enabled = enabled ? 1 : 0;
}

/* ================================================================== */
/* SYN-ACK generation (called when backlog is full / OOM)               */
/* ================================================================== */

int
lwip_syn_cookie_send_synack(struct tcp_pcb_listen *pcb,
                            struct netif *netif,
                            const ip_addr_t *src_ip, u16_t src_port,
                            const ip_addr_t *dst_ip, u16_t dst_port,
                            u32_t syn_seqno)
{
    struct pbuf *p;
    struct tcp_hdr *tcphdr;
    u32_t cookie_isn, hash;
    u32_t mss_idx;
    u16_t mss, optlen;

    if (!lwip_syn_cookie_enabled || pcb == NULL || netif == NULL) {
        return 0;
    }

    /* Compute the MSS for this connection. */
#if TCP_CALCULATE_EFF_SEND_MSS
    mss = tcp_eff_send_mss_netif(TCP_MSS, netif, src_ip);
#else
    mss = TCP_MSS;
#endif

    /* Encode MSS in the 3-bit index. */
    for (mss_idx = 0; mss_idx < SYN_COOKIE_NUM_MSS; mss_idx++) {
        if (syn_cookie_mss_table[mss_idx] == mss ||
            syn_cookie_mss_table[mss_idx] == 0) {
            break;
        }
    }
    if (mss_idx >= SYN_COOKIE_NUM_MSS) {
        mss_idx = 0;
    }

    /* Compute the SYN cookie hash. */
    hash = syn_cookie_hash(src_ip, src_port, dst_ip, dst_port,
                           syn_cookie_secrets[0]);

    /* Build ISN = timestamp | mss_idx | hash[0:24). */
    cookie_isn = syn_cookie_encode_isn(syn_cookie_timestamp, mss_idx, hash);

    /* Get the actual MSS for this index. */
    mss = syn_cookie_mss_table[mss_idx];
    if (mss == 0) {
        mss = TCP_MSS;
    }

    /* Build the SYN-ACK segment (MSS option only, 4 bytes). */
    optlen = 4;

    p = pbuf_alloc(PBUF_IP, TCP_HLEN + optlen, PBUF_RAM);
    if (p == NULL) {
        return 0;
    }

    tcphdr = (struct tcp_hdr *)p->payload;
    memset(tcphdr, 0, TCP_HLEN + optlen);

    tcphdr->src   = lwip_htons(dst_port);  /* our port */
    tcphdr->dest  = lwip_htons(src_port);  /* client port */
    tcphdr->seqno = lwip_htonl(cookie_isn);
    tcphdr->ackno = lwip_htonl(syn_seqno + 1);
    TCPH_HDRLEN_FLAGS_SET(tcphdr, (TCP_HLEN + optlen) / 4, TCP_SYN | TCP_ACK);
    tcphdr->wnd   = lwip_htons(TCP_WND);
    tcphdr->chksum = 0;
    tcphdr->urgp  = 0;

    /* MSS option (kind=2, length=4, value=MSS in network order). */
    {
        u8_t *opts = (u8_t *)(tcphdr + 1);
        opts[0] = 2;            /* kind: MSS */
        opts[1] = 4;            /* length */
        opts[2] = (u8_t)(mss >> 8);
        opts[3] = (u8_t)(mss & 0xff);
    }

    /* Compute TCP checksum (pseudo-header). */
#if CHECKSUM_GEN_TCP
    IF__NETIF_CHECKSUM_ENABLED(netif, NETIF_CHECKSUM_GEN_TCP) {
        tcphdr->chksum = ip_chksum_pseudo(p, IP_PROTO_TCP, p->tot_len,
                                          dst_ip, src_ip);
    }
#endif

    /* Send via IP layer.  pbuf is consumed by ip_output_if(). */
    ip_output_if(p, dst_ip, src_ip, TCP_TTL, 0, IP_PROTO_TCP, netif);

    return 1;
}

/* ================================================================== */
/* ACK handling (called when ACK received on LISTEN socket)            */
/* ================================================================== */

int
lwip_syn_cookie_handle_ack(struct tcp_pcb_listen *pcb,
                           u32_t ackno, u32_t seqno,
                           const ip_addr_t *src_ip, u16_t src_port,
                           const ip_addr_t *dst_ip, u16_t dst_port,
                           u16_t wnd)
{
    struct tcp_pcb *npcb;
    u32_t ack_cookie;
    u32_t ts_recv, mss_idx, hash_recv;
    u32_t hash_computed;
    int valid, secret_idx;
    int ts_diff;
    err_t rc;

    if (!lwip_syn_cookie_enabled || pcb == NULL) {
        return 0;
    }

    /* The client's ACK acknowledges our SYN-ACK: ackno = cookie_ISN + 1. */
    if (ackno == 0) {
        return 0;
    }
    ack_cookie = ackno - 1;
    syn_cookie_decode_isn(ack_cookie, &ts_recv, &mss_idx, &hash_recv);

    /* Validate MSS index. */
    if (mss_idx >= SYN_COOKIE_NUM_MSS) {
        return 0;
    }

    /* Validate timestamp: must be within [-2, +1] of current. */
    ts_diff = (int)(ts_recv - syn_cookie_timestamp);
    if (ts_diff > (int)(SYN_COOKIE_TS_MASK / 2)) {
        ts_diff -= (int)(SYN_COOKIE_TS_MASK + 1);
    } else if (ts_diff < -(int)(SYN_COOKIE_TS_MASK / 2)) {
        ts_diff += (int)(SYN_COOKIE_TS_MASK + 1);
    }
    if (ts_diff < -2 || ts_diff > 1) {
        return 0; /* timestamp outside acceptable window */
    }

    /* Try both current and previous secrets. */
    valid = 0;
    for (secret_idx = 0; secret_idx < SYN_COOKIE_NUM_SECRETS; secret_idx++) {
        hash_computed = syn_cookie_hash(src_ip, src_port, dst_ip, dst_port,
                                        syn_cookie_secrets[secret_idx]);
        if (hash_computed == hash_recv) {
            valid = 1;
            break;
        }
    }

    if (!valid) {
        return 0; /* hash mismatch — not a valid cookie */
    }

    /* ----- Valid SYN cookie! Create the TCP PCB. ----- */

    npcb = tcp_alloc(pcb->prio);
    if (npcb == NULL) {
        TCP_STATS_INC(tcp.memerr);
        return 1; /* suppress RST; client will retry */
    }

    /* Initialise the new PCB as if it just left SYN_RCVD for ESTABLISHED. */
    ip_addr_copy(npcb->local_ip,   *dst_ip);
    ip_addr_copy(npcb->remote_ip,  *src_ip);
    npcb->local_port  = dst_port;
    npcb->remote_port = src_port;
    npcb->state       = ESTABLISHED;
    npcb->rcv_nxt     = seqno;
    npcb->rcv_ann_right_edge = npcb->rcv_nxt;
    npcb->snd_wl2     = ack_cookie;
    npcb->snd_nxt     = ackno;           /* ISN + 1 */
    npcb->lastack     = ackno;
    npcb->snd_lbb     = ackno;
    npcb->snd_wl1     = seqno - 1;
    npcb->callback_arg = pcb->callback_arg;
    npcb->mss         = syn_cookie_mss_table[mss_idx];

#if LWIP_CALLBACK_API || TCP_LISTEN_BACKLOG
    npcb->listener = pcb;
#endif

    /* Inherit socket options. */
    npcb->so_options = pcb->so_options & SOF_INHERITED;
    npcb->netif_idx  = pcb->netif_idx;

    /* Set send window from the incoming ACK. */
    npcb->snd_wnd     = wnd;
    npcb->snd_wnd_max = wnd;

    /* Initial congestion window (RFC 2581). */
    npcb->cwnd     = LWIP_TCP_CALC_INITIAL_CWND(npcb->mss);
    npcb->ssthresh = npcb->cwnd * 2;

#if LWIP_TSO
    if ((ip_current_netif() != NULL) &&
        (ip_current_netif()->flags & NETIF_FLAG_TSO)) {
        tcp_set_flags(npcb, TF_TSO);
    }
#endif

    /* Register the new PCB in the active list. */
    TCP_REG_ACTIVE(npcb);
    tcp_backlog_accepted(npcb);

    /* Notify the application. */
    TCP_EVENT_ACCEPT(pcb, npcb, pcb->callback_arg, ERR_OK, rc);
    if (rc != ERR_OK) {
        if (rc != ERR_ABRT) {
            tcp_abort(npcb);
        }
        return 1;
    }

#if MIB2_STATS
    MIB2_STATS_INC(mib2.tcppassiveopens);
#endif

    return 1;
}

#endif /* LWIP_TCP_SYNCOOKIE */
