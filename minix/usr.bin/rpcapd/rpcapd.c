/*
 * rpcapd.c — Remote Packet Capture Protocol daemon.
 *
 * Listens on TCP port 2002 and serves remote capture requests
 * from WireShark / libpcap clients (rpcap://hostname/).
 *
 * Supported protocol flow:
 *   AUTH_REQ -> AUTH_REPLY
 *   FINDALLIF_REQ -> FINDALLIF_REPLY
 *   OPEN_REQ -> OPEN_REPLY
 *   STARTCAP_REQ -> (open BPF) -> STARTCAP_REPLY + PACKET stream
 *   ... (PACKET messages until ENDCAP) ...
 *   ENDCAP_REQ -> ENDCAP_REPLY     (or)
 *   CLOSE
 *
 * Currently supports only RPCAP_AUTH_NULL (no password).
 * For TLS-requiring servers, add RPCAP_AUTH_PWD + SSL later.
 */

#include "rpcap-protocol.h"

#include <sys/types.h>
#include <sys/socket.h>
#include <sys/ioctl.h>
#include <sys/time.h>
#include <sys/select.h>
#include <net/bpf.h>
#include <net/dlt.h>
#include <net/if.h>
#include <ifaddrs.h>
#include <netinet/in.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <signal.h>
#include <arpa/inet.h>

/* ── Constants ──────────────────────────────────────────────────── */

#define BPF_BUF_SIZE		32768
#define MAX_IFACES		64
#define MAX_ADDRS_PER_IFACE	16
#define DEFAULT_SNAPLEN		65535
#define RPCAPD_BACKLOG		5

/* ── Authentication credentials (set via -u and -P) ──────────── */
static const char *g_username = NULL;	/* required username, or NULL for any */
static const char *g_password = NULL;	/* required password, or NULL for any */

/* ── Global state for signal handling ──────────────────────────── */
static volatile int g_stop = 0;

static void
sigint_handler(int sig __unused)
{
	g_stop = 1;
}

/* ═══════════════════════════════════════════════════════════════════
 * Wire helpers
 * ═══════════════════════════════════════════════════════════════════ */

/*
 * Send len bytes from buf over fd, looping to handle partial writes.
 * Returns 0 on success, -1 on error.
 */
static int
sock_send(int fd, const void *buf, size_t len)
{
	const char *p = (const char *)buf;
	size_t remain = len;

	while (remain > 0) {
		ssize_t n = write(fd, p, remain);
		if (n < 0) {
			if (errno == EINTR)
				continue;
			return -1;
		}
		p += n;
		remain -= (size_t)n;
	}
	return 0;
}

/*
 * Read exactly len bytes from fd into buf.
 * Returns 0 on success, -1 on error or EOF.
 */
static int
sock_recv(int fd, void *buf, size_t len)
{
	char *p = (char *)buf;
	size_t remain = len;

	while (remain > 0) {
		ssize_t n = read(fd, p, remain);
		if (n <= 0) {
			if (n == 0)
				errno = ECONNRESET;
			else if (errno == EINTR)
				continue;
			return -1;
		}
		p += n;
		remain -= (size_t)n;
	}
	return 0;
}

/*
 * Send an rpcap header + payload (network byte order).
 * The header fields are already in network byte order when passed in.
 */
static int
send_msg(int fd, const struct rpcap_header *hdr, const void *payload)
{
	if (sock_send(fd, hdr, sizeof(*hdr)) != 0)
		return -1;
	if (payload != NULL && ntohl(hdr->plen) > 0) {
		if (sock_send(fd, payload, ntohl(hdr->plen)) != 0)
			return -1;
	}
	return 0;
}

/*
 * Send an rpcap error message.
 */
static int
send_error(int fd, uint8_t ver, uint16_t errcode, const char *msg)
{
	struct rpcap_header hdr;
	size_t msglen = strlen(msg);
	hdr.ver = ver;
	hdr.type = RPCAP_MSG_ERROR;
	hdr.value = htons(errcode);
	hdr.plen = htonl((uint32_t)msglen);

	if (sock_send(fd, &hdr, sizeof(hdr)) != 0)
		return -1;
	if (msglen > 0) {
		if (sock_send(fd, msg, msglen) != 0)
			return -1;
	}
	return 0;
}

/*
 * Receive an rpcap header + payload.
 * Returns 0 on success, -1 on error.
 * payload must be freed by caller if plen > 0.
 */
static int
recv_msg(int fd, struct rpcap_header *hdr, void **payload)
{
	if (sock_recv(fd, hdr, sizeof(*hdr)) != 0)
		return -1;

	uint32_t plen = ntohl(hdr->plen);
	if (plen > 0) {
		*payload = malloc(plen);
		if (*payload == NULL)
			return -1;
		if (sock_recv(fd, *payload, plen) != 0) {
			free(*payload);
			*payload = NULL;
			return -1;
		}
	} else {
		*payload = NULL;
	}
	return 0;
}

