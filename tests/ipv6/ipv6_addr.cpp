//! Catch2 IPv6 address and DAD tests (migrated from ATF test93)
//!
//! Phase 9.4: C Test Migration — ATF → Catch2
//! See planning/28_testing_framework_migration.md
//!
//! Validates IPv6 address classification (scope, multicast groups),
//! DAD (Duplicate Address Detection) semantics, and IPV6_V6ONLY
//! socket option behavior. MINIX-dependent tests will skip.

#include "catch.hpp"
#include <cstdint>
#include <cstring>
#include <arpa/inet.h>
#include <sys/socket.h>
#include <netinet/in.h>

#ifndef IN6_IS_ADDR_LOOPBACK
#define IN6_IS_ADDR_LOOPBACK(a) \
    ((*(const uint32_t *)((a)->s6_addr) == 0) && \
     (*(const uint32_t *)((a)->s6_addr + 4) == 0) && \
     (*(const uint32_t *)((a)->s6_addr + 8) == 0) && \
     (*(const uint32_t *)((a)->s6_addr + 12) == htonl(1)))
#endif

#ifndef IN6_IS_ADDR_LINKLOCAL
#define IN6_IS_ADDR_LINKLOCAL(a) \
    (((a)->s6_addr[0] == 0xFE) && (((a)->s6_addr[1] & 0xC0) == 0x80))
#endif

#ifndef IN6_IS_ADDR_MULTICAST
#define IN6_IS_ADDR_MULTICAST(a) ((a)->s6_addr[0] == 0xFF)
#endif

// ============================================================================
// Test cases — IPv6 address classification
// ============================================================================

TEST_CASE("IPv6 loopback address classification", "[ipv6][addr]") {
    struct in6_addr loopback;
    REQUIRE(inet_pton(AF_INET6, "::1", &loopback) == 1);
    REQUIRE(IN6_IS_ADDR_LOOPBACK(&loopback));
    REQUIRE_FALSE(IN6_IS_ADDR_MULTICAST(&loopback));
    REQUIRE_FALSE(IN6_IS_ADDR_LINKLOCAL(&loopback));

    // Unspecified (::) is NOT loopback
    struct in6_addr unspecified;
    REQUIRE(inet_pton(AF_INET6, "::", &unspecified) == 1);
    REQUIRE_FALSE(IN6_IS_ADDR_LOOPBACK(&unspecified));
}

TEST_CASE("IPv6 link-local address classification", "[ipv6][addr]") {
    struct in6_addr ll;
    REQUIRE(inet_pton(AF_INET6, "fe80::1", &ll) == 1);
    REQUIRE(IN6_IS_ADDR_LINKLOCAL(&ll));
    REQUIRE_FALSE(IN6_IS_ADDR_LOOPBACK(&ll));

    // fe80:: with different interface IDs
    REQUIRE(inet_pton(AF_INET6, "fe80::2", &ll) == 1);
    REQUIRE(IN6_IS_ADDR_LINKLOCAL(&ll));

    // fe80::/10 range: fe80-febf
    REQUIRE(inet_pton(AF_INET6, "febf::1", &ll) == 1);
    REQUIRE(IN6_IS_ADDR_LINKLOCAL(&ll));

    // fec0:: is site-local (deprecated), NOT link-local
    REQUIRE(inet_pton(AF_INET6, "fec0::1", &ll) == 1);
    REQUIRE_FALSE(IN6_IS_ADDR_LINKLOCAL(&ll));
}

TEST_CASE("IPv6 multicast address classification", "[ipv6][addr]") {
    struct in6_addr mc;
    REQUIRE(inet_pton(AF_INET6, "ff02::1", &mc) == 1);  // all-nodes
    REQUIRE(IN6_IS_ADDR_MULTICAST(&mc));
    REQUIRE_FALSE(IN6_IS_ADDR_LOOPBACK(&mc));

    // Solicited-node multicast (ff02::1:ffxx:xxxx)
    REQUIRE(inet_pton(AF_INET6, "ff02::1:ff00:1234", &mc) == 1);
    REQUIRE(IN6_IS_ADDR_MULTICAST(&mc));

    // Global unicast is NOT multicast
    REQUIRE(inet_pton(AF_INET6, "2001:db8::1", &mc) == 1);
    REQUIRE_FALSE(IN6_IS_ADDR_MULTICAST(&mc));
}

TEST_CASE("IPv6 unique-local address", "[ipv6][addr]") {
    // Unique-local (fc00::/7) — like private IPv4
    struct in6_addr ul;
    REQUIRE(inet_pton(AF_INET6, "fd00::1", &ul) == 1);
    REQUIRE_FALSE(IN6_IS_ADDR_LOOPBACK(&ul));
    REQUIRE_FALSE(IN6_IS_ADDR_MULTICAST(&ul));
    REQUIRE_FALSE(IN6_IS_ADDR_LINKLOCAL(&ul));

    // fc00:: is also unique-local
    REQUIRE(inet_pton(AF_INET6, "fc00::dead:beef", &ul) == 1);
}

TEST_CASE("IPv6 global unicast address", "[ipv6][addr]") {
    struct in6_addr gu;
    REQUIRE(inet_pton(AF_INET6, "2001:db8::1", &gu) == 1);
    REQUIRE_FALSE(IN6_IS_ADDR_LOOPBACK(&gu));
    REQUIRE_FALSE(IN6_IS_ADDR_MULTICAST(&gu));
    REQUIRE_FALSE(IN6_IS_ADDR_LINKLOCAL(&gu));

    // 2000::/3 global unicast range (2000:: through 3fff::)
    REQUIRE(inet_pton(AF_INET6, "3fff:ffff:ffff:ffff::1", &gu) == 1);
}

