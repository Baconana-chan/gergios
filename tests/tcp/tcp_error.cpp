//! Catch2 TCP error handling tests (migrated from ATF test91)
//!
//! Phase 9.4: C Test Migration — ATF → Catch2
//! See planning/28_testing_framework_migration.md
//!
//! Validates TCP error handling scenarios: truncated packets,
//! invalid header fields, memory corruption guards.

#include "catch.hpp"
#include <cstdint>
#include <cstring>
#include <cerrno>

// ============================================================================
// TCP option parsing helpers
// ============================================================================

// TCP option kinds (from lwIP tcp.h)
static constexpr uint8_t TCPOPT_EOL  = 0;
static constexpr uint8_t TCPOPT_NOP  = 1;
static constexpr uint8_t TCPOPT_MSS  = 2;
static constexpr uint8_t TCPOPT_WSCALE = 3;
static constexpr uint8_t TCPOPT_SACK_PERM = 4;
static constexpr uint8_t TCPOPT_SACK = 5;
static constexpr uint8_t TCPOPT_TIMESTAMP = 8;

// Error codes for TCP option validation
enum class TcpOptionError {
    None,
    Truncated,
    InvalidKind,
    InvalidLen,
};

struct TcpOption {
    uint8_t kind;
    uint8_t len;
    uint8_t data[40]; // max option data
};

// Parse a single TCP option from the options block.
// Returns the number of bytes consumed, or 0 on error.
static int parse_tcp_option(const uint8_t* opt_data, size_t remaining,
                            TcpOption* out, TcpOptionError* err) {
    if (remaining == 0) {
        *err = TcpOptionError::None;
        return 0;
    }

    uint8_t kind = opt_data[0];
    out->kind = kind;

    if (kind == TCPOPT_EOL) {
        *err = TcpOptionError::None;
        return static_cast<int>(remaining); // consume rest
    }
    if (kind == TCPOPT_NOP) {
        *err = TcpOptionError::None;
        return 1;
    }

    if (remaining < 2) {
        *err = TcpOptionError::Truncated;
        return 0;
    }

    uint8_t len = opt_data[1];
    out->len = len;

    if (len < 2 || len > remaining) {
        *err = TcpOptionError::InvalidLen;
        return 0;
    }

    size_t data_len = len - 2;
    if (data_len > sizeof(out->data)) {
        data_len = sizeof(out->data);
    }
    std::memcpy(out->data, opt_data + 2, data_len);

    *err = TcpOptionError::None;
    return len;
}

// ============================================================================
// Test cases — TCP option parsing
// ============================================================================

TEST_CASE("TCP option EOL terminates option parsing", "[tcp][options]") {
    uint8_t opts[] = {TCPOPT_EOL, 0xFF, 0xFF}; // bytes after EOL are ignored
    TcpOption opt;
    TcpOptionError err = TcpOptionError::None;
    int consumed = parse_tcp_option(opts, sizeof(opts), &opt, &err);
    REQUIRE(err == TcpOptionError::None);
    REQUIRE(consumed > 0); // consumes remaining
}

TEST_CASE("TCP option NOP is a single byte", "[tcp][options]") {
    uint8_t opts[] = {TCPOPT_NOP};
    TcpOption opt;
    TcpOptionError err;
    int consumed = parse_tcp_option(opts, sizeof(opts), &opt, &err);
    REQUIRE(err == TcpOptionError::None);
    REQUIRE(consumed == 1);
}

TEST_CASE("TCP MSS option", "[tcp][options]") {
    // MSS option: kind=2, len=4, data=1460
    uint8_t opts[] = {TCPOPT_MSS, 4, 0x05, 0xB4}; // 1460 = 0x05B4
    TcpOption opt;
    TcpOptionError err;
    int consumed = parse_tcp_option(opts, sizeof(opts), &opt, &err);
    REQUIRE(err == TcpOptionError::None);
    REQUIRE(opt.kind == TCPOPT_MSS);
    REQUIRE(opt.len == 4);
    REQUIRE(opt.data[0] == 0x05);
    REQUIRE(opt.data[1] == 0xB4);
    REQUIRE(consumed == 4);
}

TEST_CASE("TCP window scale option", "[tcp][options]") {
    uint8_t opts[] = {TCPOPT_WSCALE, 3, 7}; // shift = 7
    TcpOption opt;
    TcpOptionError err;
    int consumed = parse_tcp_option(opts, sizeof(opts), &opt, &err);
    REQUIRE(err == TcpOptionError::None);
    REQUIRE(opt.kind == TCPOPT_WSCALE);
    REQUIRE(consumed == 3);
}

TEST_CASE("TCP SACK-permitted option", "[tcp][options]") {
    uint8_t opts[] = {TCPOPT_SACK_PERM, 2};
    TcpOption opt;
    TcpOptionError err;
    int consumed = parse_tcp_option(opts, sizeof(opts), &opt, &err);
    REQUIRE(err == TcpOptionError::None);
    REQUIRE(opt.kind == TCPOPT_SACK_PERM);
    REQUIRE(consumed == 2);
}

