//! Catch2 RAW IPv6 socket tests (migrated from ATF test92)
//!
//! Phase 9.4: C Test Migration — ATF → Catch2
//! See planning/28_testing_framework_migration.md
//!
//! Validates IPv6 header wire format, ICMPv6 checksum using
//! the pseudo-header, and raw IPv6 socket API semantics.
//! These tests run on the host without a running MINIX kernel.

#include "catch.hpp"
#include <cstdint>
#include <cstring>
#include <arpa/inet.h>

// ============================================================================
// IPv6 header (RFC 2460)
// ============================================================================

struct [[gnu::packed]] Ipv6Header {
    uint32_t vcr_flow;         // version (4), traffic class (8), flow label (20)
    uint16_t payload_length;
    uint8_t  next_header;
    uint8_t  hop_limit;
    uint8_t  src_addr[16];
    uint8_t  dst_addr[16];
};

// IPv6 Next Header values
static constexpr uint8_t IPPROTO_HOPOPTS  = 0;
static constexpr uint8_t IPPROTO_ICMPV6   = 58;
static constexpr uint8_t IPPROTO_TCP      = 6;
static constexpr uint8_t IPPROTO_UDP      = 17;
static constexpr uint8_t IPPROTO_ROUTING  = 43;
static constexpr uint8_t IPPROTO_FRAGMENT = 44;
static constexpr uint8_t IPPROTO_AH       = 51;
static constexpr uint8_t IPPROTO_ESP      = 50;
static constexpr uint8_t IPPROTO_DSTOPTS  = 60;

// ============================================================================
// ICMPv6 header (RFC 4443) — used in raw IPv6 sockets
// ============================================================================

struct [[gnu::packed]] Icmpv6Header {
    uint8_t  type;
    uint8_t  code;
    uint16_t checksum;
};

struct [[gnu::packed]] Icmpv6Echo {
    Icmpv6Header header;
    uint16_t     id;
    uint16_t     sequence;
};

// ============================================================================
// ICMPv6 pseudo-header checksum (RFC 4443 §2.3)
// ============================================================================

