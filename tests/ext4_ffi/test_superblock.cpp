/* tests/ext4_ffi/test_superblock.cpp
 *
 * Phase 9.1: First C FFI integration test for ext4-core.
 * Tests ext4_parse_superblock() with valid and invalid superblock data.
 *
 * Byte offsets verified against Linux kernel struct ext4_super_block
 * and rust/ext4-core/src/superblock.rs.
 */

#include <catch.hpp>
#include <cstring>
#include <cstdint>
#include <vector>

extern "C" {
#include <ext4.h>
}

#include "helpers.h"

/* =================================================================
 * Test Cases
 * ================================================================= */

TEST_CASE("ext4_parse_superblock rejects null pointers", "[ext4][superblock]") {
    int ret;

    ret = ext4_parse_superblock(nullptr, nullptr);
    REQUIRE(ret == 22); // EINVAL

    ext4_sb_info sbi;
    ret = ext4_parse_superblock(nullptr, &sbi);
    REQUIRE(ret == 22);

    uint8_t dummy[1024] = {};
    ret = ext4_parse_superblock(dummy, nullptr);
    REQUIRE(ret == 22);
}

TEST_CASE("ext4_parse_superblock rejects invalid magic", "[ext4][superblock]") {
    uint8_t data[1024] = {};
    ext4_sb_info sbi;

    // All zeros — no magic
    int ret = ext4_parse_superblock(data, &sbi);
    REQUIRE(ret != 0);  // Should fail (no magic)
}

TEST_CASE("ext4_parse_superblock parses a valid minimal superblock",
          "[ext4][superblock]") {
    uint8_t data[2048];
    fill_valid_superblock(data, sizeof(data));

    ext4_sb_info sbi;
    int ret = ext4_parse_superblock(data, &sbi);

    REQUIRE(ret == 0);
    REQUIRE(sbi.block_size == 4096);
    REQUIRE(sbi.blocks_count == 8192000);
    REQUIRE(sbi.inodes_count == 1024);
    REQUIRE(sbi.block_groups_count == 250);   // 8192000 / 32768 = 250
    REQUIRE(sbi.blocks_per_group == 32768);
    REQUIRE(sbi.inodes_per_group == 512);
    REQUIRE(sbi.inode_size == 256);
    REQUIRE(sbi.desc_size == 64);
    REQUIRE(sbi.first_ino == 11);
    // feature_incompat = FILETYPE | EXTENTS (required features)
    REQUIRE(sbi.has_extents == 1);
    REQUIRE(sbi.has_64bit == 0);     // No 64bit feature set
    REQUIRE(sbi.has_flex_bg == 0);   // No flex_bg feature set
    REQUIRE(sbi.state == 1);         // CLEAN
    REQUIRE((sbi.feature_incompat & 0x0042) == 0x0042);  // FILETYPE | EXTENTS
    REQUIRE(sbi.feature_ro_compat == 0);

    // Check UUID
    for (int i = 0; i < 16; i++) {
        REQUIRE(sbi.uuid[i] == static_cast<uint8_t>(i + 1));
    }

    // Check volume name
    REQUIRE(sbi.volume_name[0] == 'T');
    REQUIRE(sbi.volume_name[4] == 'V');

    // Check csum_seed — should be 0 since no metadata_csum feature
    REQUIRE(sbi.csum_seed == 0);
}

TEST_CASE("ext4_sb_info_size returns correct value", "[ext4][superblock]") {
    size_t sz = ext4_sb_info_size();
    REQUIRE(sz == sizeof(ext4_sb_info));
}

TEST_CASE("ext4_parse_superblock handles 1024-byte block size",
          "[ext4][superblock]") {
    uint8_t data[2048];
    fill_valid_superblock(data, sizeof(data));

    // Override s_log_block_size to 0 (= 1024 bytes)
    data[24] = 0;

    ext4_sb_info sbi;
    int ret = ext4_parse_superblock(data, &sbi);

    REQUIRE(ret == 0);
    REQUIRE(sbi.block_size == 1024);
}

TEST_CASE("ext4_parse_superblock detects EXTENTS feature",
          "[ext4][superblock]") {
    uint8_t data[2048];
    fill_valid_superblock(data, sizeof(data));

    // fill_valid_superblock already sets EXTENTS (0x40) via required features.
    // Verify has_extents == 1 and the EXTENTS bit is set.
    ext4_sb_info sbi;
    int ret = ext4_parse_superblock(data, &sbi);

    REQUIRE(ret == 0);
    REQUIRE(sbi.has_extents == 1);
    REQUIRE((sbi.feature_incompat & 0x40) != 0);
}

TEST_CASE("ext4_parse_superblock detects FLEX_BG feature",
          "[ext4][superblock]") {
    uint8_t data[2048];
    fill_valid_superblock(data, sizeof(data));

    // OR in FLEX_BG bit (0x0200) at offset 96-97 (LE)
    // Must not overwrite existing FILETYPE|EXTENTS (0x0042)!
    // 0x0042 | 0x0200 = 0x0242
    // LE: data[96] = 0x42, data[97] = 0x02
    data[97] |= 0x02;  // FLEX_BG = 0x0200 → second byte

    ext4_sb_info sbi;
    int ret = ext4_parse_superblock(data, &sbi);

    REQUIRE(ret == 0);
    REQUIRE(sbi.has_flex_bg == 1);
    REQUIRE((sbi.feature_incompat & 0x0200) != 0);
}

TEST_CASE("ext4_parse_superblock rejects unsupported feature",
          "[ext4][superblock]") {
    uint8_t data[2048];
    fill_valid_superblock(data, sizeof(data));

    // Set EXT4_FEATURE_INCOMPAT_EA_INODE bit (0x0400 at byte 97+1)
    // EA_INODE = 0x0400 → second byte = 0x04
    // This is an unsupported INCOMPAT feature
    data[97] |= 0x04;

    ext4_sb_info sbi;
    int ret = ext4_parse_superblock(data, &sbi);

    // Should fail with unsupported incompat feature
    REQUIRE(ret != 0);
}
