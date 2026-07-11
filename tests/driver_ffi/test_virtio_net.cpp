/* tests/driver_ffi/test_virtio_net.cpp
 *
 * Phase 9.1: Driver FFI integration tests — virtio-net register/queue constants.
 *
 * Verifies virtio-legacy I/O port register offsets, status flags, vring
 * descriptor constants, and virtio-net feature bits via FFI calls into the
 * virtio_net staticlib.
 */

#include <catch.hpp>
#include <cstdint>
#include <cstdio>

extern "C" {
#include <virtio_net.h>
}

/* =================================================================
 * Helper: hex formatting for readable assertions
 * ================================================================= */

static std::string hex(uint32_t val) {
    char buf[16];
    std::snprintf(buf, sizeof(buf), "0x%04X", val);
    return std::string(buf);
}

/* =================================================================
 * Version
 * ================================================================= */

TEST_CASE("virtio_test_version returns legacy virtio 0.9.5",
          "[virtio][registers][version]") {
    REQUIRE(virtio_test_version() == 0x00010000);
}

/* =================================================================
 * Register Offsets — all 9 legacy I/O port registers
 * ================================================================= */

TEST_CASE("All 9 legacy virtio register offsets match spec",
          "[virtio][registers]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {VIRTIO_REG_HOST_F_OFF,      VIRTIO_HOST_F_OFF,      "HostFeatures"},
        {VIRTIO_REG_GUEST_F_OFF,     VIRTIO_GUEST_F_OFF,     "GuestFeatures"},
        {VIRTIO_REG_QADDR_OFF,       VIRTIO_QADDR_OFF,       "QueuePFN"},
        {VIRTIO_REG_QSIZE_OFF,       VIRTIO_QSIZE_OFF,       "QueueSize"},
        {VIRTIO_REG_QSEL_OFF,        VIRTIO_QSEL_OFF,        "QueueSelect"},
        {VIRTIO_REG_QNOTIFY_OFF,     VIRTIO_QNOTIFY_OFF,     "QueueNotify"},
        {VIRTIO_REG_DEV_STATUS_OFF,  VIRTIO_DEV_STATUS_OFF,  "DevStatus"},
        {VIRTIO_REG_ISR_STATUS_OFF,  VIRTIO_ISR_STATUS_OFF,  "ISRStatus"},
        {VIRTIO_REG_DEV_SPECIFIC_OFF, VIRTIO_DEV_SPECIFIC_OFF, "DevSpecific"},
    };

    for (const auto &c : cases) {
        uint32_t rust_off = virtio_test_reg_offset(c.id);
        INFO("Register: " << c.name
             << " | Rust: " << hex(rust_off)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_off == c.expected);
    }
}

TEST_CASE("Unknown virtio register ID returns 0xFFFFFFFF",
          "[virtio][registers][error]") {
    REQUIRE(virtio_test_reg_offset(99) == 0xFFFFFFFF);
    REQUIRE(virtio_test_reg_offset(0xFF) == 0xFFFFFFFF);
}

/* =================================================================
 * Device Status Flags
 * ================================================================= */