/* ═══════════════════════════════════════════════════════════════════
 * Protocol handlers
 * ═══════════════════════════════════════════════════════════════════ */

/*
 * Parse username and password from an RPCAP_AUTH_PWD payload.
 *
 * Payload layout:
 *   struct rpcap_auth  (8 bytes)
 *   username           (slen1 bytes, NOT null-terminated)
 *   password           (slen2 bytes, NOT null-terminated)
 *
 * Returns 0 on success, -1 on auth failure.
 */
static int
auth_check_pwd(struct rpcap_auth *auth, const uint8_t *payload,
    size_t plen)
{
	uint16_t slen1 = ntohs(auth->slen1);
	uint16_t slen2 = ntohs(auth->slen2);
	const uint8_t *data = (const uint8_t *)(auth + 1);

	/* Bounds check: username + password must fit in payload */
	size_t total = sizeof(*auth) + slen1 + slen2;
	if (total > plen)
		return -1;

	/* Compare username if a specific one is configured */
	if (g_username != NULL) {
		if (strlen(g_username) != (size_t)slen1)
			return -1;
		if (memcmp(g_username, data, slen1) != 0)
			return -1;
	}
	data += slen1;

	/* Compare password if a specific one is configured */
	if (g_password != NULL) {
		if (strlen(g_password) != (size_t)slen2)
			return -1;
		if (memcmp(g_password, data, slen2) != 0)
			return -1;
	}

	return 0;
}

/*
 * Handle AUTH_REQ.
 *
 * Supports two authentication modes:
 *   1) RPCAP_AUTH_NULL (type 0) — no credentials needed
 *   2) RPCAP_AUTH_PWD  (type 1) — username/password check
 *
 * If no -u/-P flags are given, NULL auth is accepted; if credentials
 * ARE configured (via -u/-P), the client MUST use RPCAP_AUTH_PWD.
 *
 * Reply with a version negotiation payload so the client knows
 * our protocol version support.
 */
static int
handle_auth(int fd, struct rpcap_header *req, void *payload)
{
	struct rpcap_header reply;
	struct rpcap_authreply authreply;
	uint32_t plen = (payload != NULL) ? ntohl(req->plen) : 0;

	if (payload != NULL && plen >= sizeof(struct rpcap_auth)) {
		struct rpcap_auth *auth = (struct rpcap_auth *)payload;
		uint16_t atype = ntohs(auth->type);

		switch (atype) {
		case RPCAP_AUTH_NULL:
			/* NULL auth is only accepted if no credentials configured */
			if (g_username != NULL || g_password != NULL) {
				send_error(fd, req->ver,
				    PCAP_ERR_AUTH,
				    "Password authentication required");
				return -1;
			}
			break;

		case RPCAP_AUTH_PWD:
			/* Verify username/password against configured credentials */
			if (auth_check_pwd(auth, (const uint8_t *)payload,
			    plen) != 0) {
				send_error(fd, req->ver,
				    PCAP_ERR_AUTH_FAILED,
				    "Authentication failed");
				return -1;
			}
			break;

		default:
			send_error(fd, req->ver,
			    PCAP_ERR_AUTH_TYPE_NOTSUP,
			    "Unsupported authentication type");
			return -1;
		}
	} else {
		/* No auth payload at all — only allowed when no credentials set */
		if (g_username != NULL || g_password != NULL) {
			send_error(fd, req->ver,
			    PCAP_ERR_AUTH,
			    "Authentication required");
			return -1;
		}
	}

	/* Build AUTH_REPLY with version negotiation */
	authreply.minvers = RPCAP_MIN_VERSION;
	authreply.maxvers = RPCAP_MAX_VERSION;
	authreply.pad[0] = 0;
	authreply.pad[1] = 0;
	authreply.byte_order_magic = htonl(RPCAP_BYTE_ORDER_MAGIC);

	reply.ver = RPCAP_MAX_VERSION;
	reply.type = RPCAP_MSG_AUTH_REPLY;
	reply.value = 0;
	reply.plen = htonl(sizeof(authreply));

	if (send_msg(fd, &reply, &authreply) != 0)
		return -1;

	return 0;
}

/*
 * Build a FINDALLIF_REPLY payload for all available interfaces.
 * Uses getifaddrs() to enumerate.
 *
 * Returns 0 on success, -1 on error.
 */
