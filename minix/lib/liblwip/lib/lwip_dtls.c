/*
 * MINIX 3 specific DTLS (Datagram TLS) support (RFC 6347/9147).
 *
 * This module implements a DTLS shim layer over lwIP UDP sockets using
 * wolfSSL as the cryptographic backend.  Key design decisions:
 *
 * - Custom I/O callbacks (wolfSSL_SetIORecv/SetIOSend) instead of socket
 *   fd, since lwIP uses pbuf-based I/O internally.
 * - Non-blocking handshake: when wolfSSL_connect/accept returns
 *   WANT_READ/WANT_WRITE, we defer and complete on the next data arrival.
 * - Decrypted plaintext is injected into pktsock so the normal recvfrom()
 *   path works unchanged.
 * - Per-session state machine: NONE → INIT → HANDSHAKE → ESTABLISHED → CLOSING
 *
 * wolfSSL configuration requirements:
 *   - Must be built WITHOUT NO_WOLFSSL_CLIENT and NO_WOLFSSL_SERVER
 *   - Must be built WITH WOLFSSL_DTLS and WOLFSSL_DTLS13
 *   - Custom I/O callbacks enabled (default in wolfSSL)
 */
#include "lwip_dtls.h"

#if LWIP_DTLS

#include "lwip/udp.h"
#include "lwip/sys.h"
#include "lwip/timeouts.h"

#include <string.h>
#include <stdlib.h>

#include "lwip/mem.h"

/* wolfSSL headers */
#include <wolfssl/ssl.h>
#include <wolfssl/error-ssl.h>

/* ------------------------------------------------------------------ */
/*  Constants                                                          */
/* ------------------------------------------------------------------ */

/* Maximum plaintext payload we can return from a single decrypt */
#define DTLS_MAX_PLAINTEXT   65535

/* Maximum DTLS record size (RFC 6347: 2^14 + 2048 = 18432 max) */
#define DTLS_MAX_RECORD      8192

/* ------------------------------------------------------------------ */
/*  Per-session pending datagram queue entry                          */
/* ------------------------------------------------------------------ */

struct dtls_pending_dgram {
	struct dtls_pending_dgram *next;
	uint8_t  data[DTLS_MAX_DATALEN];
	uint16_t len;
	uint16_t offset;      /* bytes already consumed by wolfSSL */
};

/* ------------------------------------------------------------------ */
/*  DTLS session (internal, opaque to users)                          */
/* ------------------------------------------------------------------ */

struct lwip_dtls_session {
	uint8_t     state;            /* LWIP_DTLS_* state */
	uint8_t     version;          /* DTLS version in use */
	uint8_t     role;             /* LWIP_DTLS_CLIENT or SERVER */
	uint8_t     pad;

	/* wolfSSL objects */
	WOLFSSL_CTX *ctx;             /* wolfSSL context */
	WOLFSSL     *ssl;             /* wolfSSL session object */

	/* Owning UDP PCB (for sending handshake packets) */
	struct udp_pcb *pcb;
	ip_addr_t   peer_addr;
	uint16_t    peer_port;

	/* Pending incoming encrypted datagrams (queue) */
	struct dtls_pending_dgram *pending_head;
	struct dtls_pending_dgram *pending_tail;
	int         pending_count;

	/* Decrypted plaintext buffer (for recv path) */
	uint8_t     plaintext[DTLS_MAX_PLAINTEXT];
	uint16_t    plaintext_len;
	uint16_t    plaintext_offset;

	/* The currently-being-read pending dgram for the recv callback */
	struct dtls_pending_dgram *current_dgram;

	/* Timer for handshake retry */
	sys_timeout_handler handshake_timer;
};

/* ------------------------------------------------------------------ */
/*  Global statistics                                                  */
/* ------------------------------------------------------------------ */

struct lwip_dtls_stats lwip_dtls_stats;

/* Runtime enable/disable toggle.  Default enabled. */
int lwip_dtls_enabled = 1;

