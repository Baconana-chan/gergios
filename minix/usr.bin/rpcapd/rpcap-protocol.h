/*
 * rpcap-protocol.h — Remote Packet Capture Protocol definitions.
 *
 * Adapted from libpcap's rpcap-protocol.h (BSD-3-Clause licensed).
 * Copyright (c) 2002-2005 NetGroup, Politecnico di Torino (Italy)
 * Copyright (c) 2005-2008 CACE Technologies, Davis (California)
 *
 * WARNING: All structures in this file MUST NOT have padding.
 * They are designed for 32-bit alignment — use __packed if available.
 */

#ifndef RPCAP_PROTOCOL_H
#define RPCAP_PROTOCOL_H

#include <sys/cdefs.h>
#include <stdint.h>

/* ── Ports ──────────────────────────────────────────────────────── */
#define RPCAP_DEFAULT_NETPORT      "2002"
#define RPCAP_DEFAULT_NETPORT_ACTIVE "2003"

/* ── Version ─────────────────────────────────────────────────────── */
#define RPCAP_MIN_VERSION  0
#define RPCAP_MAX_VERSION  0

/* ── Message types ──────────────────────────────────────────────── */
#define RPCAP_MSG_IS_REPLY          0x080

#define RPCAP_MSG_ERROR             0x01
#define RPCAP_MSG_FINDALLIF_REQ     0x02
#define RPCAP_MSG_OPEN_REQ          0x03
#define RPCAP_MSG_STARTCAP_REQ      0x04
#define RPCAP_MSG_UPDATEFILTER_REQ  0x05
#define RPCAP_MSG_CLOSE             0x06
#define RPCAP_MSG_PACKET            0x07
#define RPCAP_MSG_AUTH_REQ          0x08
#define RPCAP_MSG_STATS_REQ         0x09
#define RPCAP_MSG_ENDCAP_REQ        0x0A
#define RPCAP_MSG_SETSAMPLING_REQ   0x0B

#define RPCAP_MSG_FINDALLIF_REPLY   (RPCAP_MSG_FINDALLIF_REQ | RPCAP_MSG_IS_REPLY)
#define RPCAP_MSG_OPEN_REPLY        (RPCAP_MSG_OPEN_REQ | RPCAP_MSG_IS_REPLY)
#define RPCAP_MSG_STARTCAP_REPLY    (RPCAP_MSG_STARTCAP_REQ | RPCAP_MSG_IS_REPLY)
#define RPCAP_MSG_UPDATEFILTER_REPLY (RPCAP_MSG_UPDATEFILTER_REQ | RPCAP_MSG_IS_REPLY)
#define RPCAP_MSG_AUTH_REPLY        (RPCAP_MSG_AUTH_REQ | RPCAP_MSG_IS_REPLY)
#define RPCAP_MSG_STATS_REPLY       (RPCAP_MSG_STATS_REQ | RPCAP_MSG_IS_REPLY)
#define RPCAP_MSG_ENDCAP_REPLY      (RPCAP_MSG_ENDCAP_REQ | RPCAP_MSG_IS_REPLY)
#define RPCAP_MSG_SETSAMPLING_REPLY (RPCAP_MSG_SETSAMPLING_REQ | RPCAP_MSG_IS_REPLY)

/* ── Authentication types ───────────────────────────────────────── */
#define RPCAP_AUTH_NULL     0
#define RPCAP_AUTH_PWD      1

/* ── STARTCAP flags ──────────────────────────────────────────────── */
#define RPCAP_STARTCAPREQ_FLAG_PROMISC    0x0001
#define RPCAP_STARTCAPREQ_FLAG_DGRAM      0x0002
#define RPCAP_STARTCAPREQ_FLAG_SERVEROPEN 0x0004
#define RPCAP_STARTCAPREQ_FLAG_INBOUND    0x0008
#define RPCAP_STARTCAPREQ_FLAG_OUTBOUND   0x0010

/* ── Filter type ──────────────────────────────────────────────────── */
#define RPCAP_UPDATEFILTER_BPF  1

/* ── Network buffer size ──────────────────────────────────────────── */
#define RPCAP_NETBUF_SIZE  64000

/* ── Byte order magic ──────────────────────────────────────────────── */
#define RPCAP_BYTE_ORDER_MAGIC       0xa1b2c3d4U
#define RPCAP_BYTE_ORDER_MAGIC_SWAPPED 0xd4c3b2a1U

/* ── Address families (wire format) ────────────────────────────────── */
#define RPCAP_AF_INET    2    /* matches AF_INET on all OSes except Haiku */
#define RPCAP_AF_INET6   23   /* Windows value for AF_INET6 */

/* ── Error codes (PCAP_ERR_*) ──────────────────────────────────────── */
#define PCAP_ERR_NETW            1
#define PCAP_ERR_INITTIMEOUT     2
#define PCAP_ERR_AUTH            3
#define PCAP_ERR_FINDALLIF       4
#define PCAP_ERR_NOREMOTEIF      5
#define PCAP_ERR_OPEN            6
#define PCAP_ERR_UPDATEFILTER    7
#define PCAP_ERR_GETSTATS        8
#define PCAP_ERR_READEX          9
#define PCAP_ERR_HOSTNOAUTH      10
#define PCAP_ERR_REMOTEACCEPT    11
#define PCAP_ERR_STARTCAPTURE    12
#define PCAP_ERR_ENDCAPTURE      13
#define PCAP_ERR_RUNTIMETIMEOUT  14
#define PCAP_ERR_SETSAMPLING     15
#define PCAP_ERR_WRONGMSG        16
#define PCAP_ERR_WRONGVER        17
#define PCAP_ERR_AUTH_FAILED     18
#define PCAP_ERR_TLS_REQUIRED    19
#define PCAP_ERR_AUTH_TYPE_NOTSUP 20

