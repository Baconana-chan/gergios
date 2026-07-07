/*
 * MINIX 3 specific TCP MD5 signature support (RFC 2385) for BGP peering.
 */
#ifndef LWIP_LWIP_TCP_MD5_H
#define LWIP_LWIP_TCP_MD5_H

#include "lwip/opt.h"

#if LWIP_TCP_MD5SIG /* don't build if not configured */

#include "lwip/ip.h"
#include "lwip/tcp.h"
#include "lwip/err.h"

/*
 * Maximum length of the MD5 password/key, matching NetBSD's TCP_MD5SIG_MAXKEYLEN.
 */
#define TCP_MD5SIG_MAXKEYLEN  80

/*
 * TCP MD5 signature option constants (RFC 2385).
 * Kind=19, Length=18 (kind+len+16-byte digest).
 */
#define TCP_MD5SIG_OPT_KIND   19
#define TCP_MD5SIG_OPT_LEN    18
#define TCP_MD5SIG_DIGEST_LEN 16

/*
 * Structure for TCP_MD5SIG socket option, compatible with NetBSD.
 * The application fills in tcpm_addr (the remote peer address) and
 * tcpm_key/tcpm_keylen (the shared secret).
 *
 * For a listening socket, set tcpm_addr to the wildcard address
 * (all zeros) to match any peer, or to a specific address to
 * restrict the key to that peer only.
 */
struct tcp_md5sig {
	struct sockaddr_storage tcpm_addr;	/* peer address */
	uint8_t  tcpm_key[TCP_MD5SIG_MAXKEYLEN]; /* shared secret */
	uint16_t tcpm_keylen;			/* length of key */
	uint16_t tcpm_pad;			/* padding */
};

/*
 * Per-connection key information stored in tcp_ext_arg.
 */
struct tcp_md5_key {
	ip_addr_t peer_addr;			/* remote peer address (0=wildcard) */
	uint8_t  key[TCP_MD5SIG_MAXKEYLEN];	/* shared secret */
	uint16_t keylen;			/* length of key */
};

/*
 * Runtime enable/disable toggle.  Set to 1 to enable MD5 signature
 * processing (default).  When disabled, MD5 options are not added to
 * outgoing segments and incoming MD5 options are not validated.
 * This variable is exposed via sysctl as net.inet.tcp.md5sig.
 */
extern int lwip_tcp_md5_enabled;

/* Public API */

/*
 * Initialise the TCP MD5 signature module.  Must be called after lwip_init()
 * and before any TCP connections are established.
 */
void lwip_tcp_md5_init(void);

/*
 * Set the MD5 key for the given PCB.  This function is called from the
 * setsockopt handler and from the passive-open callback.
 */
err_t lwip_tcp_md5_set_key(struct tcp_pcb *pcb, const struct tcp_md5sig *md5sig);

/*
 * Retrieve the MD5 key for the given PCB, if any.
 * Returns ERR_OK if a key is found, ERR_ARG if not.
 */
err_t lwip_tcp_md5_get_key(const struct tcp_pcb *pcb, struct tcp_md5sig *md5sig);

/*
 * Remove the MD5 key for the given PCB.
 */
void lwip_tcp_md5_clear_key(struct tcp_pcb *pcb);

/*
 * Check whether the given PCB has an MD5 key configured.
 */
int lwip_tcp_md5_has_key(const struct tcp_pcb *pcb);

/* Hook functions -- called from lwIP core via lwiphooks.h */

/*
 * Reserve space for the MD5 option in outgoing TCP segments.
 * Called from tcp_out.c via LWIP_HOOK_TCP_OUT_TCPOPT_LENGTH.
 */
u8_t lwip_tcp_md5_out_tcpopt_length(const struct tcp_pcb *pcb,
	u8_t internal_option_length);

/*
 * Write the MD5 option into an outgoing TCP segment.
 * Called from tcp_out.c via LWIP_HOOK_TCP_OUT_ADD_TCPOPTS.
 */
u32_t *lwip_tcp_md5_add_tcpopts(struct pbuf *p, struct tcp_hdr *hdr,
	const struct tcp_pcb *pcb, u32_t *opts);

/*
 * Validate the MD5 option on an incoming TCP segment.
 * Called from tcp_in.c via LWIP_HOOK_TCP_INPACKET_PCB.
 */
err_t lwip_tcp_md5_inpacket(struct tcp_pcb *pcb, struct tcp_hdr *hdr,
	u16_t optlen, u16_t opt1len, u8_t *opt2, struct pbuf *p);

/* Stubs for LWIP_TCP_MD5SIG==0 */
#else /* LWIP_TCP_MD5SIG */

#define lwip_tcp_md5_enabled              0
#define lwip_tcp_md5_init()
#define lwip_tcp_md5_set_key(pcb, md5sig)  ERR_ARG
#define lwip_tcp_md5_get_key(pcb, md5sig)  ERR_ARG
#define lwip_tcp_md5_clear_key(pcb)
#define lwip_tcp_md5_has_key(pcb)          0

#define lwip_tcp_md5_out_tcpopt_length(pcb, len)  (len)
#define lwip_tcp_md5_add_tcpopts(p, hdr, pcb, opts)  (opts)
#define lwip_tcp_md5_inpacket(pcb, hdr, optlen, opt1len, opt2, p)  ERR_OK

#endif /* LWIP_TCP_MD5SIG */

#endif /* !LWIP_LWIP_TCP_MD5_H */
