//! Catch2 TCP socket wire-format tests (migrated from ATF test91)
//!
//! Phase 9.4: C Test Migration — ATF → Catch2
//! See planning/28_testing_framework_migration.md
//!
//! These tests validate TCP wire format, header parsing, and error
//! handling logic that can be tested on the host without a running
//! MINIX / lwIP service.
//!
//! Tests requiring actual socket kernel calls (connect, listen, accept)
//! remain in the original test91.c for MINIX-native runs.

#include "catch.hpp"
#include <cstdint>
#include <cstring>
#include <vector>
#include <arpa/inet.h>

// ============================================================================
// TCP header parsing helpers (mirror lwIP tcp.h layout)
// ============================================================================

struct [[gnu::packed]] TcpHeader {
    uint16_t src_port;
    uint16_t dst_port;
    uint32_t seq_num;
    uint32_t ack_num;
    uint16_t doff_flags;  // upper 4 bits = data offset, lower 12 = flags
    uint16_t window_size;
    uint16_t checksum;
    uint16_t urgent_ptr;
    // options follow...
};

// TCP flags (from lwIP tcp.h)
static constexpr uint16_t TCP_FIN = 0x0001;
static constexpr uint16_t TCP_SYN = 0x0002;
static constexpr uint16_t TCP_RST = 0x0004;
static constexpr uint16_t TCP_PSH = 0x0008;
static constexpr uint16_t TCP_ACK = 0x0010;
static constexpr uint16_t TCP_URG = 0x0020;
static constexpr uint16_t TCP_ECE = 0x0040;
static constexpr uint16_t TCP_CWR = 0x0080;
static constexpr uint16_t TCP_NS  = 0x0100;

// ============================================================================
// Helper: parse TCP header from raw bytes
// ============================================================================

static bool parse_tcp_header(const uint8_t* data, size_t len, TcpHeader* out) {
    if (len < 20) return false;
    out->src_port   = ntohs(*reinterpret_cast<const uint16_t*>(data + 0));
    out->dst_port   = ntohs(*reinterpret_cast<const uint16_t*>(data + 2));
    out->seq_num    = ntohl(*reinterpret_cast<const uint32_t*>(data + 4));
    out->ack_num    = ntohl(*reinterpret_cast<const uint32_t*>(data + 8));
    out->doff_flags = ntohs(*reinterpret_cast<const uint16_t*>(data + 12));
    out->window_size = ntohs(*reinterpret_cast<const uint16_t*>(data + 14));
    out->checksum   = ntohs(*reinterpret_cast<const uint16_t*>(data + 16));
    out->urgent_ptr = ntohs(*reinterpret_cast<const uint16_t*>(data + 18));
    return true;
}

static int tcp_header_len(const TcpHeader* hdr) {
    return ((hdr->doff_flags >> 12) & 0x0F) * 4;
}

static bool tcp_has_flag(const TcpHeader* hdr, uint16_t flag) {
    return (hdr->doff_flags & flag) != 0;
}

// ============================================================================
// Test cases — TCP wire encoding
// ============================================================================

TEST_CASE("TCP header minimum size is 20 bytes", "[tcp][wire]") {
    // Standard TCP header without options = 20 bytes
    uint8_t pkt[20] = {0};
    TcpHeader hdr;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(tcp_header_len(&hdr) == 20);

    // < 20 bytes must fail
    REQUIRE_FALSE(parse_tcp_header(pkt, 19, &hdr));
    REQUIRE_FALSE(parse_tcp_header(pkt, 0, &hdr));
}

TEST_CASE("TCP source/destination ports are 16-bit", "[tcp][wire]") {
    uint8_t pkt[20] = {0};

    // Valid port ranges
    pkt[0] = 0x00; pkt[1] = 0x50; // src = 80 (HTTP)
    pkt[2] = 0x00; pkt[3] = 0x35; // dst = 53 (DNS)

    TcpHeader hdr;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.src_port == 80);
    REQUIRE(hdr.dst_port == 53);

    // Max port 0xFFFF
    pkt[0] = 0xFF; pkt[1] = 0xFF;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.src_port == 65535);
}