static int
handle_findallif(int fd, struct rpcap_header *req)
{
	struct rpcap_header reply;
	struct ifaddrs *ifaddr, *ifa;
	int raw_sock = -1;

	/* Open a raw socket to get interface flags via ioctl */
	raw_sock = socket(AF_INET, SOCK_DGRAM, 0);
	if (raw_sock < 0) {
		send_error(fd, req->ver, PCAP_ERR_FINDALLIF,
		    "Cannot open control socket");
		return -1;
	}

	if (getifaddrs(&ifaddr) != 0) {
		close(raw_sock);
		send_error(fd, req->ver, PCAP_ERR_FINDALLIF,
		    "Cannot enumerate interfaces");
		return -1;
	}

	/*
	 * First pass: build a list of unique interfaces and their addresses.
	 * We allocate a buffer and build the full reply payload.
	 */
	struct rpcap_findalldevs_if ifdesc;
	uint8_t reply_buf[RPCAP_NETBUF_SIZE];
	size_t off = 0;
	const char *seen_ifaces[MAX_IFACES];
	unsigned int nseen = 0;

	for (ifa = ifaddr; ifa != NULL; ifa = ifa->ifa_next) {
		if (ifa->ifa_name == NULL)
			continue;

		/* Skip duplicate names — check against collected set */
		unsigned int i;
		int seen = 0;
		for (i = 0; i < nseen; i++) {
			if (strcmp(ifa->ifa_name, seen_ifaces[i]) == 0) {
				seen = 1;
				break;
			}
		}
		if (seen)
			continue;

		seen_ifaces[nseen++] = ifa->ifa_name;
		if (nseen >= MAX_IFACES)
			break;

		/* Get interface flags */
		struct ifreq ifr;
		memset(&ifr, 0, sizeof(ifr));
		strlcpy(ifr.ifr_name, ifa->ifa_name, sizeof(ifr.ifr_name));
		unsigned int ifflags = ifa->ifa_flags;

		if (ioctl(raw_sock, SIOCGIFFLAGS, &ifr) == 0)
			ifflags = (unsigned int)ifr.ifr_flags;

		/* Translate flags */
		uint32_t rpcap_flags = 0;
		if (ifflags & IFF_LOOPBACK)
			rpcap_flags |= RPCAP_IF_LOOPBACK;
		if (ifflags & IFF_UP)
			rpcap_flags |= RPCAP_IF_UP;
		if (ifflags & IFF_RUNNING)
			rpcap_flags |= RPCAP_IF_RUNNING;

		/* Count addresses */
		uint16_t naddr = 0;
		struct ifaddrs *ifa2;
		for (ifa2 = ifaddr; ifa2 != NULL; ifa2 = ifa2->ifa_next) {
			if (ifa2->ifa_name != NULL &&
			    strcmp(ifa2->ifa_name, ifa->ifa_name) == 0 &&
			    ifa2->ifa_addr != NULL &&
			    (ifa2->ifa_addr->sa_family == AF_INET ||
			     ifa2->ifa_addr->sa_family == AF_INET6)) {
				naddr++;
			}
		}

		size_t namelen = strlen(ifa->ifa_name) + 1;
		const char *desc = ifa->ifa_name;
		size_t desclen = strlen(desc) + 1;

		/* Calculate entry size */
		size_t if_entry_size = sizeof(ifdesc) + namelen + desclen +
		    (size_t)naddr * sizeof(struct rpcap_findalldevs_ifaddr);

		if (off + if_entry_size > sizeof(reply_buf)) {
			/* Buffer full — stop adding interfaces */
			break;
		}

		/* Write interface descriptor */
		ifdesc.namelen = htons((uint16_t)namelen);
		ifdesc.desclen = htons((uint16_t)desclen);
		ifdesc.flags = htonl(rpcap_flags);
		ifdesc.naddr = htons(naddr);
		ifdesc.dummy = 0;

		memcpy(reply_buf + off, &ifdesc, sizeof(ifdesc));
		off += sizeof(ifdesc);

		/* Write interface name */
		memcpy(reply_buf + off, ifa->ifa_name, namelen);
		off += namelen;

		/* Write interface description */
		memcpy(reply_buf + off, desc, desclen);
		off += desclen;

		/* Write addresses */
		for (ifa2 = ifaddr; ifa2 != NULL; ifa2 = ifa2->ifa_next) {
			if (ifa2->ifa_name == NULL ||
			    strcmp(ifa2->ifa_name, ifa->ifa_name) != 0 ||
			    ifa2->ifa_addr == NULL)
				continue;

			struct rpcap_findalldevs_ifaddr ifaddr_wire;

			memset(&ifaddr_wire, 0, sizeof(ifaddr_wire));

			if (ifa2->ifa_addr->sa_family == AF_INET) {
				struct sockaddr_in *sin = (struct sockaddr_in *)ifa2->ifa_addr;
				struct rpcap_sockaddr_in rsin;

				memset(&rsin, 0, sizeof(rsin));
				rsin.family = htons(RPCAP_AF_INET);
				rsin.addr = sin->sin_addr.s_addr;

				memcpy(&ifaddr_wire.addr, &rsin, sizeof(rsin));

				/* Netmask */
				if (ifa2->ifa_netmask != NULL) {
					sin = (struct sockaddr_in *)ifa2->ifa_netmask;
					memset(&rsin, 0, sizeof(rsin));
					rsin.family = htons(RPCAP_AF_INET);
					rsin.addr = sin->sin_addr.s_addr;
					memcpy(&ifaddr_wire.netmask, &rsin, sizeof(rsin));
				}

				/* Broadcast */
				if (ifa2->ifa_broadaddr != NULL) {
					sin = (struct sockaddr_in *)ifa2->ifa_broadaddr;
					memset(&rsin, 0, sizeof(rsin));
					rsin.family = htons(RPCAP_AF_INET);
					rsin.addr = sin->sin_addr.s_addr;
					memcpy(&ifaddr_wire.broadaddr, &rsin, sizeof(rsin));
				}
			} else if (ifa2->ifa_addr->sa_family == AF_INET6) {
				struct sockaddr_in6 *sin6 = (struct sockaddr_in6 *)ifa2->ifa_addr;
				struct rpcap_sockaddr_in6 rsin6;

				memset(&rsin6, 0, sizeof(rsin6));
				rsin6.family = htons(RPCAP_AF_INET6);
				memcpy(rsin6.addr, &sin6->sin6_addr, 16);

				memcpy(&ifaddr_wire.addr, &rsin6, sizeof(rsin6));
			}

			memcpy(reply_buf + off, &ifaddr_wire,
			    sizeof(ifaddr_wire));
			off += sizeof(ifaddr_wire);
		}
	}

	freeifaddrs(ifaddr);
	close(raw_sock);

	if (off == 0) {
		send_error(fd, req->ver, PCAP_ERR_NOREMOTEIF,
		    "No interfaces available");
		return -1;
	}

	/* Send reply */
	reply.ver = RPCAP_MAX_VERSION;
	reply.type = RPCAP_MSG_FINDALLIF_REPLY;
	reply.value = 0;
	reply.plen = htonl((uint32_t)off);

	if (send_msg(fd, &reply, reply_buf) != 0)
		return -1;

	return 0;
}