TEST_CASE("Virtio device status flags match spec",
          "[virtio][bitfields][status]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {VIRTIO_BF_STATUS_ACK,    VIRTIO_STATUS_ACK,    "STATUS_ACK"},
        {VIRTIO_BF_STATUS_DRV,    VIRTIO_STATUS_DRV,    "STATUS_DRV"},
        {VIRTIO_BF_STATUS_DRV_OK, VIRTIO_STATUS_DRV_OK, "STATUS_DRV_OK"},
        {VIRTIO_BF_STATUS_FAIL,   VIRTIO_STATUS_FAIL,   "STATUS_FAIL"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = virtio_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * Vring Descriptor Constants
 * ================================================================= */

TEST_CASE("Vring descriptor flags and struct sizes match spec",
          "[virtio][bitfields][vring]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {VIRTIO_BF_F_INDIRECT_DESC,    VIRTIO_F_INDIRECT_DESC,    "F_INDIRECT_DESC"},
        {VIRTIO_BF_VRING_DESC_F_NEXT,    VIRTIO_VRING_DESC_F_NEXT,    "VRING_DESC_F_NEXT"},
        {VIRTIO_BF_VRING_DESC_F_WRITE,   VIRTIO_VRING_DESC_F_WRITE,   "VRING_DESC_F_WRITE"},
        {VIRTIO_BF_VRING_DESC_F_INDIRECT, VIRTIO_VRING_DESC_F_INDIRECT, "VRING_DESC_F_INDIRECT"},
        {VIRTIO_BF_VRING_DESC_SIZE,        VIRTIO_VRING_DESC_SIZE,        "VringDesc size"},
        {VIRTIO_BF_VRING_USED_ELEM_SIZE,   VIRTIO_VRING_USED_ELEM_SIZE,  "VringUsedElem size"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = virtio_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << rust_val
             << " | C: " << c.expected);
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * Virtio-net Feature Bits
 * ================================================================= */

TEST_CASE("Virtio-net feature bit positions match spec",
          "[virtio][bitfields][features]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {VIRTIO_BF_NET_F_CSUM,      VIRTIO_NET_F_CSUM,      "CSUM"},
        {VIRTIO_BF_NET_F_GUEST_CSUM, VIRTIO_NET_F_GUEST_CSUM, "GUEST_CSUM"},
        {VIRTIO_BF_NET_F_MAC,       VIRTIO_NET_F_MAC,       "MAC"},
        {VIRTIO_BF_NET_F_GSO,       VIRTIO_NET_F_GSO,       "GSO"},
        {VIRTIO_BF_NET_F_STATUS,    VIRTIO_NET_F_STATUS,    "STATUS"},
        {VIRTIO_BF_NET_F_CTRL_VQ,   VIRTIO_NET_F_CTRL_VQ,   "CTRL_VQ"},
        {VIRTIO_BF_NET_F_MRG_RXBUF, VIRTIO_NET_F_MRG_RXBUF, "MRG_RXBUF"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = virtio_test_bitfield(c.id);
        INFO("Feature: " << c.name
             << " | Rust: " << rust_val
             << " | C: " << c.expected);
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * Virtio-net Link Status Flags
 * ================================================================= */

TEST_CASE("Virtio-net link status flags match spec",
          "[virtio][bitfields][link]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {VIRTIO_BF_NET_S_LINK_UP,  VIRTIO_NET_S_LINK_UP,  "LINK_UP"},
        {VIRTIO_BF_NET_S_ANNOUNCE, VIRTIO_NET_S_ANNOUNCE, "ANNOUNCE"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = virtio_test_bitfield(c.id);
        INFO("Status: " << c.name
             << " | Rust: " << rust_val
             << " | C: " << c.expected);
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * Virtio-net Header Constants
 * ================================================================= */

TEST_CASE("Virtio-net header sizes and flags match spec",
          "[virtio][bitfields][header]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {VIRTIO_BF_VIRTIO_NET_HDR_SIZE,      VIRTIO_VIRTIO_NET_HDR_SIZE,     "HdrSize"},
        {VIRTIO_BF_VIRTIO_NET_HDR_MRG_SIZE,  VIRTIO_VIRTIO_NET_HDR_MRG_SIZE, "HdrMrgSize"},
        {VIRTIO_BF_HDR_F_NEEDS_CSUM,  VIRTIO_HDR_F_NEEDS_CSUM,  "NEEDS_CSUM"},
        {VIRTIO_BF_HDR_F_DATA_VALID,  VIRTIO_HDR_F_DATA_VALID,  "DATA_VALID"},
        {VIRTIO_BF_HDR_GSO_NONE,      VIRTIO_HDR_GSO_NONE,      "GSO_NONE"},
        {VIRTIO_BF_HDR_GSO_TCPV4,     VIRTIO_HDR_GSO_TCPV4,     "GSO_TCPV4"},
        {VIRTIO_BF_HDR_GSO_TCPV6,     VIRTIO_HDR_GSO_TCPV6,     "GSO_TCPV6"},
        {VIRTIO_BF_HDR_GSO_ECN,       VIRTIO_HDR_GSO_ECN,       "GSO_ECN"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = virtio_test_bitfield(c.id);
        INFO("Constant: " << c.name
             << " | Rust: " << rust_val
             << " | C: " << c.expected);
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * Queue Indices and Driver Constants
 * ================================================================= */

TEST_CASE("Virtio-net queue indices and driver constants match spec",
          "[virtio][constants]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {VIRTIO_BF_RX_Q,           VIRTIO_RX_Q,           "RX_Q"},
        {VIRTIO_BF_TX_Q,           VIRTIO_TX_Q,           "TX_Q"},
        {VIRTIO_BF_CTRL_Q,         VIRTIO_CTRL_Q,         "CTRL_Q"},
        {VIRTIO_BF_BUF_PACKETS,    VIRTIO_BUF_PACKETS,    "BUF_PACKETS"},
        {VIRTIO_BF_MAX_PACK_SIZE,  VIRTIO_MAX_PACK_SIZE,  "MAX_PACK_SIZE"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = virtio_test_bitfield(c.id);
        INFO("Constant: " << c.name
             << " | Rust: " << rust_val
             << " | C: " << c.expected);
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * Error cases
 * ================================================================= */

TEST_CASE("Unknown virtio bitfield ID returns 0xFFFFFFFF",
          "[virtio][bitfields][error]") {
    REQUIRE(virtio_test_bitfield(99) == 0xFFFFFFFF);
    REQUIRE(virtio_test_bitfield(0xFF) == 0xFFFFFFFF);
}