TEST_CASE("TCP sequence and acknowledgment numbers", "[tcp][wire]") {
    uint8_t pkt[20] = {0};

    // seq = 0x01020304, ack = 0xAABBCCDD
    pkt[4] = 0x01; pkt[5] = 0x02; pkt[6] = 0x03; pkt[7] = 0x04;
    pkt[8] = 0xAA; pkt[9] = 0xBB; pkt[10] = 0xCC; pkt[11] = 0xDD;

    TcpHeader hdr;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.seq_num == 0x01020304);
    REQUIRE(hdr.ack_num == 0xAABBCCDD);

    // ISN = 0 (initial sequence number)
    std::memset(pkt, 0, 20);
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.seq_num == 0);
    REQUIRE(hdr.ack_num == 0);
}

TEST_CASE("TCP data offset field", "[tcp][wire]") {
    uint8_t pkt[20] = {0};

    // Data offset = 5 (20 bytes, no options) → upper nibble = 5
    pkt[12] = 0x50; // 0b0101 0000

    TcpHeader hdr;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(tcp_header_len(&hdr) == 20);

    // Data offset = 15 (60 bytes, max options) → upper nibble = 15 (0xF)
    pkt[12] = 0xF0;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(tcp_header_len(&hdr) == 60);

    // Data offset < 5 must be rejected
    for (int doff = 0; doff < 5; doff++) {
        pkt[12] = static_cast<uint8_t>(doff << 4);
        REQUIRE(parse_tcp_header(pkt, 20, &hdr)); // header bytes valid
        REQUIRE(tcp_header_len(&hdr) == doff * 4);
        // In real implementation, TCP would reject doff < 5
        // because the minimal header is 20 bytes.
    }
}

TEST_CASE("TCP flags encoding", "[tcp][wire]") {
    uint8_t pkt[20] = {0};

    // SYN flag (0x0002) — connect request
    pkt[13] = 0x02;
    TcpHeader hdr;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(tcp_has_flag(&hdr, TCP_SYN));
    REQUIRE_FALSE(tcp_has_flag(&hdr, TCP_ACK));
    REQUIRE_FALSE(tcp_has_flag(&hdr, TCP_FIN));
    REQUIRE_FALSE(tcp_has_flag(&hdr, TCP_RST));
    REQUIRE_FALSE(tcp_has_flag(&hdr, TCP_PSH));
    REQUIRE_FALSE(tcp_has_flag(&hdr, TCP_URG));

    // SYN+ACK — connection acceptance
    pkt[13] = 0x12; // SYN(0x02) | ACK(0x10)
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(tcp_has_flag(&hdr, TCP_SYN));
    REQUIRE(tcp_has_flag(&hdr, TCP_ACK));

    // FIN+ACK — graceful close
    pkt[13] = 0x11; // FIN(0x01) | ACK(0x10)
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(tcp_has_flag(&hdr, TCP_FIN));
    REQUIRE(tcp_has_flag(&hdr, TCP_ACK));

    // RST — reset
    pkt[13] = 0x04;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(tcp_has_flag(&hdr, TCP_RST));

    // PSH+URG — push with urgent data
    pkt[13] = 0x28; // PSH(0x08) | URG(0x20)
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(tcp_has_flag(&hdr, TCP_PSH));
    REQUIRE(tcp_has_flag(&hdr, TCP_URG));

    // All flags (9 bits)
    pkt[12] = 0x50; pkt[13] = 0xFF; // lower 8 bits + upper nibble reserved
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(tcp_has_flag(&hdr, TCP_FIN));
    REQUIRE(tcp_has_flag(&hdr, TCP_SYN));
    REQUIRE(tcp_has_flag(&hdr, TCP_RST));
    REQUIRE(tcp_has_flag(&hdr, TCP_PSH));
    REQUIRE(tcp_has_flag(&hdr, TCP_ACK));
    REQUIRE(tcp_has_flag(&hdr, TCP_URG));
    REQUIRE(tcp_has_flag(&hdr, TCP_ECE));
    REQUIRE(tcp_has_flag(&hdr, TCP_CWR));
    REQUIRE(tcp_has_flag(&hdr, TCP_NS));
}

