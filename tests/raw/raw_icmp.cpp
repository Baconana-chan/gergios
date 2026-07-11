//! Catch2 RAW ICMP socket tests (migrated from ATF test92)
//!
//! Phase 9.4: C Test Migration — ATF → Catch2
//! See planning/28_testing_framework_migration.md
//!
//! Validates ICMP header wire format, checksum calculation, and
//! echo request/reply encoding. These tests run on the host without
//! requiring a running MINIX kernel.

#include "catch.hpp"
#include <cstdint>
#include <cstring>
#include <vector>
#include <arpa/inet.h>

// ============================================================================
// ICMP header structures (from RFC 792)
// ============================================================================

// ICMP type codes
static constexpr uint8_t ICMP_ECHOREPLY   = 0;
static constexpr uint8_t ICMP_ECHO        = 8;
static constexpr uint8_t ICMP_UNREACH     = 3;
static constexpr uint8_t ICMP_SRCQUENCH   = 4;
static constexpr uint8_t ICMP_REDIRECT    = 5;
static constexpr uint8_t ICMP_TIME_EXCEEDED = 11;
static constexpr uint8_t ICMP_PARAMPROB   = 12;
static constexpr uint8_t ICMP_TIMESTAMP   = 13;
static constexpr uint8_t ICMP_TIMESTAMPREPLY = 14;
static constexpr uint8_t ICMP_INFO_REQUEST = 15;
static constexpr uint8_t ICMP_INFO_REPLY  = 16;

// ICMP destination unreachable codes
static constexpr uint8_t ICMP_UNREACH_NET         = 0;
static constexpr uint8_t ICMP_UNREACH_HOST        = 1;
static constexpr uint8_t ICMP_UNREACH_PROTOCOL    = 2;
static constexpr uint8_t ICMP_UNREACH_PORT        = 3;
static constexpr uint8_t ICMP_UNREACH_FRAG_NEEDED = 4;

struct [[gnu::packed]] IcmpHeader {
    uint8_t  type;
    uint8_t  code;
    uint16_t checksum;
};

struct [[gnu::packed]] IcmpEcho {
    IcmpHeader header;
    uint16_t   id;
    uint16_t   sequence;
    // payload follows...
};

struct [[gnu::packed]] IcmpUnreach {
    IcmpHeader header;
    uint16_t   unused;
    uint16_t   next_mtu;
    // original IP header + 8 bytes of datagram follow...
};

// ============================================================================
// Checksum: 16-bit one's complement (RFC 1071)
// ============================================================================

