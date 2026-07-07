/* pcapng.h — PCAP Next Generation (pcapng) block definitions and writer API */

#ifndef PCAPNG_H
#define PCAPNG_H

#include <sys/types.h>
#include <stdint.h>
#include <stdio.h>

/* Block type codes. */
#define PCAPNG_SHB		0x0A0D0D0AU	/* Section Header Block */
#define PCAPNG_IDB		0x00000001U	/* Interface Description Block */
#define PCAPNG_EPB		0x00000006U	/* Enhanced Packet Block */
#define PCAPNG_ISB		0x00000005U	/* Interface Statistics Block */

/* SHB mandatory fields. */
#define PCAPNG_BYTE_ORDER_MAGIC	0x1A2B3C4DU
#define PCAPNG_MAJOR_VERSION	1
#define PCAPNG_MINOR_VERSION	0

/* Option type codes (common). */
#define PCAPNG_OPT_ENDOFOPT	0
#define PCAPNG_OPT_COMMENT	1

/* SHB option codes. */
#define PCAPNG_SHB_HARDWARE	2
#define PCAPNG_SHB_OS		3
#define PCAPNG_SHB_USERAPPL	4

/* IDB option codes. */
#define PCAPNG_IDB_NAME		2
#define PCAPNG_IDB_DESCRIPTION	3
#define PCAPNG_IDB_TSRESOL	9

/* Timestamp resolution: 10^-6 (microseconds). */
#define PCAPNG_TSRESOL_MICRO	6

/* Minimum and default snapshot length. */
#define PCAPNG_SNAPLEN_DEF	65535

/*
 * Block header: common to all blocks.
 */
struct pcapng_block_hdr {
	uint32_t	bh_type;	/* block type */
	uint32_t	bh_totlen;	/* total block length (incl. header) */
};

/*
 * Section Header Block (SHB): always first block in the file.
 * Total size: 28 bytes + options.
 */
struct pcapng_shb {
	struct pcapng_block_hdr	shb_hdr;
	uint32_t		shb_byte_order;	/* = PCAPNG_BYTE_ORDER_MAGIC */
	uint16_t		shb_major;	/* = PCAPNG_MAJOR_VERSION */
	uint16_t		shb_minor;	/* = PCAPNG_MINOR_VERSION */
	uint64_t		shb_section_len;/* = -1 (unknown/unspecified) */
	/* Options follow, terminated by PCAPNG_OPT_ENDOFOPT. */
};

/*
 * Interface Description Block (IDB): describes a capture interface.
 * Total size: 16 bytes + options.
 */
struct pcapng_idb {
	struct pcapng_block_hdr	idb_hdr;
	uint16_t		idb_linktype;	/* DLT_ value */
	uint16_t		__reserved;	/* must be 0 */
	uint32_t		idb_snaplen;	/* snapshot length */
	/* Options follow, terminated by PCAPNG_OPT_ENDOFOPT. */
};

/*
 * Enhanced Packet Block (EPB): stores a captured packet.
 * Total size: 32 bytes + packet data (padded to 4) + options.
 */
struct pcapng_epb {
	struct pcapng_block_hdr	epb_hdr;
	uint32_t		epb_iface_id;	/* interface index (0-based) */
	uint32_t		epb_ts_high;	/* timestamp seconds (high) */
	uint32_t		epb_ts_low;	/* timestamp microseconds (low) */
	uint32_t		epb_caplen;	/* captured packet length */
	uint32_t		epb_origlen;	/* original packet length */
	/* Packet data follows (padded to 4-byte boundary). */
};

/*
 * Option: 4-byte aligned TLV.
 */
struct pcapng_opt {
	uint16_t	opt_code;
	uint16_t	opt_len;	/* length of value, excluding padding */
	/* Value follows, padded to 4-byte boundary. */
};

/* ---- Writer API ---- */

/*
 * Open a pcapng file for writing and write the Section Header Block.
 * Returns 0 on success, -1 on error.
 */
int pcapng_write_shb(FILE *fp, const char *os, const char *app);

/*
 * Write an Interface Description Block.
 * 'linktype' is a DLT_ value (e.g., DLT_EN10MB).
 * 'ifname' is the interface name (e.g., "e0").
 * 'mtu' is the interface MTU.
 * 'snaplen' is the snapshot length for this interface.
 * Returns 0 on success, -1 on error.
 */
int pcapng_write_idb(FILE *fp, uint16_t linktype, const char *ifname,
    uint32_t mtu, uint32_t snaplen);

/*
 * Write an Enhanced Packet Block.
 * 'iface_id' is the 0-based interface ID (from IDB order).
 * 'ts_sec' and 'ts_usec' are the capture timestamp.
 * 'data' points to the captured packet (link-layer header + payload).
 * 'caplen' is the number of bytes captured.
 * 'origlen' is the original packet length on the wire.
 * Returns 0 on success, -1 on error.
 */
int pcapng_write_epb(FILE *fp, uint32_t iface_id, uint32_t ts_sec,
    uint32_t ts_usec, const void *data, uint32_t caplen, uint32_t origlen);

#endif /* !PCAPNG_H */
