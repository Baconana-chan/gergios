/* tests/driver_ffi/test_e1000.cpp
 *
 * Phase 9.1: Driver FFI integration tests — e1000 register layout.
 *
 * Verifies Intel PRO/1000 register offsets and bitfield constants via
 * FFI calls into the e1000 staticlib, comparing against the C header
 * definitions in rust/e1000/include/e1000.h.
 *
 * These tests ensure that register layout constants are consistent
 * between the Rust driver implementation (rust/e1000/src/reg.rs) and
 * the C reference header.
 */

#include <catch.hpp>
#include <cstdint>
#include <cstdio>

extern "C" {
#include <e1000.h>
}

/* =================================================================
 * Helper: hex formatting for readable assertions
 * ================================================================= */

static std::string hex(uint32_t val) {
    char buf[16];
    std::snprintf(buf, sizeof(buf), "0x%08X", val);
    return std::string(buf);
}

/* =================================================================
 * Version
 * ================================================================= */

TEST_CASE("e1000_test_version returns device ID family",
          "[e1000][registers][version]") {
    uint32_t version = e1000_test_version();
    REQUIRE(version == 0x1008254E);
}

/* =================================================================
 * Register Offsets — Full sweep
 * ================================================================= */

TEST_CASE("All 35 register byte offsets match e1000 hardware spec",
          "[e1000][registers][offsets]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {E1000_REG_CTRL,     E1000_CTRL_OFFSET,     "CTRL"},
        {E1000_REG_STATUS,   E1000_STATUS_OFFSET,   "STATUS"},
        {E1000_REG_EERD,     E1000_EERD_OFFSET,     "EERD"},
        {E1000_REG_FCAL,     E1000_FCAL_OFFSET,     "FCAL"},
        {E1000_REG_FCAH,     E1000_FCAH_OFFSET,     "FCAH"},
        {E1000_REG_FCT,      E1000_FCT_OFFSET,      "FCT"},
        {E1000_REG_FCTTV,    E1000_FCTTV_OFFSET,    "FCTTV"},
        {E1000_REG_ICR,      E1000_ICR_OFFSET,      "ICR"},
        {E1000_REG_IMS,      E1000_IMS_OFFSET,      "IMS"},
        {E1000_REG_RCTL,     E1000_RCTL_OFFSET,     "RCTL"},
        {E1000_REG_TCTL,     E1000_TCTL_OFFSET,     "TCTL"},
        {E1000_REG_RDBAL,    E1000_RDBAL_OFFSET,    "RDBAL"},
        {E1000_REG_RDBAH,    E1000_RDBAH_OFFSET,    "RDBAH"},
        {E1000_REG_RDLEN,    E1000_RDLEN_OFFSET,    "RDLEN"},
        {E1000_REG_RDH,      E1000_RDH_OFFSET,      "RDH"},
        {E1000_REG_RDT,      E1000_RDT_OFFSET,      "RDT"},
        {E1000_REG_TDBAL,    E1000_TDBAL_OFFSET,    "TDBAL"},
        {E1000_REG_TDBAH,    E1000_TDBAH_OFFSET,    "TDBAH"},
        {E1000_REG_TDLEN,    E1000_TDLEN_OFFSET,    "TDLEN"},
        {E1000_REG_TDH,      E1000_TDH_OFFSET,      "TDH"},
        {E1000_REG_TDT,      E1000_TDT_OFFSET,      "TDT"},
        {E1000_REG_CRCERRS,  E1000_CRCERRS_OFFSET,  "CRCERRS"},
        {E1000_REG_RXERRC,   E1000_RXERRC_OFFSET,   "RXERRC"},
        {E1000_REG_MPC,      E1000_MPC_OFFSET,      "MPC"},
        {E1000_REG_COLC,     E1000_COLC_OFFSET,     "COLC"},
        {E1000_REG_TPR,      E1000_TPR_OFFSET,      "TPR"},
        {E1000_REG_TPT,      E1000_TPT_OFFSET,      "TPT"},
        {E1000_REG_RAL,      E1000_RAL_OFFSET,      "RAL"},
        {E1000_REG_RAH,      E1000_RAH_OFFSET,      "RAH"},
        {E1000_REG_MTA,      E1000_MTA_OFFSET,      "MTA"},
        {E1000_REG_IVAR,     E1000_IVAR_OFFSET,     "IVAR"},
        {E1000_REG_EICR,     E1000_EICR_OFFSET,     "EICR"},
        {E1000_REG_EIAC,     E1000_EIAC_OFFSET,     "EIAC"},
        {E1000_REG_EIMS,     E1000_EIMS_OFFSET,     "EIMS"},
        {E1000_REG_EIMC,     E1000_EIMC_OFFSET,     "EIMC"},
    };

    for (const auto &c : cases) {
        uint32_t rust_offset = e1000_test_reg_offset(c.id);
        INFO("Register: " << c.name
             << " | Rust: " << hex(rust_offset)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_offset == c.expected);
    }
}