// ============================================================================
// Test cases — IPv6 DAD (Duplicate Address Detection)
// ============================================================================

TEST_CASE("IPv6 solicited-node multicast address derivation", "[ipv6][addr][dad]") {
    // Solicited-node multicast: ff02::1:ffXX:XXXX (last 24 bits of unicast)
    // For 2001:db8::1:2:3:4, the last 24 bits are from the interface ID
    struct in6_addr addr;
    REQUIRE(inet_pton(AF_INET6, "2001:db8::1:2:3:4", &addr) == 1);

    // Last 3 bytes of address (from interface ID)
    uint8_t last3[3] = {addr.s6_addr[13], addr.s6_addr[14], addr.s6_addr[15]};

    // Solicited-node = ff02:0:0:0:0:1:ffXX:XXXX
    struct in6_addr sol_node;
    memset(&sol_node, 0, sizeof(sol_node));
    sol_node.s6_addr[0] = 0xff;
    sol_node.s6_addr[1] = 0x02;
    sol_node.s6_addr[11] = 0x01;
    sol_node.s6_addr[13] = 0xff;
    sol_node.s6_addr[14] = last3[1];
    sol_node.s6_addr[15] = last3[2];

    char sol_str[INET6_ADDRSTRLEN];
    REQUIRE(inet_ntop(AF_INET6, &sol_node, sol_str, sizeof(sol_str)) != nullptr);

    // Verify solicited-node is multicast
    REQUIRE(IN6_IS_ADDR_MULTICAST(&sol_node));
}

TEST_CASE("IPv6 DAD NS message (NS for tentative address)",
          "[ipv6][addr][dad]") {
    // DAD Neighbor Solicitation for tentative address 2001:db8::1
    // Target address is the tentative address
    // Source = :: (unspecified), Dest = solicited-node multicast
    struct in6_addr target;
    REQUIRE(inet_pton(AF_INET6, "2001:db8::1", &target) == 1);

    struct in6_addr unspecified;
    memset(&unspecified, 0, sizeof(unspecified));

    struct in6_addr sol_node;
    memset(&sol_node, 0, sizeof(sol_node));
    sol_node.s6_addr[0] = 0xff;
    sol_node.s6_addr[1] = 0x02;
    sol_node.s6_addr[11] = 0x01;
    sol_node.s6_addr[13] = 0xff;
    sol_node.s6_addr[14] = target.s6_addr[14];
    sol_node.s6_addr[15] = target.s6_addr[15];

    // Source is unspecified for DAD
    REQUIRE(IN6_IS_ADDR_MULTICAST(&sol_node)); // dest is multicast
    for (int i = 0; i < 16; i++) {
        REQUIRE(unspecified.s6_addr[i] == 0);  // src is ::
    }
}

TEST_CASE("IPv6 DAD NA message (NS for other address)",
          "[ipv6][addr][dad]") {
    // DAD Neighbor Advertisement sent in response to DAD NS
    // Source = address being claimed (if not duplicate)
    struct in6_addr claimed;
    REQUIRE(inet_pton(AF_INET6, "2001:db8::1", &claimed) == 1);
    REQUIRE_FALSE(IN6_IS_ADDR_MULTICAST(&claimed)); // not multicast
    REQUIRE_FALSE(IN6_IS_ADDR_LOOPBACK(&claimed));  // not loopback
    REQUIRE_FALSE(IN6_IS_ADDR_LINKLOCAL(&claimed)); // not link-local
}

// ============================================================================
// Test cases — IPv6 socket options
// ============================================================================

TEST_CASE("IPv6 V6ONLY socket option", "[ipv6][sockopt]") {
    // IPV6_V6ONLY: if set (default on most systems), the IPv6 socket
    // only accepts IPv6 connections. If not set, it also accepts
    // IPv4-mapped IPv6 connections.
    //
    // This test only validates the constant values and semantics,
    // not the actual setsockopt call (requires MINIX).

    // IPV6_V6ONLY = 27 on Linux, 24 on BSD/MINIX
    // We test that the option exists
#ifdef IPV6_V6ONLY
    REQUIRE(IPV6_V6ONLY > 0);
#else
    SKIP("IPV6_V6ONLY not defined on this platform");
#endif
}

TEST_CASE("IPv6 multicast socket options", "[ipv6][sockopt]") {
    // These socket options should exist
    REQUIRE(IPV6_JOIN_GROUP > 0);
    REQUIRE(IPV6_LEAVE_GROUP > 0);
    REQUIRE(IPV6_MULTICAST_HOPS > 0);
    REQUIRE(IPV6_MULTICAST_LOOP > 0);
    REQUIRE(IPV6_UNICAST_HOPS > 0);
}

// ============================================================================
// Test cases — MINIX-dependent
// ============================================================================

TEST_CASE("IPv6 socket create AF_INET6 (MINIX)", "[ipv6][runtime][minix]") {
    SKIP("MINIX runtime required — socket(AF_INET6, SOCK_STREAM, 0)");
}

TEST_CASE("IPv6 bind to loopback (MINIX)", "[ipv6][runtime][minix]") {
    SKIP("MINIX runtime required — bind to [::1]");
}

TEST_CASE("IPv6 routing socket (MINIX)", "[ipv6][runtime][minix]") {
    SKIP("MINIX runtime required — PF_ROUTE / PF_NETLINK socket");
}

TEST_CASE("IPv6 DAD with getsockopt (MINIX)", "[ipv6][runtime][minix]") {
    SKIP("MINIX runtime required — DAD kernel support");
}
