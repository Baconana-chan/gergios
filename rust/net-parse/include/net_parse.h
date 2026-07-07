/* rust/net-parse/include/net_parse.h — C header for net-parse FFI functions
 *
 * This header provides C-compatible declarations for calling safe Rust network
 * protocol parsers from C code. All functions perform bounds checking on input
 * buffers before accessing data.
 *
 * Usage:
 *   #include <net_parse.h>
 *
 *   struct TcpHeaderFFI tcp;
 *   int ret = net_parse_tcp_header(packet, len, &tcp);
 *   if (ret == 0) {
 *       // use parsed header fields
 *   }
 *
 * Return codes:
 *   0  (NET_PARSE_OK)        — parsing succeeded
 *   -1 (NET_PARSE_TRUNCATED) — buffer too short for header
 *   -2 (NET_PARSE_INVALID)   — invalid protocol data
 */

#ifndef MINIX_NET_PARSE_H
#define MINIX_NET_PARSE_H

#include <stdint.h>
#include <stddef.h>

/* ---------------------------------------------------------------------------
 * Return codes
 * ------------------------------------------------------------------------- */

#define NET_PARSE_OK        0
#define NET_PARSE_TRUNCATED (-1)
#define NET_PARSE_INVALID   (-2)

/* ---------------------------------------------------------------------------
 * TCP header (20 bytes + options)
 * ------------------------------------------------------------------------- */

struct TcpHeaderFFI {
    uint16_t src_port;      /* source port */
    uint16_t dst_port;      /* destination port */
    uint32_t seq_num;       /* sequence number */
    uint32_t ack_num;       /* acknowledgment number */
    uint8_t  data_offset;   /* data offset in 32-bit words (>=5) */
    uint8_t  flags;         /* TCP flags (FIN=0x01, SYN=0x02, RST=0x04,
                               PSH=0x08, ACK=0x10, URG=0x20, ECE=0x40, CWR=0x80) */
    uint16_t window_size;   /* receive window size */
    uint16_t checksum;      /* TCP checksum */
    uint16_t urgent_ptr;    /* urgent pointer */
};

/* ---------------------------------------------------------------------------
 * UDP header (8 bytes)
 * ------------------------------------------------------------------------- */

struct UdpHeaderFFI {
    uint16_t src_port;      /* source port */
    uint16_t dst_port;      /* destination port */
    uint16_t length;        /* datagram length (header + data) */
    uint16_t checksum;      /* UDP checksum */
};

/* ---------------------------------------------------------------------------
 * API functions
 * ------------------------------------------------------------------------- */

/* Parse a TCP header from raw bytes.
 * Returns NET_PARSE_OK on success, or a negative error code on failure. */
int net_parse_tcp_header(const uint8_t *buf, size_t buflen,
    struct TcpHeaderFFI *out);

/* Parse a UDP header from raw bytes.
 * Returns NET_PARSE_OK on success, or a negative error code on failure. */
int net_parse_udp_header(const uint8_t *buf, size_t buflen,
    struct UdpHeaderFFI *out);

/* Compute the Internet checksum (RFC 1071) over a data buffer.
 * Returns the 16-bit ones' complement checksum. */
uint16_t net_parse_checksum(const uint8_t *data, size_t len);

/* Verify an Internet checksum over a buffer (including checksum field).
 * Returns 1 if valid (sum folds to 0), 0 otherwise. */
int net_parse_checksum_verify(const uint8_t *data, size_t len);

#endif /* !MINIX_NET_PARSE_H */