/*
 * Handle OPEN_REQ.
 * The payload is the interface name. We just validate that it
 * exists and can be opened for capture.
 */
static int
handle_open(int fd, struct rpcap_header *req, void *payload)
{
	struct rpcap_header reply;
	struct rpcap_openreply openreply;
	struct ifaddrs *ifaddr, *ifa;
	int found = 0;

	if (payload == NULL || ntohl(req->plen) == 0) {
		send_error(fd, req->ver, PCAP_ERR_OPEN,
		    "Missing interface name");
		return -1;
	}

	char *ifname = (char *)payload;
	ifname[ntohl(req->plen) - 1] = '\0';  /* ensure null-terminated */

	/* Validate that the interface exists */
	if (getifaddrs(&ifaddr) != 0) {
		send_error(fd, req->ver, PCAP_ERR_OPEN,
		    "Cannot enumerate interfaces");
		return -1;
	}

	for (ifa = ifaddr; ifa != NULL; ifa = ifa->ifa_next) {
		if (ifa->ifa_name != NULL &&
		    strcmp(ifa->ifa_name, ifname) == 0) {
			found = 1;
			break;
		}
	}
	freeifaddrs(ifaddr);

	if (!found) {
		send_error(fd, req->ver, PCAP_ERR_OPEN,
		    "No such interface");
		return -1;
	}

	/* Get the DLT by opening a quick BPF test */
	int dlt = DLT_EN10MB;  /* default */
	int bpf_test = open("/dev/bpf", O_RDONLY);
	if (bpf_test >= 0) {
		struct ifreq ifr;
		memset(&ifr, 0, sizeof(ifr));
		strlcpy(ifr.ifr_name, ifname, sizeof(ifr.ifr_name));
		if (ioctl(bpf_test, BIOCSETIF, &ifr) == 0) {
			unsigned int udlt;
			if (ioctl(bpf_test, BIOCGDLT, &udlt) == 0)
				dlt = (int)udlt;
		}
		close(bpf_test);
	}

	/* Send OPEN_REPLY */
	openreply.linktype = htonl(dlt);
	openreply.tzoff = 0;

	reply.ver = RPCAP_MAX_VERSION;
	reply.type = RPCAP_MSG_OPEN_REPLY;
	reply.value = 0;
	reply.plen = htonl(sizeof(openreply));

	if (send_msg(fd, &reply, &openreply) != 0)
		return -1;

	return 0;
}

