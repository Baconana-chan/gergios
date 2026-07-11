/* tests/driver_ffi/test_ahci.cpp
 *
 * Phase 9.1: Driver FFI integration tests — AHCI register layout.
 *
 * Verifies AHCI 1.3 register offsets and bitfield constants via FFI
 * calls into the rust_minix-ahci staticlib, comparing against the
 * C header definitions in rust/minix-ahci/include/ahci.h.
 *
 * These tests ensure that register layout constants are consistent
 * between the Rust driver implementation and the C reference header.
 */

#include <catch.hpp>
#include <cstdint>
#include <cstdio>

extern "C" {
#include <ahci.h>
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

TEST_CASE("ahci_test_version returns AHCI 1.3",
          "[ahci][registers][version]") {
    uint32_t version = ahci_test_version();
    REQUIRE(version == 0x01030000);
}

/* =================================================================
 * HBA Register Offsets
 * ================================================================= */

TEST_CASE("HBA register byte offsets match AHCI 1.3 spec",
          "[ahci][registers][hba]") {
    // Quick-lookup table: reg_id → expected offset
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {AHCI_REG_HBA_CAP,   AHCI_HBA_CAP_OFFSET,   "HBA_CAP"},
        {AHCI_REG_HBA_GHC,   AHCI_HBA_GHC_OFFSET,   "HBA_GHC"},
        {AHCI_REG_HBA_IS,    AHCI_HBA_IS_OFFSET,    "HBA_IS"},
        {AHCI_REG_HBA_PI,    AHCI_HBA_PI_OFFSET,    "HBA_PI"},
        {AHCI_REG_HBA_VS,    AHCI_HBA_VS_OFFSET,    "HBA_VS"},
        {AHCI_REG_HBA_CAP2,  AHCI_HBA_CAP2_OFFSET,  "HBA_CAP2"},
    };

    for (const auto &c : cases) {
        uint32_t rust_offset = ahci_test_reg_offset(c.id);
        INFO("Register: " << c.name
             << " | Rust: " << hex(rust_offset)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_offset == c.expected);
    }
}

TEST_CASE("Port register byte offsets match AHCI 1.3 spec",
          "[ahci][registers][port]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {AHCI_REG_PORT_CLB,   AHCI_PORT_CLB_OFFSET,   "PORT_CLB"},
        {AHCI_REG_PORT_CLBU,  AHCI_PORT_CLBU_OFFSET,  "PORT_CLBU"},
        {AHCI_REG_PORT_FB,    AHCI_PORT_FB_OFFSET,    "PORT_FB"},
        {AHCI_REG_PORT_FBU,   AHCI_PORT_FBU_OFFSET,   "PORT_FBU"},
        {AHCI_REG_PORT_IS,    AHCI_PORT_IS_OFFSET,    "PORT_IS"},
        {AHCI_REG_PORT_IE,    AHCI_PORT_IE_OFFSET,    "PORT_IE"},
        {AHCI_REG_PORT_CMD,   AHCI_PORT_CMD_OFFSET,   "PORT_CMD"},
        {AHCI_REG_PORT_TFD,   AHCI_PORT_TFD_OFFSET,   "PORT_TFD"},
        {AHCI_REG_PORT_SIG,   AHCI_PORT_SIG_OFFSET,   "PORT_SIG"},
        {AHCI_REG_PORT_SSTS,  AHCI_PORT_SSTS_OFFSET,  "PORT_SSTS"},
        {AHCI_REG_PORT_SCTL,  AHCI_PORT_SCTL_OFFSET,  "PORT_SCTL"},
        {AHCI_REG_PORT_SERR,  AHCI_PORT_SERR_OFFSET,  "PORT_SERR"},
        {AHCI_REG_PORT_SACT,  AHCI_PORT_SACT_OFFSET,  "PORT_SACT"},
        {AHCI_REG_PORT_CI,    AHCI_PORT_CI_OFFSET,    "PORT_CI"},
    };

    for (const auto &c : cases) {
        uint32_t rust_offset = ahci_test_reg_offset(c.id);
        INFO("Register: " << c.name
             << " | Rust: " << hex(rust_offset)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_offset == c.expected);
    }
}

TEST_CASE("Unknown register ID returns 0xFFFFFFFF",
          "[ahci][registers][error]") {
    REQUIRE(ahci_test_reg_offset(99) == 0xFFFFFFFF);
    REQUIRE(ahci_test_reg_offset(0xFF) == 0xFFFFFFFF);
}

/* =================================================================
 * HBA Bitfields
 * ================================================================= */