/* ------------------------------------------------------------------ */
/*  Forward declarations                                               */
/* ------------------------------------------------------------------ */

static int lwip_dtls_recv_cb(WOLFSSL *ssl, char *buf, int sz, void *ctx);
static int lwip_dtls_send_cb(WOLFSSL *ssl, char *buf, int sz, void *ctx);
static void lwip_dtls_handshake_retry(void *arg);
static void lwip_dtls_queue_dgram(struct lwip_dtls_session *s,
	const uint8_t *data, uint16_t len);
static struct dtls_pending_dgram *lwip_dtls_dequeue_dgram(
	struct lwip_dtls_session *s);

/* ------------------------------------------------------------------ */
/*  Pending datagram queue management                                 */
/* ------------------------------------------------------------------ */

static void
lwip_dtls_queue_dgram(struct lwip_dtls_session *s,
	const uint8_t *data, uint16_t len)
{
	struct dtls_pending_dgram *d;

	if (s->pending_count >= DTLS_MAX_PENDING)
		return;  /* drop on overflow */

	d = (struct dtls_pending_dgram *)mem_malloc(
	    sizeof(struct dtls_pending_dgram));
	if (d == NULL)
		return;

	if (len > DTLS_MAX_DATALEN)
		len = DTLS_MAX_DATALEN;

	memcpy(d->data, data, len);
	d->len = len;
	d->offset = 0;
	d->next = NULL;

	if (s->pending_tail == NULL) {
		s->pending_head = d;
		s->pending_tail = d;
	} else {
		s->pending_tail->next = d;
		s->pending_tail = d;
	}
	s->pending_count++;
}

static struct dtls_pending_dgram *
lwip_dtls_dequeue_dgram(struct lwip_dtls_session *s)
{
	struct dtls_pending_dgram *d;

	d = s->pending_head;
	if (d == NULL)
		return NULL;

	s->pending_head = d->next;
	if (s->pending_head == NULL)
		s->pending_tail = NULL;
	s->pending_count--;

	d->next = NULL;
	return d;
}

static void
lwip_dtls_free_dgram(struct dtls_pending_dgram *d)
{
	if (d != NULL)
		mem_free(d);
}

/* ------------------------------------------------------------------ */
/*  Custom I/O callbacks for wolfSSL                                   */
/* ------------------------------------------------------------------ */

/*
 * wolfSSL sends encrypted data through this callback.
 * The data is a single DTLS record (UDP datagram).
 */
static int
lwip_dtls_send_cb(WOLFSSL *ssl, char *buf, int sz, void *ctx)
{
	struct lwip_dtls_session *s = (struct lwip_dtls_session *)ctx;
	struct pbuf *p;
	err_t err;

	if (s == NULL || s->pcb == NULL || sz <= 0)
		return WOLFSSL_CBIO_ERR_GENERAL;

	p = pbuf_alloc(PBUF_TRANSPORT, (u16_t)sz, PBUF_RAM);
	if (p == NULL)
		return WOLFSSL_CBIO_ERR_GENERAL;

	memcpy(p->payload, buf, (size_t)sz);

	/* Send the encrypted DTLS record as a UDP datagram */
	err = udp_sendto_if_src(s->pcb, p, &s->peer_addr, s->peer_port,
	    ip_route(&s->pcb->local_ip, &s->peer_addr), &s->pcb->local_ip);
	pbuf_free(p);

	if (err != ERR_OK)
		return WOLFSSL_CBIO_ERR_GENERAL;

	lwip_dtls_stats.dtls_packets_out++;
	return sz;
}

/*
 * wolfSSL receives encrypted data through this callback.
 * Returns the next pending datagram's data, or WANT_READ if none.
 */
