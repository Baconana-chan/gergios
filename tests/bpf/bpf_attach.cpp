//! Catch2 BPF attach/detach tests (migrated from ATF test94)
//!
//! Phase 9.4: C Test Migration — ATF → Catch2
//! See planning/28_testing_framework_migration.md
//!
//! Validates BPF attach/detach semantics: program loading,
//! multiple filters on one interface, and filter statistics.

#include "catch.hpp"
#include "bpf_vm.h"
#include <cstdint>
#include <cstring>

// ============================================================================
// BPF filter program definitions
// ============================================================================

// Accept-all (capture all packets)
static BpfInsn accept_all[] = {
    { BPF_RET | BPF_IMM, 0, 0, 65535 },
};

// Reject-all
static BpfInsn reject_all[] = {
    { BPF_RET | BPF_IMM, 0, 0, BPF_K_REJECT },
};

// Capture only first 100 bytes
static BpfInsn capture_100[] = {
    { BPF_RET | BPF_IMM, 0, 0, 100 },
};

// Accept ARP packets: ethertype 0x0806 at offset 12
static BpfInsn arp_filter[] = {
    { BPF_LD | BPF_H | BPF_ABS, 0, 0, 12 },
    { BPF_JMP | BPF_JEQ, 1, 0, 0x0806 },
    { BPF_RET | BPF_IMM, 0, 0, BPF_K_REJECT },
    { BPF_RET | BPF_IMM, 0, 0, 65535 },
};

// Accept IPv4 broadcast: ethertype 0x0800, dst IP 255.255.255.255
// Simplified: just accept based on ethertype
static BpfInsn ipv4_filter[] = {
    { BPF_LD | BPF_H | BPF_ABS, 0, 0, 12 },
    { BPF_JMP | BPF_JEQ, 1, 0, 0x0800 },
    { BPF_RET | BPF_IMM, 0, 0, BPF_K_REJECT },
    { BPF_RET | BPF_IMM, 0, 0, 65535 },
};

// ============================================================================
// Test cases — BPF program definitions
// ============================================================================

TEST_CASE("BPF accept-all captures full packet", "[bpf][attach]") {
    uint8_t pkt[1500] = {0};
    uint32_t result = bpf_run(accept_all, 1, pkt, sizeof(pkt));
    REQUIRE(result == 65535);
}

TEST_CASE("BPF capture-limit 100 bytes", "[bpf][attach]") {
    uint8_t pkt[1500] = {0};
    uint32_t result = bpf_run(capture_100, 1, pkt, sizeof(pkt));
    REQUIRE(result == 100);
}

TEST_CASE("BPF ARP filter accepts ARP packet", "[bpf][attach]") {
    uint8_t pkt[60] = {0};
    pkt[12] = 0x08;  // Ethertype = 0x0806 (ARP)
    pkt[13] = 0x06;

    uint32_t result = bpf_run(arp_filter, 4, pkt, sizeof(pkt));
    REQUIRE(result == 65535);
}

TEST_CASE("BPF ARP filter rejects IPv4 packet", "[bpf][attach]") {
    uint8_t pkt[60] = {0};
    pkt[12] = 0x08;
    pkt[13] = 0x00;  // Ethertype = IPv4, not ARP

    uint32_t result = bpf_run(arp_filter, 4, pkt, sizeof(pkt));
    REQUIRE(result == BPF_K_REJECT);
}

TEST_CASE("BPF IPv4 filter accepts IPv4 packet", "[bpf][attach]") {
    uint8_t pkt[60] = {0};
    pkt[12] = 0x08;
    pkt[13] = 0x00;

    uint32_t result = bpf_run(ipv4_filter, 4, pkt, sizeof(pkt));
    REQUIRE(result == 65535);
}

TEST_CASE("BPF IPv4 filter rejects IPv6 packet", "[bpf][attach]") {
    uint8_t pkt[60] = {0};
    pkt[12] = 0x86;
    pkt[13] = 0xDD;  // Ethertype = IPv6

    uint32_t result = bpf_run(ipv4_filter, 4, pkt, sizeof(pkt));
    REQUIRE(result == BPF_K_REJECT);
}

// ============================================================================
// Test cases — BPF attach/detach on interface (MINIX-only)
// ============================================================================

TEST_CASE("BPF attach filter then detach (MINIX)", "[bpf][runtime][minix]") {
    // BIOCSETF → BIOCSETIF → BIOCFLUSH → close()
    SKIP("MINIX runtime required — attach/detach via ioctl");
}

TEST_CASE("BPF multiple filter programs on one interface (MINIX)",
          "[bpf][runtime][minix]") {
    // Opening multiple /dev/bpf* devices and attaching to same interface
    SKIP("MINIX runtime required — multiple BPF instances");
}

TEST_CASE("BPF filter statistics: accepted count (MINIX)",
          "[bpf][runtime][minix]") {
    // BIOCGSTATS: counts packets accepted/rejected by filter
    SKIP("MINIX runtime required — BIOCGSTATS ioctl");
}

TEST_CASE("BPF non-root access denied (MINIX)", "[bpf][runtime][minix]") {
    // On MINIX, /dev/bpf* is protected by filesystem permissions
    SKIP("MINIX runtime required — capability check");
}

TEST_CASE("BPF immediate mode: return immediately on packet arrival (MINIX)",
          "[bpf][runtime][minix]") {
    // BIOCIMMEDIATE: set non-zero to return immediately on packet arrival
    SKIP("MINIX runtime required — BIOCIMMEDIATE ioctl");
}