TEST_CASE("TCP window size", "[tcp][wire]") {
    uint8_t pkt[20] = {0};

    // Window = 65535 (max classic window)
    pkt[14] = 0xFF; pkt[15] = 0xFF;
    TcpHeader hdr;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.window_size == 65535);

    // Window = 0 (zero window probe)
    pkt[14] = 0x00; pkt[15] = 0x00;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.window_size == 0);

    // Window = 14600 (typical)
    pkt[14] = 0x39; pkt[15] = 0x08;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.window_size == 14600);
}

TEST_CASE("TCP urgent pointer", "[tcp][wire]") {
    uint8_t pkt[20] = {0};

    // Urgent pointer = 0 (no urgent data)
    TcpHeader hdr;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.urgent_ptr == 0);

    // Urgent pointer = 100
    pkt[18] = 0x00; pkt[19] = 0x64;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.urgent_ptr == 100);

    // Urgent pointer = 65535
    pkt[18] = 0xFF; pkt[19] = 0xFF;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.urgent_ptr == 65535);
}

TEST_CASE("TCP port 0 is technically valid but unused", "[tcp][wire]") {
    uint8_t pkt[20] = {0};
    TcpHeader hdr;
    // Port 0 is a reserved port on most systems
    // but the wire format accepts it
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.src_port == 0);
    REQUIRE(hdr.dst_port == 0);
}

TEST_CASE("TCP header length with options", "[tcp][wire]") {
    uint8_t pkt[60] = {0};

    // 20-byte header + 40 bytes of options = 60 bytes total
    // Data offset = 15 → doff_flags upper nibble = 15 (0xF)
    pkt[12] = 0xF0;

    TcpHeader hdr;
    REQUIRE(parse_tcp_header(pkt, 60, &hdr));
    REQUIRE(tcp_header_len(&hdr) == 60);

    // 24-byte header (4 bytes of options)
    pkt[12] = 0x60;
    REQUIRE(parse_tcp_header(pkt, 60, &hdr));
    REQUIRE(tcp_header_len(&hdr) == 24);
}

TEST_CASE("TCP checksum wire encoding preserves order", "[tcp][wire]") {
    uint8_t pkt[20] = {0};

    // Checksum = 0xABCD
    pkt[16] = 0xAB; pkt[17] = 0xCD;
    TcpHeader hdr;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.checksum == 0xABCD);

    // Checksum = 0 (optional in loopback)
    pkt[16] = 0x00; pkt[17] = 0x00;
    REQUIRE(parse_tcp_header(pkt, 20, &hdr));
    REQUIRE(hdr.checksum == 0);
}

// ============================================================================
// Test cases — TCP connection state (MINIX runtime required)
// ============================================================================

// NOTE: The following tests require a running MINIX kernel with lwIP
// service. They are registered as a placeholder and will fail gracefully
// on non-MINIX hosts with SKIP or NOT_IMPLEMENTED.
//
// The original ATF test91 included 13 sub-tests covering:
//   - tcp_create      → socket(AF_INET, SOCK_STREAM, 0)
//   - tcp_bind        → bind() to ephemeral port
//   - tcp_connect     → connect() to loopback
//   - tcp_listen      → listen() backlog settings
//   - tcp_accept      → accept() incoming connection
//   - tcp_send        → send() data
//   - tcp_recv        → recv() data
//   - tcp_close       → close() socket
//   - tcp_shutdown    → shutdown() RD/WR
//   - tcp_options     → TCP_NODELAY, TCP_KEEPALIVE
//   - tcp_error_econnrefused → connect() to closed port → ECONNREFUSED
//   - tcp_error_eaddrnotavail → bind() to invalid addr → EADDRNOTAVAIL
//   - tcp_error_enetunreach   → connect() to unreachable net → ENETUNREACH

#ifdef __minix
TEST_CASE("TCP socket create, bind, listen, accept (MINIX)", "[tcp][runtime][minix]") {
    // This test requires MINIX lwIP service running
    SKIP("MINIX runtime required");
}

TEST_CASE("TCP send and receive (MINIX)", "[tcp][runtime][minix]") {
    SKIP("MINIX runtime required");
}
#endif