/* ── Interface flags (for rpcap_findalldevs_if.flags) ────────────── */
#define RPCAP_IF_LOOPBACK   0x00000001
#define RPCAP_IF_UP         0x00000002
#define RPCAP_IF_RUNNING    0x00000004

/* ════════════════════════════════════════════════════════════════════
 * Wire format structures
 *
 * All multi-byte fields are in NETWORK BYTE ORDER (big-endian).
 * ════════════════════════════════════════════════════════════════════ */

/* ── Common header for all RPCAP messages (8 bytes) ──────────────── */
struct rpcap_header {
	uint8_t  ver;		/* protocol version */
	uint8_t  type;		/* message type (RPCAP_MSG_*) */
	uint16_t value;		/* message-dependent value */
	uint32_t plen;		/* payload length (following this header) */
} __packed;

/* ── Authentication reply (version negotiation) ──────────────────── */
struct rpcap_authreply {
	uint8_t  minvers;
	uint8_t  maxvers;
	uint8_t  pad[2];
	uint32_t byte_order_magic;
} __packed;

/* ── FINDALLIF_REPLY: per-interface descriptor ───────────────────── */
struct rpcap_findalldevs_if {
	uint16_t namelen;	/* length of interface name */
	uint16_t desclen;	/* length of interface description */
	uint32_t flags;		/* interface flags */
	uint16_t naddr;		/* number of addresses */
	uint16_t dummy;		/* must be zero */
} __packed;

/* ── FINDALLIF_REPLY: wire-format sockaddr (128 bytes) ───────────── */
struct rpcap_sockaddr {
	uint16_t family;
	char     data[128 - 2];
} __packed;

/* ── FINDALLIF_REPLY: wire-format sockaddr_in ────────────────────── */
struct rpcap_sockaddr_in {
	uint16_t family;
	uint16_t port;
	uint32_t addr;
	uint8_t  zero[8];
} __packed;

/* ── FINDALLIF_REPLY: wire-format sockaddr_in6 ───────────────────── */
struct rpcap_sockaddr_in6 {
	uint16_t family;
	uint16_t port;
	uint32_t flowinfo;
	uint8_t  addr[16];
	uint32_t scope_id;
} __packed;

/* ── FINDALLIF_REPLY: per-address descriptor ─────────────────────── */
struct rpcap_findalldevs_ifaddr {
	struct rpcap_sockaddr addr;
	struct rpcap_sockaddr netmask;
	struct rpcap_sockaddr broadaddr;
	struct rpcap_sockaddr dstaddr;
} __packed;

/* ── OPEN_REPLY ────────────────────────────────────────────────────── */
struct rpcap_openreply {
	int32_t linktype;
	int32_t tzoff;
} __packed;

/* ── STARTCAP_REQ ──────────────────────────────────────────────────── */
struct rpcap_startcapreq {
	uint32_t snaplen;
	uint32_t read_timeout;
	uint16_t flags;
	uint16_t portdata;
} __packed;

/* ── STARTCAP_REPLY ────────────────────────────────────────────────── */
struct rpcap_startcapreply {
	int32_t  bufsize;
	uint16_t portdata;
	uint16_t dummy;
} __packed;

/* ── PACKET: per-packet header ─────────────────────────────────────── */
struct rpcap_pkthdr {
	uint32_t timestamp_sec;
	uint32_t timestamp_usec;
	uint32_t caplen;
	uint32_t len;
	uint32_t npkt;
} __packed;

/* ── AUTH: authentication data ─────────────────────────────────────── */
struct rpcap_auth {
	uint16_t type;		/* RPCAP_AUTH_NULL or RPCAP_AUTH_PWD */
	uint16_t dummy;		/* must be zero */
	uint16_t slen1;		/* length of first auth item (username) */
	uint16_t slen2;		/* length of second auth item (password) */
} __packed;

/* ── FILTER: BPF filter description ────────────────────────────────── */
struct rpcap_filter {
	uint16_t filtertype;	/* RPCAP_UPDATEFILTER_BPF */
	uint16_t dummy;
	uint32_t nitems;
} __packed;

/* ── FILTER BPF instruction ──────────────────────────────────────────── */
struct rpcap_filterbpf_insn {
	uint16_t code;
	uint8_t  jt;
	uint8_t  jf;
	int32_t  k;
} __packed;

/* ── STATS reply ────────────────────────────────────────────────────── */
struct rpcap_stats {
	uint32_t ifrecv;
	uint32_t ifdrop;
	uint32_t krnldrop;
	uint32_t svrcapt;
} __packed;

/* ── SAMPLING ───────────────────────────────────────────────────────── */
struct rpcap_sampling {
	uint8_t  method;
	uint8_t  dummy1;
	uint16_t dummy2;
	uint32_t value;
} __packed;

#endif /* RPCAP_PROTOCOL_H */
