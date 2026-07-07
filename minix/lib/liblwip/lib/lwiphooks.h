/*
 * MINIX 3 specific hooks for lwIP.
 */
#ifndef LWIP_LWIPHOOKS_H
#define LWIP_LWIPHOOKS_H

/* TCP ISN hook. */
u32_t lwip_hook_tcp_isn(const ip_addr_t * local_ip, u16_t local_port,
	const ip_addr_t * remote_ip, u16_t remote_port);

#define LWIP_HOOK_TCP_ISN lwip_hook_tcp_isn

/*
 * IPv4 route hook.  Since we override the IPv4 routing function altogether,
 * this hook should not be called and will panic if it is called, because that
 * is an indication that something is seriously wrong.  Note that we do not use
 * the IPv4 source route hook, because that one would be called (needlessly).
 */
struct netif *lwip_hook_ip4_route(const ip4_addr_t * dst);

#define LWIP_HOOK_IP4_ROUTE lwip_hook_ip4_route

/* IPv4 gateway hook. */
const ip4_addr_t *lwip_hook_etharp_get_gw(struct netif * netif,
	const ip4_addr_t * ipaddr);

#define LWIP_HOOK_ETHARP_GET_GW lwip_hook_etharp_get_gw

/*
 * IPv6 route hook.  Since we override the IPv6 routing function altogether,
 * this hook should not be called and will panic if it is called, because that
 * is an indication that something is seriously wrong.
 */
struct netif *lwip_hook_ip6_route(const ip6_addr_t * dst,
	const ip6_addr_t * src);

#define LWIP_HOOK_IP6_ROUTE lwip_hook_ip6_route

/* IPv6 gateway (next-hop) hook. */
const ip6_addr_t *lwip_hook_nd6_get_gw(struct netif * netif,
	const ip6_addr_t * ipaddr);

#define LWIP_HOOK_ND6_GET_GW lwip_hook_nd6_get_gw

/*
 * TCP MD5 signature hooks (RFC 2385).  These hooks intercept outgoing and
 * incoming TCP segments to add/validate MD5 digest options for BGP peering.
 * They are only operational when LWIP_TCP_MD5SIG is enabled.
 */
#if LWIP_TCP_MD5SIG
u8_t lwip_tcp_md5_out_tcpopt_length(const struct tcp_pcb *pcb,
	u8_t internal_option_length);

#define LWIP_HOOK_TCP_OUT_TCPOPT_LENGTH(pcb, internal_len) \
	lwip_tcp_md5_out_tcpopt_length(pcb, internal_len)

u32_t *lwip_tcp_md5_add_tcpopts(struct pbuf *p, struct tcp_hdr *hdr,
	const struct tcp_pcb *pcb, u32_t *opts);

#define LWIP_HOOK_TCP_OUT_ADD_TCPOPTS(p, hdr, pcb, opts) \
	lwip_tcp_md5_add_tcpopts(p, hdr, pcb, opts)

err_t lwip_tcp_md5_inpacket(struct tcp_pcb *pcb, struct tcp_hdr *hdr,
	u16_t optlen, u16_t opt1len, u8_t *opt2, struct pbuf *p);

#define LWIP_HOOK_TCP_INPACKET_PCB(pcb, hdr, optlen, opt1len, opt2, p) \
	lwip_tcp_md5_inpacket(pcb, hdr, optlen, opt1len, opt2, p)

#endif /* LWIP_TCP_MD5SIG */

/*
 * IPsec ESP/AH hooks (RFC 4301/4302/4303).
 * These hooks intercept incoming and outgoing IPv4 packets to apply
 * IPsec transforms.  They are only operational when LWIP_IPSEC is enabled.
 */
#if LWIP_IPSEC
#include "lwip_ipsec.h"

int lwip_ipsec_input_hook(struct pbuf *p, struct netif *inp);
int lwip_ipsec_output_hook(struct pbuf *p, struct netif *netif,
	const ip4_addr_t *dest);

/*
 * LWIP_HOOK_IP4_INPUT: called early in ip4_input() before any validation.
 * Returns 1 if the packet was eaten (ESP/AH processed), 0 to continue.
 */
#define LWIP_HOOK_IP4_INPUT(p, inp)  lwip_ipsec_input_hook(p, inp)

/*
 * LWIP_HOOK_IP4_OUTPUT: called in ip4_output_if_src() (via patch 0008)
 * after IP header construction but before netif->output().
 * Returns 1 if the packet was transformed and sent, 0 to continue.
 */
#define LWIP_HOOK_IP4_OUTPUT(p, netif, dest) \
	lwip_ipsec_output_hook(p, netif, dest)

#else /* LWIP_IPSEC */
/* Stubs -- no IPsec processing */
#define LWIP_HOOK_IP4_INPUT(p, inp)     (0)
#define LWIP_HOOK_IP4_OUTPUT(p, n, d)   (0)
#endif /* LWIP_IPSEC */

#endif /* !LWIP_LWIPHOOKS_H */