/*
 * Data structure for a capture session on one connection.
 */
struct capture_session {
	int	cs_bpf_fd;		/* BPF file descriptor */
	int	cs_client_fd;		/* client TCP fd */
	int	cs_promisc;		/* promiscuous mode flag */
	int	cs_running;		/* is capture active? */
	uint32_t cs_snaplen;		/* snapshot length */
	uint32_t cs_pkt_count;		/* packet counter */
	uint8_t	cs_buf[BPF_BUF_SIZE];	/* BPF read buffer */
};

/*
 * Send one captured packet as a PACKET message.
 */
static int
send_packet(int fd, struct capture_session *cs,
    const struct bpf_hdr *bh, const uint8_t *data)
{
	struct rpcap_header hdr;
	struct rpcap_pkthdr pkthdr;
	uint32_t caplen = bh->bh_caplen;

	cs->cs_pkt_count++;

	pkthdr.timestamp_sec  = htonl((uint32_t)bh->bh_tstamp.tv_sec);
	pkthdr.timestamp_usec = htonl((uint32_t)bh->bh_tstamp.tv_usec);
	pkthdr.caplen  = htonl(caplen);
	pkthdr.len     = htonl(bh->bh_datalen);
	pkthdr.npkt    = htonl(cs->cs_pkt_count);

	hdr.ver = RPCAP_MAX_VERSION;
	hdr.type = RPCAP_MSG_PACKET;
	hdr.value = 0;
	hdr.plen = htonl(sizeof(pkthdr) + caplen);

	if (sock_send(fd, &hdr, sizeof(hdr)) != 0)
		return -1;
	if (sock_send(fd, &pkthdr, sizeof(pkthdr)) != 0)
		return -1;
	if (caplen > 0) {
		if (sock_send(fd, data, caplen) != 0)
			return -1;
	}
	return 0;
}

/*
 * Capture loop: read from BPF device and send PACKET messages.
 * Also checks the control socket for commands (ENDCAP, CLOSE, STATS).
 * Returns 0 on clean stop, -1 on error.
 */
static int
capture_loop(struct capture_session *cs)
{
	int bpf_fd = cs->cs_bpf_fd;
	int cli_fd = cs->cs_client_fd;

	while (cs->cs_running && !g_stop) {
		fd_set rfds;
		struct timeval tv;
		int maxfd = (bpf_fd > cli_fd) ? bpf_fd : cli_fd;

		FD_ZERO(&rfds);
		FD_SET(bpf_fd, &rfds);
		FD_SET(cli_fd, &rfds);
		tv.tv_sec = 1;
		tv.tv_usec = 0;

		int ret = select(maxfd + 1, &rfds, NULL, NULL, &tv);
		if (ret < 0) {
			if (errno == EINTR)
				continue;
			return -1;
		}
		if (ret == 0)
			continue;  /* timeout — check g_stop */

		/* Check control socket for incoming commands */
		if (FD_ISSET(cli_fd, &rfds)) {
			struct rpcap_header hdr;
			void *payload = NULL;

			if (recv_msg(cli_fd, &hdr, &payload) != 0)
				return -1;

			uint8_t rtype = hdr.type;

			if (rtype == RPCAP_MSG_ENDCAP_REQ) {
				struct rpcap_header reply;
				reply.ver = RPCAP_MAX_VERSION;
				reply.type = RPCAP_MSG_ENDCAP_REPLY;
				reply.value = 0;
				reply.plen = 0;
				send_msg(cli_fd, &reply, NULL);
				cs->cs_running = 0;
				free(payload);
				return 0;
			} else if (rtype == RPCAP_MSG_CLOSE) {
				cs->cs_running = 0;
				free(payload);
				return 0;
			} else if (rtype == RPCAP_MSG_STATS_REQ) {
				struct rpcap_header reply;
				struct rpcap_stats stats;
				memset(&stats, 0, sizeof(stats));
				stats.svrcapt = htonl(cs->cs_pkt_count);
				reply.ver = RPCAP_MAX_VERSION;
				reply.type = RPCAP_MSG_STATS_REPLY;
				reply.value = 0;
				reply.plen = htonl(sizeof(stats));
				send_msg(cli_fd, &reply, &stats);
			} else if (rtype == RPCAP_MSG_UPDATEFILTER_REQ) {
				struct rpcap_header reply;
				reply.ver = RPCAP_MAX_VERSION;
				reply.type = RPCAP_MSG_UPDATEFILTER_REPLY;
				reply.value = 0;
				reply.plen = 0;
				send_msg(cli_fd, &reply, NULL);
			} else {
				/* Unknown command — ignore or error */
				send_error(cli_fd, hdr.ver,
				    PCAP_ERR_WRONGMSG,
				    "Unexpected command during capture");
			}
			free(payload);
		}

		/* Read packets from BPF */
		if (FD_ISSET(bpf_fd, &rfds)) {
			ssize_t n = read(bpf_fd, cs->cs_buf, BPF_BUF_SIZE);
			if (n < 0) {
				if (errno == EINTR)
					continue;
				return -1;
			}
			if (n == 0)
				break;

			/* Parse BPF buffer for packet entries */
			off_t off = 0;
			while ((size_t)off + sizeof(struct bpf_hdr) <= (size_t)n) {
				struct bpf_hdr *bh = (struct bpf_hdr *)(cs->cs_buf + off);

				if (bh->bh_hdrlen < sizeof(struct bpf_hdr))
					break;

				size_t entry_size = BPF_WORDALIGN(
				    bh->bh_hdrlen + bh->bh_caplen);

				if (off + entry_size > (size_t)n)
					break;

				/* Send packet to client */
				if (send_packet(cli_fd, cs, bh,
				    cs->cs_buf + off + bh->bh_hdrlen) != 0) {
					return -1;
				}

				/* Check snapshot length */
				if (cs->cs_pkt_count >= 0xFFFFFFFFUL)
					break;

				off += (off_t)entry_size;
			}
		}
	}

	return 0;
}