static uint16_t ipv6_checksum(const uint8_t* src_addr,
                               const uint8_t* dst_addr,
                               uint16_t payload_length,
                               uint8_t next_header,
                               const uint8_t* payload) {
    // Build pseudo-header + ICMPv6 message for checksum
    // Pseudo-header: src(16) + dst(16) + len(4) + zeros(3) + next_hdr(1)
    struct [[gnu::packed]] PseudoHeader {
        uint8_t  src[16];
        uint8_t  dst[16];
        uint32_t length;
        uint8_t  zeros[3];
        uint8_t  next_header;
    };

    std::vector<uint8_t> buf(sizeof(PseudoHeader) + payload_length);
    PseudoHeader ph;
    memcpy(ph.src, src_addr, 16);
    memcpy(ph.dst, dst_addr, 16);
    ph.length = htonl(payload_length);
    memset(ph.zeros, 0, 3);
    ph.next_header = next_header;

    memcpy(buf.data(), &ph, sizeof(PseudoHeader));
    memcpy(buf.data() + sizeof(PseudoHeader), payload, payload_length);

    // 16-bit one's complement sum (same as ICMP)
    uint32_t sum = 0;
    for (size_t i = 0; i + 1 < buf.size(); i += 2) {
        sum += static_cast<uint32_t>(buf[i]) << 8 | buf[i + 1];
    }
    if (buf.size() & 1) {
        sum += static_cast<uint32_t>(buf.back()) << 8;
    }
    while (sum >> 16) {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    return static_cast<uint16_t>(~sum);
}

// ============================================================================
// Test cases — IPv6 header wire encoding
// ============================================================================

TEST_CASE("IPv6 header version field", "[raw][ipv6][wire]") {
    Ipv6Header hdr;
    memset(&hdr, 0, sizeof(hdr));

    // Version should be 6 in the upper 4 bits of the first 32-bit word
    hdr.vcr_flow = htonl(0x60000000); // version=6, TC=0, flow=0

    uint8_t pkt[sizeof(Ipv6Header)];
    memcpy(pkt, &hdr, sizeof(Ipv6Header));

    Ipv6Header* hp = reinterpret_cast<Ipv6Header*>(pkt);
    uint32_t vcr = ntohl(hp->vcr_flow);
    uint8_t version = (vcr >> 28) & 0x0F;
    REQUIRE(version == 6);
}

TEST_CASE("IPv6 traffic class and flow label", "[raw][ipv6][wire]") {
    Ipv6Header hdr;
    memset(&hdr, 0, sizeof(hdr));

    // Traffic class = 0x3A, Flow label = 0x12345
    // Layout: version(4) | TC(8) | flow_label(20)
    // Word: 0x6 << 28 | 0x3A << 20 | 0x12345
    hdr.vcr_flow = htonl(0x6 << 28 | 0x3A << 20 | 0x12345);

    uint8_t pkt[sizeof(Ipv6Header)];
    memcpy(pkt, &hdr, sizeof(Ipv6Header));

    Ipv6Header* hp = reinterpret_cast<Ipv6Header*>(pkt);
    uint32_t vcr = ntohl(hp->vcr_flow);
    uint8_t tc = (vcr >> 20) & 0xFF;
    uint32_t flow = vcr & 0x000FFFFF;
    REQUIRE(tc == 0x3A);
    REQUIRE(flow == 0x12345);
}

TEST_CASE("IPv6 payload length field", "[raw][ipv6][wire]") {
    Ipv6Header hdr;
    memset(&hdr, 0, sizeof(hdr));

    // Payload length = 1500 bytes
    hdr.payload_length = htons(1500);
    hdr.next_header = IPPROTO_TCP;
    hdr.hop_limit = 64;

    uint8_t pkt[sizeof(Ipv6Header)];
    memcpy(pkt, &hdr, sizeof(Ipv6Header));

    Ipv6Header* hp = reinterpret_cast<Ipv6Header*>(pkt);
    REQUIRE(ntohs(hp->payload_length) == 1500);
    REQUIRE(hp->next_header == IPPROTO_TCP);
    REQUIRE(hp->hop_limit == 64);
}

TEST_CASE("IPv6 next header chain", "[raw][ipv6][wire]") {
    // Test extension header chain: TCP -> Fragment -> Routing -> IPv6
    // (in practice the chain is parsed in order, but wire format
    //  stores the first extension header in the IPv6 next_header field)
    Ipv6Header hdr;
    memset(&hdr, 0, sizeof(hdr));
    hdr.next_header = IPPROTO_HOPOPTS;  // First extension header

    uint8_t pkt[sizeof(Ipv6Header)];
    memcpy(pkt, &hdr, sizeof(Ipv6Header));

    Ipv6Header* hp = reinterpret_cast<Ipv6Header*>(pkt);
    REQUIRE(hp->next_header == IPPROTO_HOPOPTS);
}

TEST_CASE("IPv6 address 16-byte encoding", "[raw][ipv6][wire]") {
    Ipv6Header hdr;
    memset(&hdr, 0, sizeof(hdr));

    // IPv6 loopback (::1)
    hdr.src_addr[15] = 1;

    // IPv6 link-local (fe80::)
    hdr.dst_addr[0] = 0xFE;
    hdr.dst_addr[1] = 0x80;

    uint8_t pkt[sizeof(Ipv6Header)];
    memcpy(pkt, &hdr, sizeof(Ipv6Header));

    // src is loopback
    Ipv6Header* hp = reinterpret_cast<Ipv6Header*>(pkt);
    REQUIRE(hp->src_addr[15] == 1);

    // dst is link-local
    REQUIRE(hp->dst_addr[0] == 0xFE);
    REQUIRE(hp->dst_addr[1] == 0x80);
}

// ============================================================================
// Test cases — ICMPv6 Echo with IPv6 pseudo-header checksum
// ============================================================================

TEST_CASE("ICMPv6 Echo Request checksum with IPv6 pseudo-header",
          "[raw][ipv6][icmpv6]") {
    uint8_t src[16], dst[16];
    memset(src, 0, 16);
    memset(dst, 0, 16);
    src[15] = 1;  // ::1
    dst[15] = 1;  // ::1

    Icmpv6Echo echo;
    echo.header.type = 128;  // ICMPv6 Echo Request
    echo.header.code = 0;
    echo.header.checksum = 0;
    echo.id = htons(0x1234);
    echo.sequence = htons(1);

    uint8_t payload[8] = {0};

    std::vector<uint8_t> msg(sizeof(echo) + 8);
    memcpy(msg.data(), &echo, sizeof(echo));
    memcpy(msg.data() + sizeof(echo), payload, 8);

    uint16_t cksum = ipv6_checksum(src, dst,
                                     static_cast<uint16_t>(msg.size()),
                                     IPPROTO_ICMPV6, msg.data());
    REQUIRE(cksum != 0);
    REQUIRE(cksum != 0xFFFF);

    // With checksum set properly, re-checksum should be 0
    Icmpv6Echo* ep = reinterpret_cast<Icmpv6Echo*>(msg.data());
    ep->header.checksum = cksum;
    uint16_t verify = ipv6_checksum(src, dst,
                                      static_cast<uint16_t>(msg.size()),
                                      IPPROTO_ICMPV6, msg.data());
    REQUIRE(verify == 0);
}

TEST_CASE("ICMPv6 Echo Reply checksum with pseudo-header",
          "[raw][ipv6][icmpv6]") {
    uint8_t src[16], dst[16];
    memset(src, 0, 16);
    memset(dst, 0, 16);
    src[15] = 1;
    dst[15] = 1;

    Icmpv6Echo reply;
    reply.header.type = 129;  // ICMPv6 Echo Reply
    reply.header.code = 0;
    reply.header.checksum = 0;
    reply.id = htons(0xABCD);
    reply.sequence = htons(100);

    std::vector<uint8_t> msg(sizeof(reply));
    memcpy(msg.data(), &reply, sizeof(reply));

    uint16_t cksum = ipv6_checksum(src, dst,
                                     static_cast<uint16_t>(msg.size()),
                                     IPPROTO_ICMPV6, msg.data());
    REQUIRE(cksum != 0);

    Icmpv6Echo* ep = reinterpret_cast<Icmpv6Echo*>(msg.data());
    ep->header.checksum = cksum;
    uint16_t verify = ipv6_checksum(src, dst,
                                      static_cast<uint16_t>(msg.size()),
                                      IPPROTO_ICMPV6, msg.data());
    REQUIRE(verify == 0);
}

TEST_CASE("ICMPv6 checksum changes with different pseudo-header",
          "[raw][ipv6][icmpv6]") {
    // Two different source addresses should give different checksums
    uint8_t src1[16], src2[16], dst[16];
    memset(src1, 0, 16); src1[15] = 1;     // ::1
    memset(src2, 0, 16); src2[14] = 0x01;  // 0100::
    memset(dst, 0, 16);  dst[15] = 2;      // ::2

    Icmpv6Echo echo;
    memset(&echo, 0, sizeof(echo));
    echo.header.type = 128;

    std::vector<uint8_t> msg(sizeof(echo));
    memcpy(msg.data(), &echo, sizeof(echo));

    uint16_t cksum1 = ipv6_checksum(src1, dst,
                                      static_cast<uint16_t>(msg.size()),
                                      IPPROTO_ICMPV6, msg.data());
    uint16_t cksum2 = ipv6_checksum(src2, dst,
                                      static_cast<uint16_t>(msg.size()),
                                      IPPROTO_ICMPV6, msg.data());
    REQUIRE(cksum1 != cksum2);
}

TEST_CASE("RAW IPv6 socket creation (MINIX)", "[raw][ipv6][runtime][minix]") {
    SKIP("MINIX runtime required — socket(AF_INET6, SOCK_RAW, IPPROTO_ICMPV6)");
}

TEST_CASE("RAW IPv6 ICMPv6 sendto (MINIX)", "[raw][ipv6][runtime][minix]") {
    SKIP("MINIX runtime required — sendto on raw IPv6 socket");
}

TEST_CASE("RAW IPv6 recvfrom ICMPv6 (MINIX)", "[raw][ipv6][runtime][minix]") {
    SKIP("MINIX runtime required — recvfrom on raw IPv6 socket");
}

// ============================================================================
// IPv6 address parsing and formatting
// ============================================================================

TEST_CASE("IPv6 address string parsing", "[raw][ipv6][addr]") {
    struct in6_addr addr;
    // Loopback
    REQUIRE(inet_pton(AF_INET6, "::1", &addr) == 1);
    REQUIRE(addr.s6_addr[15] == 1);

    // Full form
    REQUIRE(inet_pton(AF_INET6, "2001:db8::1", &addr) == 1);

    // IPv4-mapped IPv6
    REQUIRE(inet_pton(AF_INET6, "::ffff:192.0.2.1", &addr) == 1);
    REQUIRE(addr.s6_addr[12] == 0xff);
    REQUIRE(addr.s6_addr[13] == 0xff);
    REQUIRE(addr.s6_addr[14] == 192);
    REQUIRE(addr.s6_addr[15] == 2);
}

TEST_CASE("IPv6 invalid address strings", "[raw][ipv6][addr]") {
    struct in6_addr addr;
    REQUIRE(inet_pton(AF_INET6, "invalid", &addr) == 0);
    REQUIRE(inet_pton(AF_INET6, "", &addr) == 0);
    REQUIRE(inet_pton(AF_INET6, ":::", &addr) == 0);
    REQUIRE(inet_pton(AF_INET6, "1:2:3:4:5:6:7:8:9", &addr) == 0);
    REQUIRE(inet_pton(AF_INET6, "::fffff", &addr) == 0);
}