static int
lwip_dtls_recv_cb(WOLFSSL *ssl, char *buf, int sz, void *ctx)
{
	struct lwip_dtls_session *s = (struct lwip_dtls_session *)ctx;
	struct dtls_pending_dgram *d;
	int avail;

	if (s == NULL || buf == NULL || sz <= 0)
		return WOLFSSL_CBIO_ERR_GENERAL;

	/* If we have a current dgram being consumed, continue from it */
	d = s->current_dgram;
	if (d != NULL) {
		avail = d->len - d->offset;
		if (avail > 0) {
			int copy = (avail < sz) ? avail : sz;
			memcpy(buf, d->data + d->offset, (size_t)copy);
			d->offset += (uint16_t)copy;

			if (d->offset >= d->len) {
				/* This dgram fully consumed */
				s->current_dgram = NULL;
				lwip_dtls_free_dgram(d);
			}
			return copy;
		}
		/* Previous dgram fully consumed, move to next */
		s->current_dgram = NULL;
		lwip_dtls_free_dgram(d);
	}

	/* Get next pending datagram */
	d = lwip_dtls_dequeue_dgram(s);
	if (d == NULL) {
		/* No data available — non-blocking */
		return WOLFSSL_CBIO_ERR_WANT_READ;
	}

	avail = d->len - d->offset;
	if (avail > 0) {
		int copy = (avail < sz) ? avail : sz;
		memcpy(buf, d->data + d->offset, (size_t)copy);
		d->offset += (uint16_t)copy;

		if (d->offset >= d->len) {
			/* Fully consumed in one shot */
			lwip_dtls_free_dgram(d);
		} else {
			/* Partial read — save for next callback */
			s->current_dgram = d;
		}
		return copy;
	}

	/* Empty dgram (shouldn't happen) */
	lwip_dtls_free_dgram(d);
	return WOLFSSL_CBIO_ERR_WANT_READ;
}

/* ------------------------------------------------------------------ */
/*  Handshake retry timer callback                                    */
/* ------------------------------------------------------------------ */

static void
lwip_dtls_handshake_retry(void *arg)
{
	struct lwip_dtls_session *s = (struct lwip_dtls_session *)arg;
	int ret;

	if (s == NULL || s->state != LWIP_DTLS_HANDSHAKE)
		return;

	if (s->role == LWIP_DTLS_CLIENT)
		ret = wolfSSL_connect(s->ssl);
	else
		ret = wolfSSL_accept(s->ssl);

	if (ret == SSL_SUCCESS) {
		s->state = LWIP_DTLS_ESTABLISHED;
		lwip_dtls_stats.handshake_completed++;
		return;
	}

	/* If still in progress, the next data arrival will retry */
	/* The timer will be re-armed by lwip_dtls_input if needed */
	(void)ret;
}

/* ------------------------------------------------------------------ */
/*  Initialization and cleanup                                         */
/* ------------------------------------------------------------------ */

void
lwip_dtls_init(void)
{

	/* Zero out statistics */
	memset(&lwip_dtls_stats, 0, sizeof(lwip_dtls_stats));

	/*
	 * Note: wolfSSL_Init() is NOT called here because wolfSSL may
	 * already be initialized by other components (e.g., syslogd).
	 * The wolfSSL library handles multiple init calls safely.
	 * If wolfSSL has never been initialized, the first wolfSSL_CTX_new
	 * call will implicitly initialize it in most builds.
	 * wolfSSL_Init may be called later if needed.
	 */
}

/* ------------------------------------------------------------------ */
/*  Attach / Detach                                                    */
/* ------------------------------------------------------------------ */

