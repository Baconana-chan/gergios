/* main.c — pcapng capture tool: captures from BPF and writes pcapng format */

#include "pcapng.h"

#include <sys/types.h>
#include <sys/time.h>
#include <sys/ioctl.h>
#include <net/bpf.h>
#include <net/dlt.h>
#include <net/if.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>

/*
 * Default BPF buffer size (32 KB).
 */
#define BPF_BUF_SIZE	32768

/*
 * Global flag for signal handler.
 */
static volatile int g_stop = 0;

static void
sigint_handler(int sig __unused)
{

	g_stop = 1;
}

/*
 * Open /dev/bpf and return a file descriptor, or -1 on error.
 * MINIX uses a cloning BPF device: only /dev/bpf (minor 0) accepts
 * open, which returns a new cloned instance with a unique minor.
 */
static int
bpf_open(void)
{
	int fd;

	fd = open("/dev/bpf", O_RDONLY);
	if (fd < 0) {
		fprintf(stderr, "pcapng: unable to open /dev/bpf: %s\n",
		    strerror(errno));
		return -1;
	}

	return fd;
}

/*
 * Attach BPF device to an interface.
 * Returns 0 on success, -1 on error.
 */
static int
bpf_attach(int fd, const char *ifname)
{
	struct ifreq ifr;

	memset(&ifr, 0, sizeof(ifr));
	strlcpy(ifr.ifr_name, ifname, sizeof(ifr.ifr_name));

	if (ioctl(fd, BIOCSETIF, &ifr) < 0) {
		fprintf(stderr, "pcapng: BIOCSETIF %s: %s\n",
		    ifname, strerror(errno));
		return -1;
	}

	return 0;
}

/*
 * Get the DLT for the attached interface.
 * Returns DLT_ value on success, -1 on error.
 */
static int
bpf_get_dlt(int fd)
{
	unsigned int dlt;

	if (ioctl(fd, BIOCGDLT, &dlt) < 0) {
		fprintf(stderr, "pcapng: BIOCGDLT: %s\n", strerror(errno));
		return -1;
	}

	return (int)dlt;
}

/*
 * Get the interface name (BIOCGETIF).
 */
static int
bpf_get_ifname(int fd, char *name, size_t namelen)
{
	struct ifreq ifr;

	memset(&ifr, 0, sizeof(ifr));

	if (ioctl(fd, BIOCGETIF, &ifr) < 0) {
		fprintf(stderr, "pcapng: BIOCGETIF: %s\n", strerror(errno));
		return -1;
	}

	strlcpy(name, ifr.ifr_name, namelen);
	return 0;
}

/*
 * Set BPF buffer size (BIOCSBLEN).
 * Returns 0 on success, -1 on error.
 */
static int
bpf_set_bufsize(int fd, unsigned int size)
{
	unsigned int blen = size;

	if (ioctl(fd, BIOCSBLEN, &blen) < 0) {
		fprintf(stderr, "pcapng: BIOCSBLEN: %s\n", strerror(errno));
		return -1;
	}

	return 0;
}

/*
 * Enable immediate mode (BIOCIMMEDIATE).
 * Returns 0 on success, -1 on error.
 */
static int
bpf_set_immediate(int fd)
{
	unsigned int imm = 1;

	if (ioctl(fd, BIOCIMMEDIATE, &imm) < 0) {
		fprintf(stderr, "pcapng: BIOCIMMEDIATE: %s\n", strerror(errno));
		return -1;
	}

	return 0;
}

/*
 * Set promiscuous mode (BIOCPROMISC).
 * Returns 0 on success, -1 on error.
 */
static int
bpf_set_promisc(int fd)
{
	if (ioctl(fd, BIOCPROMISC, NULL) < 0) {
		/* Not fatal (may not be supported for "any"). */
		fprintf(stderr, "pcapng: BIOCPROMISC: %s (ignored)\n",
		    strerror(errno));
	}

	return 0;
}

/*
 * Flush BPF buffer.
 */
static void
bpf_flush(int fd)
{

	ioctl(fd, BIOCFLUSH, NULL);
}

/*
 * Capture packets and write them in pcapng format.
 * Returns number of packets captured.
 */
static unsigned long
capture(int fd, FILE *outfp, uint32_t iface_id, unsigned int snaplen,
    unsigned long max_packets)
{
	struct bpf_hdr *bh;
	char *buf;
	ssize_t n;
	unsigned long count = 0;

	buf = malloc(BPF_BUF_SIZE);
	if (buf == NULL) {
		fprintf(stderr, "pcapng: out of memory\n");
		return 0;
	}

	/*
	 * Read packets from BPF in a loop.  The BPF device fills the buffer
	 * with a series of bpf_hdr + packet data entries.  We parse each
	 * entry and write an EPB for it.
	 */
	while (!g_stop) {
		n = read(fd, buf, BPF_BUF_SIZE);
		if (n < 0) {
			if (errno == EINTR)
				continue;
			fprintf(stderr, "pcapng: read error: %s\n",
			    strerror(errno));
			break;
		}
		if (n == 0)
			break;

		/*
		 * Parse the BPF buffer: it contains one or more packets,
		 * each preceded by a struct bpf_hdr.
		 */
		off_t off = 0;

		while ((size_t)off + sizeof(struct bpf_hdr) <= (size_t)n) {
			struct timeval tv;

			bh = (struct bpf_hdr *)(buf + off);

			/* Sanity check the header length. */
			if (bh->bh_hdrlen < sizeof(struct bpf_hdr))
				break;

			/* Calculate total entry size (header + data, padded). */
			size_t entry_size = BPF_WORDALIGN(
			    bh->bh_hdrlen + bh->bh_caplen);

			if (off + entry_size > (size_t)n)
				break;

			/* Get timestamp from the BPF header. */
			tv.tv_sec = bh->bh_tstamp.tv_sec;
			tv.tv_usec = bh->bh_tstamp.tv_usec;

			/* Write the EPB. */
			if (pcapng_write_epb(outfp, iface_id,
			    (uint32_t)tv.tv_sec, (uint32_t)tv.tv_usec,
			    buf + off + bh->bh_hdrlen,
			    bh->bh_caplen, bh->bh_datalen) != 0) {
				fprintf(stderr,
				    "pcapng: write EPB failed: %s\n",
				    strerror(errno));
				free(buf);
				return count;
			}

			count++;

			if (max_packets > 0 && count >= max_packets) {
				free(buf);
				return count;
			}

			off += (off_t)entry_size;
		}
	}

	free(buf);
	return count;
}