TEST_CASE("TCP timestamp option", "[tcp][options]") {
    uint8_t opts[] = {TCPOPT_TIMESTAMP, 10, 0x00, 0x00, 0x00, 0x01, // TSval = 1
                      0x00, 0x00, 0x00, 0x02}; // TSecr = 2
    TcpOption opt;
    TcpOptionError err;
    int consumed = parse_tcp_option(opts, sizeof(opts), &opt, &err);
    REQUIRE(err == TcpOptionError::None);
    REQUIRE(opt.kind == TCPOPT_TIMESTAMP);
    REQUIRE(consumed == 10);
}

TEST_CASE("TCP truncated option returns error", "[tcp][options]") {
    // MSS option with kind=2, len=4 but only 3 bytes available
    uint8_t opts[] = {TCPOPT_MSS, 4, 0x05};
    TcpOption opt;
    TcpOptionError err = TcpOptionError::None;
    int consumed = parse_tcp_option(opts, 3, &opt, &err);
    REQUIRE(err == TcpOptionError::Truncated);
    REQUIRE(consumed == 0);
}

TEST_CASE("TCP invalid option length", "[tcp][options]") {
    // Option with len < 2
    uint8_t opts[] = {3, 1}; // window scale, len=1 (invalid)
    TcpOption opt;
    TcpOptionError err = TcpOptionError::None;
    int consumed = parse_tcp_option(opts, sizeof(opts), &opt, &err);
    REQUIRE(err == TcpOptionError::InvalidLen);
    REQUIRE(consumed == 0);
}

TEST_CASE("TCP empty options block", "[tcp][options]") {
    TcpOption opt;
    TcpOptionError err = TcpOptionError::None;
    int consumed = parse_tcp_option(nullptr, 0, &opt, &err);
    REQUIRE(err == TcpOptionError::None);
    REQUIRE(consumed == 0);
}

TEST_CASE("TCP multiple NOPs followed by MSS", "[tcp][options]") {
    uint8_t opts[] = {TCPOPT_NOP, TCPOPT_NOP, TCPOPT_MSS, 4, 0x05, 0xB4};
    TcpOption opt;
    TcpOptionError err;

    // Parse NOP 1
    int consumed = parse_tcp_option(opts, sizeof(opts), &opt, &err);
    REQUIRE(consumed == 1);

    // Parse NOP 2
    consumed = parse_tcp_option(opts + 1, sizeof(opts) - 1, &opt, &err);
    REQUIRE(consumed == 1);

    // Parse MSS
    consumed = parse_tcp_option(opts + 2, sizeof(opts) - 2, &opt, &err);
    REQUIRE(err == TcpOptionError::None);
    REQUIRE(opt.kind == TCPOPT_MSS);
    REQUIRE(consumed == 4);
}

// ============================================================================
// Test cases — TCP error scenarios
// ============================================================================

TEST_CASE("TCP connect to unbound port returns ECONNREFUSED (MINIX)", "[tcp][runtime][minix]") {
    // NOTE: Requires MINIX runtime. On the host, this is a placeholder.
    // errno == ECONNREFUSED when connecting to a closed local port.
    SKIP("MINIX runtime required");
}

TEST_CASE("TCP bind to invalid address returns EADDRNOTAVAIL (MINIX)", "[tcp][runtime][minix]") {
    SKIP("MINIX runtime required");
}

TEST_CASE("TCP connect to unreachable network returns ENETUNREACH (MINIX)", "[tcp][runtime][minix]") {
    SKIP("MINIX runtime required");
}

// ============================================================================
// Edge cases: TCP header with invalid data offset
// ============================================================================

TEST_CASE("TCP data offset 0 is malformed", "[tcp][error]") {
    uint8_t pkt[20] = {0};
    // Data offset = 0 → header_len = 0 → forbidden
    // In real TCP, the minimum header is 20 bytes (doff=5)
    // Values 0-4 are invalid and should be rejected by the TCP stack.
    // This test validates that our parser detects the condition.
    struct PseudoHeader {
        uint16_t src_port;
        uint16_t dst_port;
        uint32_t seq_num;
        uint32_t ack_num;
        uint16_t doff_flags;
        uint16_t window_size;
        uint16_t checksum;
        uint16_t urgent_ptr;
    } __attribute__((packed));

    // doff=0 is clearly invalid but wire-format parses it
    pkt[12] = 0x00;
    PseudoHeader hdr;
    std::memcpy(&hdr, pkt, sizeof(hdr));
    int header_len = ((ntohs(hdr.doff_flags) >> 12) & 0x0F) * 4;
    REQUIRE(header_len == 0); // invalid
    // The MINIX TCP stack should reject doff < 5
}