/*
 * Handle STARTCAP_REQ.
 * Opens a BPF device, attaches to the requested interface, and
 * enters the capture loop.
 */
static int
handle_startcap(int fd, struct rpcap_header *req, void *payload)
{
	struct rpcap_header reply;
	struct rpcap_startcapreq *sc_req;
	struct rpcap_startcapreply sc_reply;
	char ifname[IFNAMSIZ];
	struct capture_session cs;

	if (payload == NULL || ntohl(req->plen) < sizeof(struct rpcap_startcapreq)) {
		send_error(fd, req->ver, PCAP_ERR_STARTCAPTURE,
		    "Invalid startcap request");
		return -1;
	}

	sc_req = (struct rpcap_startcapreq *)payload;

	uint32_t snaplen = ntohl(sc_req->snaplen);
	uint16_t flags = ntohs(sc_req->flags);
	int promisc = (flags & RPCAP_STARTCAPREQ_FLAG_PROMISC) ? 1 : 0;

	/* Interface name follows the startcapreq struct */
	size_t req_size = ntohl(req->plen);
	char *ifname_ptr = NULL;
	if (req_size > sizeof(struct rpcap_startcapreq)) {
		ifname_ptr = (char *)payload + sizeof(struct rpcap_startcapreq);
		/* Ensure null-terminated */
		ifname_ptr[req_size - sizeof(struct rpcap_startcapreq) - 1] = '\0';
		strlcpy(ifname, ifname_ptr, sizeof(ifname));
	} else {
		strlcpy(ifname, "any", sizeof(ifname));
	}

	/* Open BPF device */
	int bpf_fd = open("/dev/bpf", O_RDONLY);
	if (bpf_fd < 0) {
		send_error(fd, req->ver, PCAP_ERR_STARTCAPTURE,
		    "Cannot open BPF device");
		return -1;
	}

	/* Set buffer size */
	unsigned int blen = BPF_BUF_SIZE;
	ioctl(bpf_fd, BIOCSBLEN, &blen);

	/* Attach to interface */
	struct ifreq ifr;
	memset(&ifr, 0, sizeof(ifr));
	strlcpy(ifr.ifr_name, ifname, sizeof(ifr.ifr_name));
	if (ioctl(bpf_fd, BIOCSETIF, &ifr) < 0) {
		close(bpf_fd);
		send_error(fd, req->ver, PCAP_ERR_STARTCAPTURE,
		    "Cannot attach BPF to interface");
		return -1;
	}

	/* Set promiscuous mode */
	if (promisc)
		ioctl(bpf_fd, BIOCPROMISC, NULL);

	/* Set immediate mode */
	unsigned int imm = 1;
	ioctl(bpf_fd, BIOCIMMEDIATE, &imm);

	/* Flush any stale data */
	ioctl(bpf_fd, BIOCFLUSH, NULL);

	/* Send STARTCAP_REPLY */
	sc_reply.bufsize = htonl(BPF_BUF_SIZE);
	sc_reply.portdata = 0;
	sc_reply.dummy = 0;

	reply.ver = RPCAP_MAX_VERSION;
	reply.type = RPCAP_MSG_STARTCAP_REPLY;
	reply.value = 0;
	reply.plen = htonl(sizeof(sc_reply));

	if (send_msg(fd, &reply, &sc_reply) != 0) {
		close(bpf_fd);
		return -1;
	}

	/* Enter capture loop */
	memset(&cs, 0, sizeof(cs));
	cs.cs_bpf_fd = bpf_fd;
	cs.cs_client_fd = fd;
	cs.cs_promisc = promisc;
	cs.cs_running = 1;
	cs.cs_snaplen = snaplen > 0 ? snaplen : DEFAULT_SNAPLEN;
	cs.cs_pkt_count = 0;

	int ret = capture_loop(&cs);

	close(bpf_fd);
	return ret;
}

