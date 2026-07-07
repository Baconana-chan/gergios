/*
 * netstat — network statistics utility for GergiOS
 *
 * Displays network interface statistics, protocol statistics,
 * and active connection information by querying the lwIP service
 * via the sysctl MIB interface.
 *
 * Usage:
 *   netstat        — show active connections (like netstat -a)
 *   netstat -i     — show per-interface statistics
 *   netstat -s     — show protocol statistics
 *   netstat -h     — show help
 */

#include <sys/types.h>
#include <sys/socket.h>
#include <sys/sysctl.h>
#include <net/if.h>
#include <net/if_dl.h>
#include <net/if_types.h>
#include <net/route.h>
#include <netinet/in.h>
#include <netinet/in_systm.h>
#include <netinet/ip.h>
#include <netinet/ip_var.h>
#include <netinet/tcp.h>
#include <netinet/tcp_fsm.h>
#include <netinet/tcp_var.h>
#include <netinet/udp.h>
#include <netinet/udp_var.h>
#include <arpa/inet.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <unistd.h>
#include <inttypes.h>

/* Interface statistics structure (mirrors ifstat.c). */
struct if_stat {
	char		ifs_name[IFNAMSIZ];
	uint64_t	ifs_type;
	uint64_t	ifs_mtu;
	uint64_t	ifs_link_state;
	uint64_t	ifs_ipackets;
	uint64_t	ifs_ierrors;
	uint64_t	ifs_opackets;
	uint64_t	ifs_oerrors;
	uint64_t	ifs_ibytes;
	uint64_t	ifs_obytes;
	uint64_t	ifs_imcasts;
	uint64_t	ifs_omcasts;
	uint64_t	ifs_iqdrops;
	uint64_t	ifs_collisions;
};

/* TCP extended entry structure (mirrors tcp_ext.c). */
struct tcp_ext_entry {
	uint64_t tee_sockaddr;
	uint32_t tee_state;
	uint32_t tee_snd_cwnd;
	uint32_t tee_snd_wnd;
	uint32_t tee_rcv_wnd;
	uint32_t tee_rto;
	uint32_t tee_rtt;
	uint32_t tee_rttvar;
	uint32_t tee_nrtx;
	uint32_t tee_mss;
	uint32_t tee_snd_buf;
	uint32_t tee_unsent;
	uint32_t tee_unacked;
	uint8_t  tee_local_ip[16];
	uint8_t  tee_remote_ip[16];
	uint16_t tee_local_port;
	uint16_t tee_remote_port;
	uint8_t  tee_is_v6;
};

/* Driver info structure (mirrors ndev.c). */
#define NDI_ACTIVE	0x01
#define NDI_INUSE	0x02

struct ndev_drvinfo {
	char		ndi_label[16];	/* LABEL_MAX = 16 */
	int		ndi_endpt;
	uint32_t	ndi_flags;
	uint32_t	ndi_sendq_depth[2];	/* NDEV_NUM_SENDQ = 2 */
	uint32_t	ndi_sendq_max[2];
	uint32_t	ndi_recvq_depth;
	uint32_t	ndi_recvq_max;
};

static int show_interfaces;	/* -i */
static int show_stats;		/* -s */
static int show_drivers;	/* -d */

/*
 * Human-readable bytes formatting.
 */
static const char *
fmt_bytes(uint64_t bytes)
{
	static char buf[32];

	if (bytes < 1024)
		snprintf(buf, sizeof(buf), "%" PRIu64 " B", bytes);
	else if (bytes < 1024 * 1024)
		snprintf(buf, sizeof(buf), "%.1f KB", (double)bytes / 1024);
	else if (bytes < 1024LL * 1024 * 1024)
		snprintf(buf, sizeof(buf), "%.1f MB",
		    (double)bytes / (1024 * 1024));
	else
		snprintf(buf, sizeof(buf), "%.1f GB",
		    (double)bytes / (1024LL * 1024 * 1024));
	return buf;
}

/*
 * Format link state number to string.
 */
static const char *
fmt_link_state(uint64_t state)
{
	switch ((int)state) {
	case LINK_STATE_UP:	return "UP";
	case LINK_STATE_DOWN:	return "DOWN";
	default:		return "UNKN";
	}
}

/*
 * Query interface stats from minix.lwip.ifaces.
 * Returns number of interfaces read, or -1 on error.
 */
static int
query_if_stats(struct if_stat *entries, size_t max)
{
	size_t len;

	len = max * sizeof(struct if_stat);
	if (sysctlbyname("minix.lwip.ifaces", entries, &len, NULL, 0) != 0)
		return -1;

	return (int)(len / sizeof(struct if_stat));
}

/*
 * Show per-interface statistics (netstat -i).
 */
