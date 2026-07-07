/*
 * MINIX 3 specific DTLS (Datagram TLS) support (RFC 6347/9147).
 *
 * Provides DTLS 1.2 and 1.3 over UDP sockets using wolfSSL as the
 * cryptographic backend.  This module implements a DTLS shim layer
 * between the application and the lwIP UDP socket, handling per-session
 * DTLS state, non-blocking handshake, and transparent encrypt/decrypt.
 *
 * Architecture: DTLS Shim Layer over UDP
 *   - NOT ALTCP (ALTCP is TCP-only)
 *   - Custom I/O callbacks for wolfSSL (pbuf-based, not socket fd)
 *   - Non-blocking handshake via lwIP timer callbacks
 *   - Decrypted plaintext injected into pktsock for normal recv path
 */
#ifndef LWIP_LWIP_DTLS_H
#define LWIP_LWIP_DTLS_H

#include "lwip/opt.h"

#if LWIP_DTLS /* don't build if not configured */

#include "lwip/udp.h"
#include "lwip/pbuf.h"
#include "lwip/ip_addr.h"
#include "lwip/err.h"

/* ------------------------------------------------------------------ */
/*  Constants                                                          */
/* ------------------------------------------------------------------ */

/* Socket option for enabling DTLS on a UDP socket */
#define UDP_DTLS  43   /* IPPROTO_UDP level */

/* DTLS protocol versions */
#define LWIP_DTLS_1_2     0xFEFF  /* DTLS 1.2 (RFC 6347) */
#define LWIP_DTLS_1_3     0xFEFC  /* DTLS 1.3 (RFC 9147) */

/* DTLS roles */
#define LWIP_DTLS_CLIENT  0  /* Client role — initiates handshake */
#define LWIP_DTLS_SERVER  1  /* Server role — accepts handshake */

/* DTLS session states */
#define LWIP_DTLS_NONE       0  /* DTLS not configured */
#define LWIP_DTLS_INIT       1  /* wolfSSL initialized, no handshake yet */
#define LWIP_DTLS_HANDSHAKE  2  /* Handshake in progress (non-blocking) */
#define LWIP_DTLS_ESTABLISHED 3 /* Handshake complete, data flowing */
#define LWIP_DTLS_CLOSING    4  /* Closure alert in progress */
#define LWIP_DTLS_FAILED     5  /* Handshake or crypto failure */

/* DTLS retry interval for non-blocking handshake (milliseconds) */
#define DTLS_HANDSHAKE_RETRY_MS  50

/* Max DTLS datagram size (sufficient for handshake + data) */
#define DTLS_MAX_DATALEN     8192

/* Max pending encrypted datagrams per session */
#define DTLS_MAX_PENDING     4

/* ------------------------------------------------------------------ */
/*  Forward declarations                                              */
/* ------------------------------------------------------------------ */

struct lwip_dtls_session;

/* ------------------------------------------------------------------ */
/*  Userspace DTLS configuration (for setsockopt)                     */
/* ------------------------------------------------------------------ */

struct lwip_dtls_config {
	uint8_t     enable;           /* 1 = enable, 0 = disable */
	uint8_t     dtls_version;     /* LWIP_DTLS_1_2 or LWIP_DTLS_1_3 */
	uint8_t     role;             /* LWIP_DTLS_CLIENT or LWIP_DTLS_SERVER */
	uint8_t     pad;
	/* Certificate and key material (PEM, optional for PSK mode) */
	const char *ca_cert;          /* CA certificate PEM */
	const char *cert;             /* Own certificate PEM */
	const char *key;              /* Private key PEM */
	size_t      ca_cert_len;
	size_t      cert_len;
	size_t      key_len;
};

/* ------------------------------------------------------------------ */
/*  DTLS statistics                                                    */
/* ------------------------------------------------------------------ */

struct lwip_dtls_stats {
	uint32_t    sessions_created;
	uint32_t    sessions_destroyed;
	uint32_t    handshake_completed;
	uint32_t    handshake_failed;
	uint32_t    decrypt_errors;
	uint32_t    encrypt_errors;
	uint32_t    dtls_packets_in;
	uint32_t    dtls_packets_out;
};

extern struct lwip_dtls_stats lwip_dtls_stats;

/* Runtime enable/disable toggle.  Set to 1 to enable DTLS processing. */
extern int lwip_dtls_enabled;

/* ------------------------------------------------------------------ */
/*  Public API                                                         */
/* ------------------------------------------------------------------ */

/*
 * Initialize the DTLS subsystem.  Must be called after lwip_init().
 * Calls wolfSSL_Init() internally.
 */
void lwip_dtls_init(void);

/*
 * Attach a DTLS session to a UDP PCB.
 * Called from udpsock_setsockopt(UDP_DTLS).
 * Returns 0 on success, or a negative error code.
 */
int lwip_dtls_attach(struct udp_pcb *pcb, const struct lwip_dtls_config *cfg,
	void **session_out);

/*
 * Detach a DTLS session from a UDP PCB and free all resources.
 * Called from udpsock_close().
 */
void lwip_dtls_detach(void *session);

/*
 * Process an incoming UDP datagram for a DTLS-enabled socket.
 * Called from udpsock_input() (the UDP receive callback).
 *
 * If the packet contains DTLS handshake data, it is consumed internally.
 * If it contains encrypted application data, wolfSSL decrypts it and
 * the plaintext is placed in the session's decrypted buffer.
 * Returns:
 *   ERR_OK  — packet was consumed, do NOT forward to pktsock
 *   ERR_INPROGRESS — packet was consumed (handshake), no plaintext yet
 *   ERR_VAL — packet is not DTLS (should not happen if DTLS is enabled)
 */
err_t lwip_dtls_input(void *session, struct pbuf *p,
	const ip_addr_t *src_addr, uint16_t src_port);

/*
 * Encrypt an outgoing payload for a DTLS-enabled socket.
 * Called from udpsock_send() before the actual udp_sendto().
 *
 * Returns:
 *   ERR_OK  — encryption successful, use the encrypted pbuf instead
 *   ERR_INPROGRESS — handshake in progress, try again later
 *   ERR_MEM — out of memory
 *   ERR_ABRT — DTLS error (encryption failed)
 */
err_t lwip_dtls_output(void *session, struct pbuf **p);

/*
 * Check if DTLS is established on this session.
 * Returns 1 if handshake is complete and data can flow.
 */
int lwip_dtls_is_established(void *session);

/*
 * Get the current DTLS session state name (for debugging).
 */
const char *lwip_dtls_state_str(void *session);

/*
 * Read decrypted plaintext from the DTLS session's internal buffer.
 * Used by udpsock_input() to inject plaintext into pktsock.
 * Returns the number of bytes read, 0 if no data available, or -1 on error.
 */
int lwip_dtls_read_plaintext(void *session, uint8_t *buf, int len);

/* ------------------------------------------------------------------ */
/*  Stubs for LWIP_DTLS == 0                                          */
/* ------------------------------------------------------------------ */

#else /* LWIP_DTLS */

#define lwip_dtls_init()
#define lwip_dtls_attach(pcb, cfg, out)  (-1)
#define lwip_dtls_detach(s)
#define lwip_dtls_input(s, p, a, port)   (ERR_VAL)
#define lwip_dtls_output(s, p)           (ERR_VAL)
#define lwip_dtls_is_established(s)      (0)
#define lwip_dtls_state_str(s)           ("DTLS disabled")
#define lwip_dtls_read_plaintext(s, b, l) (-1)

#endif /* LWIP_DTLS */

#endif /* !LWIP_LWIP_DTLS_H */