/* ═══════════════════════════════════════════════════════════════════
 * Per-connection handler
 * ═══════════════════════════════════════════════════════════════════ */

static void
handle_client(int fd)
{
	struct rpcap_header hdr;
	void *payload = NULL;

	/*
	 * Phase 1: Authentication.
	 * The first message MUST be AUTH_REQ.
	 */
	if (recv_msg(fd, &hdr, &payload) != 0)
		return;

	if (hdr.type != RPCAP_MSG_AUTH_REQ) {
		send_error(fd, hdr.ver, PCAP_ERR_AUTH,
		    "First message must be AUTH_REQ");
		free(payload);
		return;
	}

	if (handle_auth(fd, &hdr, payload) != 0) {
		free(payload);
		return;
	}
	free(payload);

	/*
	 * Phase 2: Command loop.
	 * After authentication, the client sends various requests.
	 */
	while (!g_stop) {
		if (recv_msg(fd, &hdr, &payload) != 0)
			break;

		switch (hdr.type) {
		case RPCAP_MSG_FINDALLIF_REQ:
			handle_findallif(fd, &hdr);
			break;

		case RPCAP_MSG_OPEN_REQ:
			handle_open(fd, &hdr, payload);
			break;

		case RPCAP_MSG_STARTCAP_REQ:
			/*
			 * handle_startcap enters the capture loop and
			 * does not return until capture ends or error.
			 */
			handle_startcap(fd, &hdr, payload);
			break;

		case RPCAP_MSG_CLOSE:
			/* Client sent CLOSE — exit */
			free(payload);
			return;

		case RPCAP_MSG_STATS_REQ: {
			struct rpcap_header reply;
			struct rpcap_stats stats;
			memset(&stats, 0, sizeof(stats));
			reply.ver = RPCAP_MAX_VERSION;
			reply.type = RPCAP_MSG_STATS_REPLY;
			reply.value = 0;
			reply.plen = htonl(sizeof(stats));
			send_msg(fd, &reply, &stats);
			break;
		}

		case RPCAP_MSG_SETSAMPLING_REQ: {
			struct rpcap_header reply;
			reply.ver = RPCAP_MAX_VERSION;
			reply.type = RPCAP_MSG_SETSAMPLING_REPLY;
			reply.value = 0;
			reply.plen = 0;
			send_msg(fd, &reply, NULL);
			break;
		}

		case RPCAP_MSG_UPDATEFILTER_REQ: {
			struct rpcap_header reply;
			reply.ver = RPCAP_MAX_VERSION;
			reply.type = RPCAP_MSG_UPDATEFILTER_REPLY;
			reply.value = 0;
			reply.plen = 0;
			send_msg(fd, &reply, NULL);
			break;
		}

		default:
			/* Unknown message type */
			send_error(fd, hdr.ver, PCAP_ERR_WRONGMSG,
			    "Unknown message type");
			break;
		}

		free(payload);
		payload = NULL;
	}
}

/* ═══════════════════════════════════════════════════════════════════
 * Main
 * ═══════════════════════════════════════════════════════════════════ */

static void __dead
usage(void)
{
	fprintf(stderr,
	    "Usage: rpcapd [options]\n"
	    "Options:\n"
	    "  -p port       Listening port (default: %s)\n"
	    "  -4            Listen on IPv4 only\n"
	    "  -6            Listen on IPv6 only\n"
	    "  -u username   Require this username for authentication\n"
	    "  -P password   Require this password for authentication\n"
	    "  -d            Run in foreground (don't daemonize)\n"
	    "  -h            Show this help\n",
	    RPCAP_DEFAULT_NETPORT);
	exit(1);
}