TEST_CASE("Unknown register ID returns 0xFFFFFFFF",
          "[e1000][registers][error]") {
    REQUIRE(e1000_test_reg_offset(99) == 0xFFFFFFFF);
    REQUIRE(e1000_test_reg_offset(0xFF) == 0xFFFFFFFF);
}

/* =================================================================
 * CTRL Register Bitfields
 * ================================================================= */

TEST_CASE("CTRL bitfields match e1000 hardware spec",
          "[e1000][bitfields][ctrl]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {E1000_BF_CTRL_LRST,     E1000_CTRL_LRST,     "CTRL.LRST"},
        {E1000_BF_CTRL_ASDE,     E1000_CTRL_ASDE,     "CTRL.ASDE"},
        {E1000_BF_CTRL_SLU,      E1000_CTRL_SLU,      "CTRL.SLU"},
        {E1000_BF_CTRL_ILOS,     E1000_CTRL_ILOS,     "CTRL.ILOS"},
        {E1000_BF_CTRL_RST,      E1000_CTRL_RST,      "CTRL.RST"},
        {E1000_BF_CTRL_VME,      E1000_CTRL_VME,      "CTRL.VME"},
        {E1000_BF_CTRL_PHY_RST,  E1000_CTRL_PHY_RST,  "CTRL.PHY_RST"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = e1000_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * STATUS Register Bitfields
 * ================================================================= */

TEST_CASE("STATUS bitfields match e1000 hardware spec",
          "[e1000][bitfields][status]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {E1000_BF_STATUS_FD,           E1000_STATUS_FD,           "STATUS.FD"},
        {E1000_BF_STATUS_LU,           E1000_STATUS_LU,           "STATUS.LU"},
        {E1000_BF_STATUS_TXOFF,        E1000_STATUS_TXOFF,        "STATUS.TXOFF"},
        {E1000_BF_STATUS_SPEED,        E1000_STATUS_SPEED,        "STATUS.SPEED"},
        {E1000_BF_STATUS_SPEED_10,     E1000_STATUS_SPEED_10,     "STATUS.SPEED_10"},
        {E1000_BF_STATUS_SPEED_100,    E1000_STATUS_SPEED_100,    "STATUS.SPEED_100"},
        {E1000_BF_STATUS_SPEED_1000_A, E1000_STATUS_SPEED_1000_A, "STATUS.SPEED_1000_A"},
        {E1000_BF_STATUS_SPEED_1000_B, E1000_STATUS_SPEED_1000_B, "STATUS.SPEED_1000_B"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = e1000_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * EERD Register Bitfields
 * ================================================================= */

TEST_CASE("EERD bitfields match e1000 hardware spec",
          "[e1000][bitfields][eerd]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {E1000_BF_EERD_START, E1000_EERD_START, "EERD.START"},
        {E1000_BF_EERD_DONE,  E1000_EERD_DONE,  "EERD.DONE"},
        {E1000_BF_EERD_ADDR,  E1000_EERD_ADDR,  "EERD.ADDR"},
        {E1000_BF_EERD_DATA,  E1000_EERD_DATA,  "EERD.DATA"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = e1000_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * ICR Interrupt Bitfields
 * ================================================================= */

TEST_CASE("ICR interrupt cause bitfields match e1000 hardware spec",
          "[e1000][bitfields][icr]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {E1000_BF_ICR_TXDW, E1000_ICR_TXDW, "ICR.TXDW"},
        {E1000_BF_ICR_TXQE, E1000_ICR_TXQE, "ICR.TXQE"},
        {E1000_BF_ICR_LSC,  E1000_ICR_LSC,  "ICR.LSC"},
        {E1000_BF_ICR_RXO,  E1000_ICR_RXO,  "ICR.RXO"},
        {E1000_BF_ICR_RXT,  E1000_ICR_RXT,  "ICR.RXT"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = e1000_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * RCTL / TCTL Receive/Transmit Control Bitfields
 * ================================================================= */

TEST_CASE("RCTL receive control bitfields match e1000 hardware spec",
          "[e1000][bitfields][rctl]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {E1000_BF_RCTL_EN,   E1000_RCTL_EN,   "RCTL.EN"},
        {E1000_BF_RCTL_UPE,  E1000_RCTL_UPE,  "RCTL.UPE"},
        {E1000_BF_RCTL_MPE,  E1000_RCTL_MPE,  "RCTL.MPE"},
        {E1000_BF_RCTL_BAM,  E1000_RCTL_BAM,  "RCTL.BAM"},
        {E1000_BF_RCTL_BSIZE, E1000_RCTL_BSIZE, "RCTL.BSIZE"},
        {E1000_BF_RCTL_BSEX, E1000_RCTL_BSEX, "RCTL.BSEX"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = e1000_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

TEST_CASE("TCTL transmit control bitfields match e1000 hardware spec",
          "[e1000][bitfields][tctl]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {E1000_BF_TCTL_EN,  E1000_TCTL_EN,  "TCTL.EN"},
        {E1000_BF_TCTL_PSP, E1000_TCTL_PSP, "TCTL.PSP"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = e1000_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * Extended Interrupt (EICR, IVAR) and RAH Bitfields
 * ================================================================= */

TEST_CASE("EICR and IVAR bitfields match e1000 hardware spec",
          "[e1000][bitfields][msix]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {E1000_BF_RAH_AV,      E1000_RAH_AV,      "RAH.AV"},
        {E1000_BF_IVAR_VALID,  E1000_IVAR_VALID,  "IVAR.VALID"},
        {E1000_BF_EICR_RX0,    E1000_EICR_RX0,    "EICR.RX0"},
        {E1000_BF_EICR_TX0,    E1000_EICR_TX0,    "EICR.TX0"},
        {E1000_BF_EICR_OTHER,  E1000_EICR_OTHER,  "EICR.OTHER"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = e1000_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * Configuration & IVAR entry offset constants
 * ================================================================= */

TEST_CASE("Configuration constants match e1000 hardware spec",
          "[e1000][constants]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {E1000_BF_RXDESC_NR,         E1000_RXDESC_NR,         "RXDESC_NR"},
        {E1000_BF_TXDESC_NR,         E1000_TXDESC_NR,         "TXDESC_NR"},
        {E1000_BF_IOBUF_SIZE,        E1000_IOBUF_SIZE,        "IOBUF_SIZE"},
        {E1000_BF_EERD_READ_TIMEOUT, E1000_EERD_READ_TIMEOUT, "EERD_READ_TIMEOUT"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = e1000_test_bitfield(c.id);
        INFO("Constant: " << c.name
             << " | Rust: " << rust_val
             << " | C: " << c.expected);
        REQUIRE(rust_val == c.expected);
    }
}

TEST_CASE("IVAR entry offsets match e1000 hardware spec",
          "[e1000][constants][ivar]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {E1000_BF_IVAR_RX0,   E1000_IVAR_RX0,   "IVAR_RX0"},
        {E1000_BF_IVAR_TX0,   E1000_IVAR_TX0,   "IVAR_TX0"},
        {E1000_BF_IVAR_RX1,   E1000_IVAR_RX1,   "IVAR_RX1"},
        {E1000_BF_IVAR_TX1,   E1000_IVAR_TX1,   "IVAR_TX1"},
        {E1000_BF_IVAR_OTHER, E1000_IVAR_OTHER, "IVAR_OTHER"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = e1000_test_bitfield(c.id);
        INFO("IVAR offset: " << c.name
             << " | Rust: " << rust_val
             << " | C: " << c.expected);
        REQUIRE(rust_val == c.expected);
    }
}

/* =================================================================
 * Error cases
 * ================================================================= */

TEST_CASE("Unknown bitfield ID returns 0xFFFFFFFF",
          "[e1000][bitfields][error]") {
    REQUIRE(e1000_test_bitfield(99) == 0xFFFFFFFF);
    REQUIRE(e1000_test_bitfield(0xFF) == 0xFFFFFFFF);
}