TEST_CASE("HBA capability bitfields match AHCI 1.3 spec",
          "[ahci][bitfields][hba]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {AHCI_BF_CAP_SNCQ,      AHCI_CAP_SNCQ,      "CAP.SNCQ"},
        {AHCI_BF_CAP_SCLO,      AHCI_CAP_SCLO,      "CAP.SCLO"},
        {AHCI_BF_CAP_NCS_MASK,  AHCI_CAP_NCS_MASK,  "CAP.NCS_MASK"},
        {AHCI_BF_GHC_AE,        AHCI_GHC_AE,        "GHC.AE"},
        {AHCI_BF_GHC_HR,        AHCI_GHC_HR,        "GHC.HR"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = ahci_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

TEST_CASE("Port interrupt status bitfields match AHCI 1.3 spec",
          "[ahci][bitfields][port_is]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {AHCI_BF_IS_TFES,  AHCI_IS_TFES,  "IS.TFES"},
        {AHCI_BF_IS_PRCS,  AHCI_IS_PRCS,  "IS.PRCS"},
        {AHCI_BF_IS_PCS,   AHCI_IS_PCS,   "IS.PCS"},
        {AHCI_BF_IS_DHRS,  AHCI_IS_DHRS,  "IS.DHRS"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = ahci_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

TEST_CASE("Port command/status bitfields match AHCI 1.3 spec",
          "[ahci][bitfields][port_cmd]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {AHCI_BF_CMD_ST,   AHCI_CMD_ST,   "CMD.ST"},
        {AHCI_BF_CMD_FRE,  AHCI_CMD_FRE,  "CMD.FRE"},
        {AHCI_BF_CMD_FR,   AHCI_CMD_FR,   "CMD.FR"},
        {AHCI_BF_CMD_CR,   AHCI_CMD_CR,   "CMD.CR"},
        {AHCI_BF_CMD_SUD,  AHCI_CMD_SUD,  "CMD.SUD"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = ahci_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

TEST_CASE("TFD, SSTS, SERR bitfields match AHCI 1.3 spec",
          "[ahci][bitfields][status]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {AHCI_BF_TFD_BSY,        AHCI_TFD_BSY,        "TFD.BSY"},
        {AHCI_BF_TFD_DRQ,        AHCI_TFD_DRQ,        "TFD.DRQ"},
        {AHCI_BF_TFD_ERR,        AHCI_TFD_ERR,        "TFD.ERR"},
        {AHCI_BF_SSTS_DET_PHY,   AHCI_SSTS_DET_PHY,   "SSTS.DET_PHY"},
        {AHCI_BF_SSTS_DET_NONE,  AHCI_SSTS_DET_NONE,  "SSTS.DET_NONE"},
        {AHCI_BF_SERR_DIAG_X,    AHCI_SERR_DIAG_X,    "SERR.DIAG_X"},
        {AHCI_BF_SERR_DIAG_N,    AHCI_SERR_DIAG_N,    "SERR.DIAG_N"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = ahci_test_bitfield(c.id);
        INFO("Bitfield: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

TEST_CASE("FIS and memory layout constants match AHCI 1.3 spec",
          "[ahci][bitfields][fis]") {
    struct { uint32_t id; uint32_t expected; const char *name; } cases[] = {
        {AHCI_BF_FIS_TYPE_H2D,  0x27,        "FIS.TYPE_H2D"},
        {AHCI_BF_FIS_DEV_LBA,   0x40,        "FIS.DEV_LBA"},
        {AHCI_BF_ATA_SECTOR,    AHCI_ATA_SECTOR_SIZE, "ATA_SECTOR_SIZE"},
        {AHCI_BF_MAX_PORTS,     AHCI_MAX_PORTS,       "MAX_PORTS"},
        {AHCI_BF_MAX_CMDS,      AHCI_MAX_CMDS,        "MAX_CMDS"},
    };

    for (const auto &c : cases) {
        uint32_t rust_val = ahci_test_bitfield(c.id);
        INFO("Constant: " << c.name
             << " | Rust: " << hex(rust_val)
             << " | C: " << hex(c.expected));
        REQUIRE(rust_val == c.expected);
    }
}

TEST_CASE("Unknown bitfield ID returns 0xFFFFFFFF",
          "[ahci][bitfields][error]") {
    REQUIRE(ahci_test_bitfield(99) == 0xFFFFFFFF);
    REQUIRE(ahci_test_bitfield(0xFF) == 0xFFFFFFFF);
}

/* =================================================================
 * Memory layout constants
 * ================================================================= */

TEST_CASE("AHCI memory layout constants are consistent",
          "[ahci][memory]") {
    // These are compile-time constants — verify they make sense
    REQUIRE(AHCI_MEM_BASE_SIZE == 0x100);
    REQUIRE(AHCI_MEM_PORT_SIZE == 0x80);
    REQUIRE(AHCI_FIS_SIZE == 256);
    REQUIRE(AHCI_CL_SIZE == 1024);
    REQUIRE(AHCI_CT_SIZE > AHCI_CL_SIZE);
}