int
main(int argc, char *argv[])
{
	const char *port = RPCAP_DEFAULT_NETPORT;
	int af = AF_UNSPEC;	/* accept both v4 and v6 */
	int daemonize = 1;
	int listen_fd4 = -1, listen_fd6 = -1;
	int opt;

	signal(SIGINT, sigint_handler);
	signal(SIGPIPE, SIG_IGN);
	signal(SIGCHLD, SIG_IGN);

	while ((opt = getopt(argc, argv, "p:46u:P:dh")) != -1) {
		switch (opt) {
		case 'p':
			port = optarg;
			break;
		case '4':
			af = AF_INET;
			break;
		case '6':
			af = AF_INET6;
			break;
		case 'u':
			g_username = optarg;
			break;
		case 'P':
			g_password = optarg;
			break;
		case 'd':
			daemonize = 0;
			break;
		case 'h':
		default:
			usage();
		}
	}

	/* Create IPv4 listener */
	if (af != AF_INET6) {
		struct sockaddr_in sin;
		memset(&sin, 0, sizeof(sin));
		sin.sin_family = AF_INET;
		sin.sin_addr.s_addr = INADDR_ANY;
		sin.sin_port = htons((uint16_t)atoi(port));

		listen_fd4 = socket(AF_INET, SOCK_STREAM, 0);
		if (listen_fd4 < 0) {
			perror("rpcapd: socket(AF_INET)");
			return 1;
		}

		int on = 1;
		setsockopt(listen_fd4, SOL_SOCKET, SO_REUSEADDR, &on, sizeof(on));

		if (bind(listen_fd4, (struct sockaddr *)&sin, sizeof(sin)) < 0) {
			perror("rpcapd: bind (IPv4)");
			return 1;
		}

		if (listen(listen_fd4, RPCAPD_BACKLOG) < 0) {
			perror("rpcapd: listen (IPv4)");
			return 1;
		}
	}

	/* Create IPv6 listener */
	if (af != AF_INET) {
		struct sockaddr_in6 sin6;
		memset(&sin6, 0, sizeof(sin6));
		sin6.sin6_family = AF_INET6;
		sin6.sin6_addr = in6addr_any;
		sin6.sin6_port = htons((uint16_t)atoi(port));

		listen_fd6 = socket(AF_INET6, SOCK_STREAM, 0);
		if (listen_fd6 >= 0) {
			int on = 1;
			setsockopt(listen_fd6, SOL_SOCKET, SO_REUSEADDR, &on, sizeof(on));
#ifdef IPV6_V6ONLY
			setsockopt(listen_fd6, IPPROTO_IPV6, IPV6_V6ONLY, &on, sizeof(on));
#endif

			if (bind(listen_fd6, (struct sockaddr *)&sin6, sizeof(sin6)) < 0) {
				close(listen_fd6);
				listen_fd6 = -1;
			} else if (listen(listen_fd6, RPCAPD_BACKLOG) < 0) {
				close(listen_fd6);
				listen_fd6 = -1;
			}
		}
	}

	if (listen_fd4 < 0 && listen_fd6 < 0) {
		fprintf(stderr, "rpcapd: no listening socket available\n");
		return 1;
	}

	if (g_username != NULL) {
		fprintf(stderr, "rpcapd: authentication required (user: %s)\n",
		    g_username);
	} else if (g_password != NULL) {
		fprintf(stderr, "rpcapd: password authentication required\n");
	}
	fprintf(stderr, "rpcapd: listening on port %s\n", port);

	/* Daemonize */
	if (daemonize) {
		if (daemon(0, 0) < 0) {
			perror("rpcapd: daemon");
			return 1;
		}
	}

	/* Accept loop */
	while (!g_stop) {
		fd_set rfds;
		int maxfd = 0;

		FD_ZERO(&rfds);
		if (listen_fd4 >= 0) {
			FD_SET(listen_fd4, &rfds);
			if (listen_fd4 > maxfd) maxfd = listen_fd4;
		}
		if (listen_fd6 >= 0) {
			FD_SET(listen_fd6, &rfds);
			if (listen_fd6 > maxfd) maxfd = listen_fd6;
		}

		int ret = select(maxfd + 1, &rfds, NULL, NULL, NULL);
		if (ret < 0) {
			if (errno == EINTR)
				continue;
			break;
		}

		int client_fd;

		if (listen_fd4 >= 0 && FD_ISSET(listen_fd4, &rfds)) {
			client_fd = accept(listen_fd4, NULL, NULL);
		} else if (listen_fd6 >= 0 && FD_ISSET(listen_fd6, &rfds)) {
			client_fd = accept(listen_fd6, NULL, NULL);
		} else {
			continue;
		}

		if (client_fd < 0) {
			if (errno == EINTR)
				continue;
			break;
		}

		/* Fork for each client */
		pid_t pid = fork();
		if (pid < 0) {
			close(client_fd);
			continue;
		}
		if (pid == 0) {
			/* Child: handle client */
			signal(SIGINT, SIG_DFL);
			if (listen_fd4 >= 0) close(listen_fd4);
			if (listen_fd6 >= 0) close(listen_fd6);
			handle_client(client_fd);
			close(client_fd);
			_exit(0);
		}
		/* Parent: close client fd and continue */
		close(client_fd);
	}

	/* Cleanup */
	if (listen_fd4 >= 0) close(listen_fd4);
	if (listen_fd6 >= 0) close(listen_fd6);

	return 0;
}