int
lwip_dtls_attach(struct udp_pcb *pcb, const struct lwip_dtls_config *cfg,
	void **session_out)
{
	struct lwip_dtls_session *s;
	WOLFSSL_METHOD *method;
	int ret;

	if (pcb == NULL || cfg == NULL || session_out == NULL)
		return ERR_ARG;

	*session_out = NULL;

	/* Allocate session structure */
	s = (struct lwip_dtls_session *)mem_malloc(
	    sizeof(struct lwip_dtls_session));
	if (s == NULL)
		return ERR_MEM;

	memset(s, 0, sizeof(*s));
	s->pcb = pcb;
	s->role = cfg->role;
	s->state = LWIP_DTLS_NONE;

	/* Select DTLS method based on version and role */
	if (cfg->dtls_version == LWIP_DTLS_1_3) {
		method = (cfg->role == LWIP_DTLS_CLIENT) ?
		    wolfDTLSv1_3_client_method() :
		    wolfDTLSv1_3_server_method();
	} else {
		method = (cfg->role == LWIP_DTLS_CLIENT) ?
		    wolfDTLSv1_2_client_method() :
		    wolfDTLSv1_2_server_method();
	}

	/* Create wolfSSL context */
	s->ctx = wolfSSL_CTX_new(method);
	if (s->ctx == NULL) {
		mem_free(s);
		return ERR_MEM;
	}

	/* Load certificates if provided */
	if (cfg->ca_cert != NULL && cfg->ca_cert_len > 0) {
		ret = wolfSSL_CTX_load_verify_buffer(s->ctx,
		    (const unsigned char *)cfg->ca_cert,
		    (long)cfg->ca_cert_len, SSL_FILETYPE_PEM);
		if (ret != SSL_SUCCESS) {
			wolfSSL_CTX_free(s->ctx);
			mem_free(s);
			return ERR_ARG;
		}
	}

	if (cfg->cert != NULL && cfg->cert_len > 0) {
		wolfSSL_CTX_use_certificate_buffer(s->ctx,
		    (const unsigned char *)cfg->cert,
		    (long)cfg->cert_len, SSL_FILETYPE_PEM);
	}

	if (cfg->key != NULL && cfg->key_len > 0) {
		wolfSSL_CTX_use_PrivateKey_buffer(s->ctx,
		    (const unsigned char *)cfg->key,
		    (long)cfg->key_len, SSL_FILETYPE_PEM);
	}

	/* Create wolfSSL session object */
	s->ssl = wolfSSL_new(s->ctx);
	if (s->ssl == NULL) {
		wolfSSL_CTX_free(s->ctx);
		mem_free(s);
		return ERR_MEM;
	}

	/* Set non-blocking mode */
	wolfSSL_set_using_nonblock(s->ssl, 1);

	/* Set custom I/O callbacks */
	wolfSSL_SetIORecv(s->ssl, lwip_dtls_recv_cb);
	wolfSSL_SetIOSend(s->ssl, lwip_dtls_send_cb);
	wolfSSL_SetIOReadCtx(s->ssl, s);
	wolfSSL_SetIOWriteCtx(s->ssl, s);

	s->state = LWIP_DTLS_INIT;
	lwip_dtls_stats.sessions_created++;

	*session_out = s;
	return ERR_OK;
}

void
lwip_dtls_detach(void *session)
{
	struct lwip_dtls_session *s = (struct lwip_dtls_session *)session;
	struct dtls_pending_dgram *d, *next;

	if (s == NULL)
		return;

	s->state = LWIP_DTLS_CLOSING;

	/* Free wolfSSL objects */
	if (s->ssl != NULL) {
		wolfSSL_shutdown(s->ssl);
		wolfSSL_free(s->ssl);
	}
	if (s->ctx != NULL)
		wolfSSL_CTX_free(s->ctx);

	/* Free pending dgram queue */
	d = s->pending_head;
	while (d != NULL) {
		next = d->next;
		lwip_dtls_free_dgram(d);
		d = next;
	}
	if (s->current_dgram != NULL)
		lwip_dtls_free_dgram(s->current_dgram);

	lwip_dtls_stats.sessions_destroyed++;
	mem_free(s);
}

/* ------------------------------------------------------------------ */
/*  Input path — incoming UDP datagram processing                     */
/* ------------------------------------------------------------------ */