static void
show_if_stats(void)
{
	struct if_stat entries[16];
	int count, i;

	count = query_if_stats(entries, 16);
	if (count <= 0) {
		printf("netstat: unable to query interface stats\n");
		return;
	}

	printf("%-8s %5s %6s %5s %10s %10s %8s %8s %6s %6s\n",
	    "Name", "MTU", "Link", "Type",
	    "Ipkts", "Opkts", "Ierrs", "Oerrs",
	    "Drop", "Coll");
	printf("%-8s %5s %6s %5s %10s %10s %8s %8s %6s %6s\n",
	    "------", "---", "------", "-----",
	    "-----", "-----", "-----", "-----",
	    "----", "----");

	for (i = 0; i < count; i++) {
		const char *type;

		switch ((int)entries[i].ifs_type) {
		case IFT_ETHER:	type = "ether"; break;
		case IFT_LOOP:	type = "loop";  break;
		default:	type = "unk?";  break;
		}

		printf("%-8s %5" PRIu64 " %-6s %5s %10" PRIu64 " %10"
		    PRIu64 " %8" PRIu64 " %8" PRIu64 " %6" PRIu64 " %6"
		    PRIu64 "\n",
		    entries[i].ifs_name,
		    entries[i].ifs_mtu,
		    fmt_link_state(entries[i].ifs_link_state),
		    type,
		    entries[i].ifs_ipackets,
		    entries[i].ifs_opackets,
		    entries[i].ifs_ierrors,
		    entries[i].ifs_oerrors,
		    entries[i].ifs_iqdrops,
		    entries[i].ifs_collisions);
	}
}

/*
 * Show protocol statistics (netstat -s).
 */
static void
show_proto_stats(void)
{
	uint64_t val;

	printf("--- Protocol Statistics ---\n");

	/* TCP stats */
	if (sysctlbyname("net.inet.tcp.sendspace", &val, sizeof(val),
	    NULL, 0) == 0) {
		printf("TCP:\n");
		printf("  Default send buffer:  %s\n", fmt_bytes(val));
	}
	if (sysctlbyname("net.inet.tcp.recvspace", &val, sizeof(val),
	    NULL, 0) == 0)
		printf("  Default recv buffer:  %s\n", fmt_bytes(val));

	/* UDP stats */
	if (sysctlbyname("net.inet.udp.sendspace", &val, sizeof(val),
	    NULL, 0) == 0) {
		printf("UDP:\n");
		printf("  Default send buffer:  %s\n", fmt_bytes(val));
	}
	if (sysctlbyname("net.inet.udp.recvspace", &val, sizeof(val),
	    NULL, 0) == 0)
		printf("  Default recv buffer:  %s\n", fmt_bytes(val));

	/* IP stats */
	if (sysctlbyname("net.inet.ip.forwarding", &val, sizeof(val),
	    NULL, 0) == 0) {
		printf("IP:\n");
		printf("  Forwarding: %s\n", val ? "on" : "off");
	}

	/* Latency histograms */
	printf("Latency histograms:\n");
	printf("  Available via: sysctl minix.lwip.latency\n");
}

/*
 * Format TCP state.
 */
static const char *
fmt_tcp_state(uint32_t state)
{
	switch ((int)state) {
	case TCPS_CLOSED:	return "CLOSED";
	case TCPS_LISTEN:	return "LISTEN";
	case TCPS_SYN_SENT:	return "SYN_SENT";
	case TCPS_SYN_RECEIVED:	return "SYN_RCVD";
	case TCPS_ESTABLISHED:	return "ESTAB";
	case TCPS_CLOSE_WAIT:	return "CLOSE_WAIT";
	case TCPS_FIN_WAIT_1:	return "FIN_WAIT_1";
	case TCPS_FIN_WAIT_2:	return "FIN_WAIT_2";
	case TCPS_CLOSING:	return "CLOSING";
	case TCPS_LAST_ACK:	return "LAST_ACK";
	case TCPS_TIME_WAIT:	return "TIME_WAIT";
	default:		return "UNKNOWN";
	}
}

/*
 * Format an IP address from raw bytes.
 */
static const char *
fmt_ipaddr(const uint8_t *addr, int is_v6)
{
	static char buf[64];
	static char ip6[INET6_ADDRSTRLEN];

	if (is_v6) {
		struct in6_addr a;

		memcpy(a.s6_addr, addr, 16);
		if (inet_ntop(AF_INET6, &a, ip6, sizeof(ip6)) != NULL)
			snprintf(buf, sizeof(buf), "%s", ip6);
		else
			snprintf(buf, sizeof(buf), "::");
	} else {
		struct in_addr a;

		memcpy(&a, addr, 4);
		snprintf(buf, sizeof(buf), "%s", inet_ntoa(a));
	}
	return buf;
}

/*
 * Query TCP extended info from minix.lwip.tcp_ext.
 */
static int
query_tcp_ext(struct tcp_ext_entry *entries, size_t *count)
{
	size_t len;

	len = *count * sizeof(struct tcp_ext_entry);
	if (sysctlbyname("minix.lwip.tcp_ext", entries, &len, NULL, 0) != 0)
		return -1;

	*count = len / sizeof(struct tcp_ext_entry);
	return 0;
}

