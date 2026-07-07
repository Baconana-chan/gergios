/* LWIP service - bpf_any.h - BPF "any" capture interface definitions */
/*
 * Implementation of global ("any") packet capture, analogous to Linux's
 * tcpdump -i any.  A BPF device bound to "any" receives packets from ALL
 * network interfaces, with a cooked header prepended that identifies the
 * source interface and packet type.
 *
 * Cooked header format (struct bpf_any_hdr, 20 bytes):
 *   uint16_t baf_ifindex    - interface index (host order)
 *   uint16_t baf_pkttype    - packet type (PACKET_* value)
 *   uint16_t baf_hatype     - ARPHRD_ hardware type (host order)
 *   uint16_t baf_halen      - link-layer address length
 *   uint8_t  baf_addr[8]    - link-layer source address (or zeros)
 *   uint16_t baf_protocol   - Ethernet protocol type (network order)
 *
 * DLT: DLT_MINIX_ANY (based on DLT_USER0).
 */

#ifndef MINIX_NET_LWIP_BPF_ANY_H
#define MINIX_NET_LWIP_BPF_ANY_H

#include <net/dlt.h>

/*
 * DLT value for MINIX "any" captures.  Uses DLT_USER0 (147) which is reserved
 * for private use and will not conflict with standard link-layer types.
 */
#define DLT_MINIX_ANY		DLT_USER0

/*
 * Packet type values for bpf_any_hdr.baf_pkttype.  These follow the Linux
 * PACKET_* convention for compatibility with libpcap/tcpdump semantics.
 */
#define PACKET_HOST		0	/* To us */
#define PACKET_BROADCAST	1	/* To all */
#define PACKET_MULTICAST	2	/* To group */
#define PACKET_OTHERHOST	3	/* To someone else */
#define PACKET_OUTGOING		4	/* Outgoing from us */

/*
 * Cooked header prepended to packets delivered to "any" BPF listeners.
 * Total size: 20 bytes, followed by the original packet data including
 * the link-layer header.
 */
struct bpf_any_hdr {
	uint16_t baf_ifindex;		/* Interface index (host order) */
	uint16_t baf_pkttype;		/* Packet type (PACKET_*) */
	uint16_t baf_hatype;		/* Hardware type (host order) */
	uint16_t baf_halen;		/* Link-layer address length */
	uint8_t  baf_addr[8];		/* Link-layer source address */
	uint16_t baf_protocol;		/* Ethernet protocol (net order) */
} __packed;

/* Fixed size of the cooked header. */
#define BPF_ANY_HDRLEN		sizeof(struct bpf_any_hdr)

/*
 * Deliver a packet to all "any" BPF listeners.  Called from ifdev_input()
 * and ifdev_output() for every packet crossing the network stack.
 */
void ifdev_deliver_to_any(const struct ifdev * ifdev, const struct pbuf * pbuf,
    int pkttype);

#endif /* !MINIX_NET_LWIP_BPF_ANY_H */
