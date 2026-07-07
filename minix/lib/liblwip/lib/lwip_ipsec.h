/*
 * MINIX 3 specific IPsec ESP Transport + AH support (RFC 4301/4302/4303).
 *
 * This module provides minimal IPsec functionality covering gaps not served
 * by WireGuard: ESP Transport Mode (encryption of IP payload) and AH
 * (authentication without encryption).  Manual keying only -- no IKEv2.
 *
 * Architecture: BITS (Bump In The Stack) with hooks in ip4_input() and
 * ip4_output_if_src() via LWIP_HOOK_IP4_INPUT and LWIP_HOOK_IP4_OUTPUT.
 */
#ifndef LWIP_LWIP_IPSEC_H
#define LWIP_LWIP_IPSEC_H

#include "lwip/opt.h"

#if LWIP_IPSEC /* don't build if not configured */

#include "lwip/ip.h"
#include "lwip/ip4.h"
#include "lwip/pbuf.h"
#include "lwip/err.h"
#include "lwip/tcp.h"
#include "lwip/prot/ip.h"

/* ------------------------------------------------------------------ */
/*  Constants                                                         */
/* ------------------------------------------------------------------ */

/* IP Protocol numbers for ESP and AH */
#define IP_PROTO_ESP  50  /* Encapsulating Security Payload */
#define IP_PROTO_AH   51  /* Authentication Header */

/* Socket option */
#define IP_IPSEC_SA  42  /* setsockopt level: IPPROTO_IP */

/* SA flags */
#define IPSEC_SA_FLAG_ESP      0x01  /* Enable ESP */
#define IPSEC_SA_FLAG_AH       0x02  /* Enable AH */
#define IPSEC_SA_FLAG_OUTBOUND 0x04  /* Outbound SA (per-socket) */

/* ESP encryption algorithms */
#define IPSEC_ESP_AES_GCM_16       1  /* AES-GCM, 16-byte ICV */
#define IPSEC_ESP_AES_GCM_8        2  /* AES-GCM, 8-byte ICV */
#define IPSEC_ESP_AES_CBC          3  /* AES-CBC + separate HMAC-SHA256 */
#define IPSEC_ESP_CHACHA20_POLY1305  4  /* ChaCha20-Poly1305 */

/* AH authentication algorithms */
#define IPSEC_AH_HMAC_SHA1_96      1  /* HMAC-SHA1-96 (RFC 2404) */
#define IPSEC_AH_HMAC_SHA256_128   2  /* HMAC-SHA256-128 (RFC 4868) */

/* SADB limits */
#define IPSEC_SADB_MAX_ENTRIES  32   /* max SA entries in global SADB */

/* Max key lengths */
#define IPSEC_MAX_KEY_LEN       64
#define IPSEC_MAX_AUTH_KEY_LEN  64
#define IPSEC_IV_LEN            16   /* max IV length (AES-CBC) */
#define IPSEC_SALT_LEN           4   /* for GCM/ChaCha20-Poly1305 */

/* AH header length (without ICV) */
#define IPSEC_AH_HDR_LEN        8    /* next_hdr(1)+len(1)+rsv(2)+spi(4) */

/* ESP overhead estimates */
#define IPSEC_ESP_OVERHEAD_MAX  64   /* worst-case padding + headers */

/* Anti-replay window size */
#define IPSEC_REPLAY_WINDOW     64

/* ------------------------------------------------------------------ */
/*  Algorithm key sizes and ICV lengths                               */
/* ------------------------------------------------------------------ */

#define IPSEC_AES128_KEY_LEN    16
#define IPSEC_AES256_KEY_LEN    32
#define IPSEC_CHACHA20_KEY_LEN  32
#define IPSEC_SHA256_HMAC_LEN   32
#define IPSEC_SHA1_HMAC_LEN     20
#define IPSEC_GCM_IV_LEN         8   /* 4-byte salt + 4-byte seq/IV */
#define IPSEC_CBC_IV_LEN        16

/* ------------------------------------------------------------------ */
/*  Data structures                                                    */
/* ------------------------------------------------------------------ */

/*
 * Userspace-facing SA structure for setsockopt(IP_IPSEC_SA).
 * The application fills this in and passes it via setsockopt.
 */
struct ipsec_sa {
	uint32_t    spi;              /* Security Parameter Index */
	uint8_t     flags;            /* IPSEC_SA_FLAG_* */
	uint8_t     esp_cipher;       /* ESP encryption algorithm (0=none) */
	uint8_t     ah_auth;          /* AH authentication algorithm (0=none) */
	uint8_t     pad;              /* padding */
	uint8_t     enc_key[IPSEC_MAX_KEY_LEN];
	uint8_t     enc_keylen;
	uint8_t     auth_key[IPSEC_MAX_AUTH_KEY_LEN];
	uint8_t     auth_keylen;
	uint8_t     salt[IPSEC_SALT_LEN]; /* for GCM/ChaCha20 */
};