/*
 * Show all active TCP connections (default output).
 */
static void
show_connections(void)
{
	struct tcp_ext_entry entries[32];
	size_t count;
	unsigned int i;

	count = sizeof(entries) / sizeof(entries[0]);

	if (query_tcp_ext(entries, &count) != 0) {
		printf("(TCP extended metrics unavailable — "
		    "try sysctl minix.lwip.tcp_ext)\n");
		return;
	}

	printf("%-18s %-5s %-18s %-5s %-10s %5s %5s %4s %4s\n",
	    "Local", "Port", "Remote", "Port",
	    "State", "CWND", "RTT", "RTO", "RTX");
	printf("%-18s %-5s %-18s %-5s %-10s %5s %5s %4s %4s\n",
	    "-----", "----", "-----", "----",
	    "-----", "----", "---", "---", "---");

	for (i = 0; i < count; i++) {
		/*
		 * lwIP stores ports in network byte order.
		 * Use ntohs() to convert for display.
		 */
		printf("%-18s %-5u %-18s %-5u %-10s %5u %5u %4u %4u\n",
		    fmt_ipaddr(entries[i].tee_local_ip,
		        entries[i].tee_is_v6),
		    ntohs(entries[i].tee_local_port),
		    fmt_ipaddr(entries[i].tee_remote_ip,
		        entries[i].tee_is_v6),
		    ntohs(entries[i].tee_remote_port),
		    fmt_tcp_state(entries[i].tee_state),
		    entries[i].tee_snd_cwnd,
		    entries[i].tee_rtt,
		    entries[i].tee_rto,
		    entries[i].tee_nrtx);
	}

	printf("\n  %zu active connections\n", count);
}

/*
 * Query driver info from minix.lwip.drivers.info.
 * Returns number of drivers read, or -1 on error.
 */
static int
query_driver_stats(struct ndev_drvinfo *entries, size_t max)
{
	size_t len;

	len = max * sizeof(struct ndev_drvinfo);
	if (sysctlbyname("minix.lwip.drivers.info", entries, &len,
	    NULL, 0) != 0)
		return -1;

	return (int)(len / sizeof(struct ndev_drvinfo));
}

/*
 * Show per-driver statistics (netstat -d).
 */
static void
show_driver_stats(void)
{
	struct ndev_drvinfo entries[8];
	int count, i;
	uint64_t val;

	count = query_driver_stats(entries, 8);
	if (count <= 0) {
		printf("netstat: unable to query driver stats\n");
		return;
	}

	printf("--- Network Driver Statistics ---\n");

	if (sysctlbyname("minix.lwip.drivers.pending", &val, sizeof(val),
	    NULL, 0) == 0 && val > 0)
		printf("  %" PRIu64 " driver(s) pending initialization\n", val);

	for (i = 0; i < count; i++) {
		const char *status;

		if (entries[i].ndi_flags & NDI_ACTIVE)
			status = "active";
		else if (entries[i].ndi_flags & NDI_INUSE)
			status = "init";
		else
			continue;

		printf("\n");
		printf("  %s (endpt %d) [%s]\n",
		    entries[i].ndi_label,
		    entries[i].ndi_endpt,
		    status);
		printf("    SendQ[0]: %u/%u  SendQ[1]: %u/%u\n",
		    entries[i].ndi_sendq_depth[0],
		    entries[i].ndi_sendq_max[0],
		    entries[i].ndi_sendq_depth[1],
		    entries[i].ndi_sendq_max[1]);
		printf("    RecvQ:    %u/%u\n",
		    entries[i].ndi_recvq_depth,
		    entries[i].ndi_recvq_max);
	}
}

/*
 * Print usage information.
 */
static void
usage(void)
{

	printf("Usage: netstat [ -h ] [ -i ] [ -s ] [ -d ]\n");
	printf("Options:\n");
	printf("  -i    Show per-interface statistics\n");
	printf("  -s    Show protocol statistics\n");
	printf("  -d    Show network driver statistics\n");
	printf("  -a    Show all active TCP connections (default)\n");
	printf("  -h    Show this help message\n");
}

int
main(int argc, char *argv[])
{
	int opt;

	show_interfaces = 0;
	show_stats = 0;
	show_drivers = 0;

	while ((opt = getopt(argc, argv, "hids")) != -1) {
		switch (opt) {
		case 'i':
			show_interfaces = 1;
			break;
		case 'd':
			show_drivers = 1;
			break;
		case 's':
			show_stats = 1;
			break;
		case 'h':
			usage();
			return 0;
		default:
			usage();
			return 1;
		}
	}

	/* If no flags given, show TCP connections (default). */
	if (!show_interfaces && !show_stats && !show_drivers)
		show_connections();

	if (show_interfaces)
		show_if_stats();

	if (show_drivers)
		show_driver_stats();

	if (show_stats)
		show_proto_stats();

	return 0;
}