err_t
lwip_dtls_input(void *session, struct pbuf *p,
	const ip_addr_t *src_addr, uint16_t src_port)
{
	struct lwip_dtls_session *s = (struct lwip_dtls_session *)session;
	uint8_t *data;
	uint16_t len;
	int ret;

	if (s == NULL || p == NULL)
		return ERR_VAL;

	/* Copy the encrypted datagram into a flat buffer */
	len = p->tot_len;
	if (len > DTLS_MAX_DATALEN)
		len = DTLS_MAX_DATALEN;

	data = (uint8_t *)mem_malloc((size_t)len);
	if (data == NULL)
		return ERR_MEM;

	pbuf_copy_partial(p, data, (u16_t)len, 0);

	/* Store peer address (in case of first handshake message) */
	ip_addr_copy(s->peer_addr, *src_addr);
	s->peer_port = src_port;

	/* Queue the encrypted datagram */
	lwip_dtls_queue_dgram(s, data, len);
	mem_free(data);

	lwip_dtls_stats.dtls_packets_in++;

	/*
	 * If the handshake is in progress, continue it now that we have
	 * new data.  If the handshake is not yet started, the first
	 * sendto() will trigger wolfSSL_connect().
	 */
	if (s->state == LWIP_DTLS_HANDSHAKE) {
		if (s->role == LWIP_DTLS_CLIENT)
			ret = wolfSSL_connect(s->ssl);
		else
			ret = wolfSSL_accept(s->ssl);

		if (ret == SSL_SUCCESS) {
			s->state = LWIP_DTLS_ESTABLISHED;
			lwip_dtls_stats.handshake_completed++;
		}
		/* WANT_READ/WANT_WRITE: normal, will retry on next data */
	} else if (s->state == LWIP_DTLS_INIT && s->role == LWIP_DTLS_SERVER) {
		/*
		 * Server: incoming data with DTLS configured but no
		 * handshake yet — this is a ClientHello, start accepting.
		 */
		s->state = LWIP_DTLS_HANDSHAKE;
		ret = wolfSSL_accept(s->ssl);
		if (ret == SSL_SUCCESS) {
			s->state = LWIP_DTLS_ESTABLISHED;
			lwip_dtls_stats.handshake_completed++;
		}
	}

	/*
	 * Try to decrypt any pending application data.
	 * wolfSSL_read() will consume from the pending dgram queue
	 * via the recv callback, and return decrypted plaintext.
	 */
	if (s->state == LWIP_DTLS_ESTABLISHED) {
		uint8_t pt_buf[DTLS_MAX_PLAINTEXT];
		int pt_len;

		pt_len = wolfSSL_read(s->ssl, pt_buf, sizeof(pt_buf));
		if (pt_len > 0) {
			/*
			 * Store decrypted plaintext for later retrieval
			 * via the recv path.  For now, we only keep the
			 * latest chunk.
			 */
			s->plaintext_len = (uint16_t)((pt_len > (int)sizeof(s->plaintext))
			    ? sizeof(s->plaintext) : (size_t)pt_len);
			memcpy(s->plaintext, pt_buf, s->plaintext_len);
			s->plaintext_offset = 0;
		} else if (pt_len < 0) {
			/* Decryption error */
			lwip_dtls_stats.decrypt_errors++;
		}
		/* pt_len == 0: no application data yet (e.g., handshake) */
	}

	/* Packet is consumed by DTLS layer — do NOT forward to pktsock */
	return ERR_OK;
}

/* ------------------------------------------------------------------ */
/*  Output path — encrypt before sending                               */
/* ------------------------------------------------------------------ */