static uint16_t icmp_checksum(const uint8_t* data, size_t len) {
    uint32_t sum = 0;
    for (size_t i = 0; i + 1 < len; i += 2) {
        sum += static_cast<uint32_t>(data[i]) << 8 |
               static_cast<uint32_t>(data[i + 1]);
    }
    if (len & 1) {
        sum += static_cast<uint32_t>(data[len - 1]) << 8;
    }
    while (sum >> 16) {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    return static_cast<uint16_t>(~sum);
}

// ============================================================================
// Test cases — ICMP header wire encoding
// ============================================================================

TEST_CASE("ICMP Echo Request header encoding", "[raw][icmp][wire]") {
    IcmpEcho echo;
    echo.header.type = ICMP_ECHO;
    echo.header.code = 0;
    echo.header.checksum = 0;  // filled in after
    echo.id = htons(0x1234);
    echo.sequence = htons(1);

    uint8_t payload[8] = {0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07};

    uint8_t packet[sizeof(IcmpEcho) + 8];
    memcpy(packet, &echo, sizeof(IcmpEcho));
    memcpy(packet + sizeof(IcmpEcho), payload, 8);

    // Compute checksum over the whole packet
    uint16_t cksum = icmp_checksum(packet, sizeof(packet));
    // With checksum field = 0, the computed checksum should be non-zero
    REQUIRE(cksum != 0);
    REQUIRE(cksum != 0xFFFF);

    // Verify the checksum is correct: checksummed packet should yield 0
    IcmpEcho* echo_pkt = reinterpret_cast<IcmpEcho*>(packet);
    echo_pkt->header.checksum = cksum;
    uint16_t verify = icmp_checksum(packet, sizeof(packet));
    REQUIRE(verify == 0);
}

TEST_CASE("ICMP Echo Reply header encoding", "[raw][icmp][wire]") {
    IcmpEcho reply;
    reply.header.type = ICMP_ECHOREPLY;
    reply.header.code = 0;
    reply.header.checksum = 0;
    reply.id = htons(0x5678);
    reply.sequence = htons(42);

    uint8_t pkt[sizeof(IcmpEcho)];
    memcpy(pkt, &reply, sizeof(IcmpEcho));

    // Set checksum
    uint16_t cksum = icmp_checksum(pkt, sizeof(pkt));
    IcmpEcho* rp = reinterpret_cast<IcmpEcho*>(pkt);
    rp->header.checksum = cksum;

    // Verify
    REQUIRE(icmp_checksum(pkt, sizeof(pkt)) == 0);
}

TEST_CASE("ICMP Destination Unreachable header encoding", "[raw][icmp][wire]") {
    IcmpUnreach unreach;
    unreach.header.type = ICMP_UNREACH;
    unreach.header.code = ICMP_UNREACH_PORT;
    unreach.header.checksum = 0;
    unreach.unused = 0;
    unreach.next_mtu = htons(1500);

    uint8_t pkt[sizeof(IcmpUnreach)];
    memcpy(pkt, &unreach, sizeof(IcmpUnreach));

    uint16_t cksum = icmp_checksum(pkt, sizeof(pkt));
    IcmpUnreach* up = reinterpret_cast<IcmpUnreach*>(pkt);
    up->header.checksum = cksum;

    REQUIRE(icmp_checksum(pkt, sizeof(pkt)) == 0);

    // Verify fields
    up = reinterpret_cast<IcmpUnreach*>(pkt);
    REQUIRE(up->header.type == ICMP_UNREACH);
    REQUIRE(up->header.code == ICMP_UNREACH_PORT);
    REQUIRE(ntohs(up->next_mtu) == 1500);
}

TEST_CASE("ICMP checksum is endian-independent", "[raw][icmp][wire]") {
    // The checksum field is transmitted in network byte order,
    // but the ICMP checksum algorithm treats the data as 16-bit
    // big-endian words. Verify that swapping bytes in the payload
    // produces a different checksum.

    uint8_t pkt1[8] = {0x01, 0x02, 0x03, 0x04, 0x00, 0x00, 0x00, 0x00};
    uint8_t pkt2[8] = {0x02, 0x01, 0x04, 0x03, 0x00, 0x00, 0x00, 0x00};

    uint16_t cksum1 = icmp_checksum(pkt1, 8);
    uint16_t cksum2 = icmp_checksum(pkt2, 8);
    REQUIRE(cksum1 != cksum2);
}

TEST_CASE("ICMP Echo Request id/sequence 16-bit fields", "[raw][icmp][wire]") {
    // Maximum values
    {
        IcmpEcho echo;
        echo.header.type = ICMP_ECHO;
        echo.header.code = 0;
        echo.header.checksum = 0;
        echo.id = htons(65535);
        echo.sequence = htons(65535);

        uint8_t pkt[sizeof(IcmpEcho)];
        memcpy(pkt, &echo, sizeof(IcmpEcho));

        IcmpEcho* ep = reinterpret_cast<IcmpEcho*>(pkt);
        REQUIRE(ntohs(ep->id) == 65535);
        REQUIRE(ntohs(ep->sequence) == 65535);
    }

    // Zero values
    {
        IcmpEcho echo;
        echo.header.type = ICMP_ECHO;
        echo.header.code = 0;
        echo.header.checksum = 0;
        echo.id = 0;
        echo.sequence = 0;

        uint8_t pkt[sizeof(IcmpEcho)];
        memcpy(pkt, &echo, sizeof(IcmpEcho));

        IcmpEcho* ep = reinterpret_cast<IcmpEcho*>(pkt);
        REQUIRE(ntohs(ep->id) == 0);
        REQUIRE(ntohs(ep->sequence) == 0);
    }
}

TEST_CASE("ICMP type/code field ranges", "[raw][icmp][wire]") {
    // ICMP types 0-18 are defined; test each valid type
    for (uint8_t type = 0; type <= 18; type++) {
        for (uint8_t code = 0; code <= 15; code++) {
            IcmpHeader hdr;
            hdr.type = type;
            hdr.code = code;
            hdr.checksum = 0;

            uint8_t pkt[sizeof(IcmpHeader)];
            memcpy(pkt, &hdr, sizeof(IcmpHeader));
            uint16_t cksum = icmp_checksum(pkt, sizeof(pkt));

            IcmpHeader* hp = reinterpret_cast<IcmpHeader*>(pkt);
            hp->checksum = cksum;
            REQUIRE(icmp_checksum(pkt, sizeof(pkt)) == 0);
        }
    }
}

TEST_CASE("ICMP raw socket creation (MINIX)", "[raw][icmp][runtime][minix]") {
    // Requires MINIX: socket(AF_INET, SOCK_RAW, IPPROTO_ICMP)
    SKIP("MINIX runtime required — raw socket creation needs CAP_NET_RAW");
}

TEST_CASE("ICMP sendto raw socket (MINIX)", "[raw][icmp][runtime][minix]") {
    SKIP("MINIX runtime required — sendto on raw socket");
}

TEST_CASE("ICMP recvfrom raw socket (MINIX)", "[raw][icmp][runtime][minix]") {
    SKIP("MINIX runtime required — recvfrom on raw socket");
}

TEST_CASE("ICMP error: socket without CAP_NET_RAW (MINIX)", "[raw][icmp][runtime][minix]") {
    // On MINIX, creating a raw socket without CAP_NET_RAW returns EPERM
    SKIP("MINIX runtime required — capability check");
}