static void
usage(void)
{

	fprintf(stderr,
	    "Usage: pcapng [options] [-w file] [filter]\n"
	    "Options:\n"
	    "  -i ifname    Interface to capture from (default: \"any\")\n"
	    "  -w file      Write to file (default: stdout)\n"
	    "  -s snaplen   Snapshot length (default: %d)\n"
	    "  -c count     Stop after capturing 'count' packets\n"
	    "  -p           Don't put interface in promiscuous mode\n"
	    "  -D           List available interfaces\n"
	    "  -h           Show this help\n",
	    PCAPNG_SNAPLEN_DEF);
}

int
main(int argc, char *argv[])
{
	const char *ifname = "any";
	const char *outfile = NULL;
	unsigned int snaplen = PCAPNG_SNAPLEN_DEF;
	unsigned long max_packets = 0;
	int promisc = 1;
	int list_ifaces = 0;
	int bpf_fd;
	FILE *outfp;
	int dlt;
	char bpf_ifname[IFNAMSIZ];
	unsigned long count;
	int opt;

	signal(SIGINT, sigint_handler);

	while ((opt = getopt(argc, argv, "hi:w:s:c:pD")) != -1) {
		switch (opt) {
		case 'i':
			ifname = optarg;
			break;
		case 'w':
			outfile = optarg;
			break;
		case 's':
			snaplen = (unsigned int)atoi(optarg);
			if (snaplen < 16 || snaplen > 262144) {
				fprintf(stderr,
				    "pcapng: invalid snaplen: %s\n", optarg);
				return 1;
			}
			break;
		case 'c':
			max_packets = (unsigned long)atol(optarg);
			break;
		case 'p':
			promisc = 0;
			break;
		case 'D':
			list_ifaces = 1;
			break;
		case 'h':
			usage();
			return 0;
		default:
			usage();
			return 1;
		}
	}

	if (list_ifaces) {
		/* TODO: list interfaces via sysctl if needed. */
		fprintf(stderr, "pcapng: -D not yet implemented\n");
		return 1;
	}

	/* Open BPF device. */
	bpf_fd = bpf_open();
	if (bpf_fd < 0)
		return 1;

	/* Set buffer size. */
	if (bpf_set_bufsize(bpf_fd, BPF_BUF_SIZE) < 0) {
		close(bpf_fd);
		return 1;
	}

	/* Attach to interface. */
	if (bpf_attach(bpf_fd, ifname) < 0) {
		close(bpf_fd);
		return 1;
	}

	/* Get interface name and DLT. */
	if (bpf_get_ifname(bpf_fd, bpf_ifname, sizeof(bpf_ifname)) < 0) {
		strlcpy(bpf_ifname, ifname, sizeof(bpf_ifname));
	}
	dlt = bpf_get_dlt(bpf_fd);
	if (dlt < 0) {
		close(bpf_fd);
		return 1;
	}

	/* Enable promiscuous mode. */
	if (promisc)
		bpf_set_promisc(bpf_fd);

	/* Enable immediate mode for low-latency capture. */
	bpf_set_immediate(bpf_fd);

	/* Flush any stale data. */
	bpf_flush(bpf_fd);

	/* Open output file (or use stdout). */
	if (outfile != NULL) {
		outfp = fopen(outfile, "wb");
		if (outfp == NULL) {
			fprintf(stderr, "pcapng: cannot open %s: %s\n",
			    outfile, strerror(errno));
			close(bpf_fd);
			return 1;
		}
	} else
		outfp = stdout;

	/* Write Section Header Block. */
	if (pcapng_write_shb(outfp, "GergiOS", "pcapng") != 0) {
		fprintf(stderr, "pcapng: write SHB failed\n");
		if (outfile != NULL) fclose(outfp);
		close(bpf_fd);
		return 1;
	}

	/* Write Interface Description Block (one interface). */
	if (pcapng_write_idb(outfp, (uint16_t)dlt, bpf_ifname,
	    PCAPNG_SNAPLEN_DEF, snaplen) != 0) {
		fprintf(stderr, "pcapng: write IDB failed\n");
		if (outfile != NULL) fclose(outfp);
		close(bpf_fd);
		return 1;
	}

	/* Capture packets and write EPBs. */
	fprintf(stderr, "pcapng: capturing on %s (DLT %d)...\n",
	    bpf_ifname, dlt);
	fprintf(stderr, "pcapng: Ctrl-C to stop\n");

	count = capture(bpf_fd, outfp, 0 /*iface_id*/, snaplen, max_packets);

	fprintf(stderr, "pcapng: captured %lu packet(s)\n", count);

	if (outfile != NULL)
		fclose(outfp);

	close(bpf_fd);

	return 0;
}