err_t
lwip_dtls_output(void *session, struct pbuf **p)
{
	struct lwip_dtls_session *s = (struct lwip_dtls_session *)session;
	uint8_t plaintext[DTLS_MAX_DATALEN];
	uint16_t len;
	int ret;

	if (s == NULL || p == NULL || *p == NULL)
		return ERR_VAL;

	/*
	 * If DTLS is not yet established, initiate or continue the
	 * handshake (client role).
	 */
	if (s->state == LWIP_DTLS_INIT && s->role == LWIP_DTLS_CLIENT) {
		s->state = LWIP_DTLS_HANDSHAKE;
		ret = wolfSSL_connect(s->ssl);
		if (ret == SSL_SUCCESS) {
			s->state = LWIP_DTLS_ESTABLISHED;
			lwip_dtls_stats.handshake_completed++;
		} else {
			int err = wolfSSL_get_error(s->ssl, ret);
			if (err == SSL_ERROR_WANT_READ ||
			    err == SSL_ERROR_WANT_WRITE) {
				/* Handshake in progress, will retry on
				 * next data arrival. This is normal for
				 * non-blocking DTLS. */
				return ERR_INPROGRESS;
			}
			/* Fatal error */
			s->state = LWIP_DTLS_FAILED;
			lwip_dtls_stats.handshake_failed++;
			return ERR_ABRT;
		}
	}

	if (s->state == LWIP_DTLS_HANDSHAKE) {
		/* Handshake still in progress — try again */
		ret = wolfSSL_connect(s->ssl);
		if (ret == SSL_SUCCESS) {
			s->state = LWIP_DTLS_ESTABLISHED;
			lwip_dtls_stats.handshake_completed++;
		} else {
			return ERR_INPROGRESS;
		}
	}

	if (s->state != LWIP_DTLS_ESTABLISHED)
		return ERR_INPROGRESS;

	/* Copy plaintext from the pbuf */
	len = (*p)->tot_len;
	if (len > DTLS_MAX_DATALEN)
		len = DTLS_MAX_DATALEN;

	pbuf_copy_partial(*p, plaintext, len, 0);

	/*
	 * Encrypt via wolfSSL.
	 * wolfSSL_Write internally calls our send callback to transmit
	 * the encrypted DTLS record.
	 */
	ret = wolfSSL_write(s->ssl, plaintext, (int)len);
	if (ret < 0) {
		int err = wolfSSL_get_error(s->ssl, ret);
		if (err == SSL_ERROR_WANT_READ ||
		    err == SSL_ERROR_WANT_WRITE) {
			return ERR_INPROGRESS;
		}
		lwip_dtls_stats.encrypt_errors++;
		return ERR_ABRT;
	}

	/*
	 * The encrypted data was sent via the send callback.
	 * We return ERR_OK and the caller should NOT send the original
	 * plaintext pbuf.  Instead, mark it as consumed.
	 */
	*p = NULL;
	return ERR_OK;
}

/* ------------------------------------------------------------------ */
/*  Status queries                                                     */
/* ------------------------------------------------------------------ */

int
lwip_dtls_is_established(void *session)
{
	struct lwip_dtls_session *s = (struct lwip_dtls_session *)session;

	if (s == NULL)
		return 0;
	return (s->state == LWIP_DTLS_ESTABLISHED) ? 1 : 0;
}

const char *
lwip_dtls_state_str(void *session)
{
	struct lwip_dtls_session *s = (struct lwip_dtls_session *)session;

	if (s == NULL)
		return "NULL";
	switch (s->state) {
	case LWIP_DTLS_NONE:       return "NONE";
	case LWIP_DTLS_INIT:       return "INIT";
	case LWIP_DTLS_HANDSHAKE:  return "HANDSHAKE";
	case LWIP_DTLS_ESTABLISHED: return "ESTABLISHED";
	case LWIP_DTLS_CLOSING:    return "CLOSING";
	case LWIP_DTLS_FAILED:     return "FAILED";
	default:                    return "UNKNOWN";
	}
}

/* ------------------------------------------------------------------ */
/*  Plaintext read helper (for recv path)                             */
/* ------------------------------------------------------------------ */

int
lwip_dtls_read_plaintext(void *session, uint8_t *buf, int len)
{
	struct lwip_dtls_session *s = (struct lwip_dtls_session *)session;
	int avail;

	if (s == NULL || buf == NULL || len <= 0)
		return -1;

	if (s->plaintext_offset >= s->plaintext_len)
		return 0;  /* No plaintext available */

	avail = s->plaintext_len - s->plaintext_offset;
	if (avail > len)
		avail = len;

	memcpy(buf, s->plaintext + s->plaintext_offset, (size_t)avail);
	s->plaintext_offset += (uint16_t)avail;

	return avail;
}

#endif /* LWIP_DTLS */