/*
 * Internal SADB entry (global table).
 */
struct ipsec_sadb_entry {
	int         used;             /* 1 if slot is occupied */
	ip_addr_t   dst_ip;           /* destination IP address */
	uint32_t    spi;              /* Security Parameter Index */
	uint8_t     proto;            /* IPPROTO_ESP or IPPROTO_AH */
	uint8_t     flags;
	uint8_t     esp_cipher;
	uint8_t     ah_auth;
	uint8_t     enc_key[IPSEC_MAX_KEY_LEN];
	uint8_t     enc_keylen;
	uint8_t     auth_key[IPSEC_MAX_AUTH_KEY_LEN];
	uint8_t     auth_keylen;
	uint8_t     salt[IPSEC_SALT_LEN];
	uint32_t    seq;              /* outbound sequence number */
	uint32_t    replay_bitmap;    /* inbound anti-replay bitmap */
	uint32_t    replay_last_seq;  /* highest seen sequence number */
};

/* ------------------------------------------------------------------ */
/*  IPsec statistics                                                   */
/* ------------------------------------------------------------------ */

struct ipsec_stats {
	uint32_t    sa_miss;          /* SA not found */
	uint32_t    auth_fail;        /* ICV mismatch */
	uint32_t    hmac_fail;        /* HMAC verification failed */
	uint32_t    replay_drop;      /* anti-replay dropped */
	uint32_t    pad_fail;         /* padding invalid */
	uint32_t    esp_packets;      /* ESP packets processed */
	uint32_t    ah_packets;       /* AH packets processed */
	uint32_t    esp_bytes;        /* ESP payload bytes processed */
};

extern struct ipsec_stats lwip_ipsec_stats;

/* Runtime enable/disable toggle.  Set to 1 to enable IPsec processing. */
extern int lwip_ipsec_enabled;

/* ------------------------------------------------------------------ */
/*  Public API                                                         */
/* ------------------------------------------------------------------ */

/*
 * Initialise the IPsec module.  Must be called after lwip_init().
 */
void lwip_ipsec_init(void);

/*
 * SADB management.  Used by setsockopt handler and for inbound lookup.
 * For outbound, the SA is added with the destination IP from the PCB.
 * For inbound, the SA is looked up by (dst_ip, spi, proto).
 */
int  lwip_ipsec_sa_add(const struct ipsec_sa *sa, const ip_addr_t *dst);
int  lwip_ipsec_sa_del(uint32_t spi, const ip_addr_t *dst, uint8_t proto);
int  lwip_ipsec_sa_lookup(uint32_t spi, const ip_addr_t *dst,
	uint8_t proto, struct ipsec_sadb_entry **entry);

/*
 * Hook functions -- called from lwIP core via lwiphooks.h.
 *
 * LWIP_HOOK_IP4_INPUT:
 *   Called early in ip4_input().  If the packet is ESP (50) or AH (51),
 *   we process the IPsec transform and modify the packet in-place so that
 *   the inner protocol can be delivered normally.  Returns 1 if the packet
 *   was eaten (consumed/dropped) or 0 to continue processing.
 *
 * LWIP_HOOK_IP4_OUTPUT:
 *   Called near the end of ip4_output_if_src(), after the IP header is
 *   fully constructed and checksummed.  If the destination has an outbound
 *   SA, we transform the packet (add ESP/AH header, encrypt/auth) and send
 *   it via netif->output(), returning 1 to prevent the normal send path.
 */
int  lwip_ipsec_input_hook(struct pbuf *p, struct netif *inp);
int  lwip_ipsec_output_hook(struct pbuf *p, struct netif *netif,
	const ip4_addr_t *dest);

/* For tcpsock.c: get SA for a PCB (used to check if SA is configured) */
int  lwip_ipsec_has_sa(const struct tcp_pcb *pcb);

/* Stubs for LWIP_IPSEC==0 */
#else /* LWIP_IPSEC */

#define lwip_ipsec_init()
#define lwip_ipsec_sa_add(sa, dst)        (-1)
#define lwip_ipsec_sa_del(spi, dst, p)    (-1)
#define lwip_ipsec_sa_lookup(spi, d, p, e) (-1)
#define lwip_ipsec_input_hook(p, inp)     (0)
#define lwip_ipsec_output_hook(p, n, d)   (0)
#define lwip_ipsec_has_sa(pcb)            (0)

#endif /* LWIP_IPSEC */

#endif /* !LWIP_LWIP_IPSEC_H */
