/* tests/ext4_ffi/helpers.h
 *
 * Shared helpers for ext4-core C FFI integration tests.
 * Phase 9.1 — See planning/28_testing_framework_migration.md
 */

#ifndef EXT4_FFI_HELPERS_H
#define EXT4_FFI_HELPERS_H

#include <cstring>
#include <cstdint>

/** The ext4 superblock magic number (little-endian at offset 56). */
static constexpr uint16_t EXT4_SUPER_MAGIC = 0xEF53;

/**
 * Fill a 1024-byte buffer with a minimal valid ext4 superblock.
 *
 * Byte offsets from Linux kernel struct ext4_super_block:
 *   see rust/ext4-core/src/superblock.rs for the canonical reference.
 */
inline void fill_valid_superblock(uint8_t *data, size_t size) {
    // Use REQUIRE only if Catch2 is available; otherwise just return
    if (size < 1024) return;
    std::memset(data, 0, size);

    // offset 0: s_inodes_count (u32 LE) = 1024
    uint32_t inodes = 1024;
    std::memcpy(&data[0], &inodes, sizeof(inodes));

    // offset 4: s_blocks_count_lo (u32 LE) = 8192000 (~31 GiB)
    uint32_t blocks = 8192000;
    std::memcpy(&data[4], &blocks, sizeof(blocks));

    // offset 12: s_free_blocks_count_lo (u32 LE) = 7892000
    uint32_t free_blocks = 7892000;
    std::memcpy(&data[12], &free_blocks, sizeof(free_blocks));

    // offset 16: s_free_inodes_count (u32 LE) = 824
    uint32_t free_inodes = 824;
    std::memcpy(&data[16], &free_inodes, sizeof(free_inodes));

    // offset 20: s_first_data_block (u32 LE) = 0 (block size > 1024)
    uint32_t first_data_block = 0;
    std::memcpy(&data[20], &first_data_block, sizeof(first_data_block));

    // offset 24: s_log_block_size (u32 LE) = 2 → 4096 bytes
    uint32_t log_block_size = 2;
    std::memcpy(&data[24], &log_block_size, sizeof(log_block_size));

    // offset 28: s_log_cluster_size (u32 LE) = 2
    uint32_t log_cluster_size = 2;
    std::memcpy(&data[28], &log_cluster_size, sizeof(log_cluster_size));

    // offset 32: s_blocks_per_group (u32 LE) = 32768
    uint32_t blocks_per_group = 32768;
    std::memcpy(&data[32], &blocks_per_group, sizeof(blocks_per_group));

    // offset 36: s_clusters_per_group (u32 LE) = 32768
    uint32_t clusters_per_group = 32768;
    std::memcpy(&data[36], &clusters_per_group, sizeof(clusters_per_group));

    // offset 40: s_inodes_per_group (u32 LE) = 512
    uint32_t inodes_per_group = 512;
    std::memcpy(&data[40], &inodes_per_group, sizeof(inodes_per_group));

    // offset 56: s_magic (u16 LE) = 0xEF53
    std::memcpy(&data[56], &EXT4_SUPER_MAGIC, sizeof(EXT4_SUPER_MAGIC));

    // offset 58: s_state (u16 LE) = 1 (clean)
    uint16_t state = 1;
    std::memcpy(&data[58], &state, sizeof(state));

    // offset 76: s_rev_level (u32 LE) = 1 (dynamic inode size)
    uint32_t rev_level = 1;
    std::memcpy(&data[76], &rev_level, sizeof(rev_level));

    // offset 84: s_first_ino (u32 LE) = 11
    uint32_t first_ino = 11;
    std::memcpy(&data[84], &first_ino, sizeof(first_ino));

    // offset 88: s_inode_size (u16 LE) = 256
    uint16_t inode_size = 256;
    std::memcpy(&data[88], &inode_size, sizeof(inode_size));

    // offset 92: s_feature_compat (u32 LE) = 0
    uint32_t feature_compat = 0;
    std::memcpy(&data[92], &feature_compat, sizeof(feature_compat));

    // offset 96: s_feature_incompat (u32 LE)
    // REQUIRED: FILETYPE (0x0002) | EXTENTS (0x0040) = 0x0042
    uint32_t feature_incompat = 0x0042;
    std::memcpy(&data[96], &feature_incompat, sizeof(feature_incompat));

    // offset 100: s_feature_ro_compat (u32 LE) = 0
    uint32_t feature_ro_compat = 0;
    std::memcpy(&data[100], &feature_ro_compat, sizeof(feature_ro_compat));

    // offset 104: s_uuid[16]
    for (int i = 0; i < 16; i++) {
        data[104 + i] = static_cast<uint8_t>(i + 1);
    }

    // offset 120: s_volume_name[16]
    const char *vol = "Test Volume";
    std::memcpy(&data[120], vol, std::strlen(vol));

    // offset 286: s_desc_size (u16 LE) = 64
    uint16_t desc_size = 64;
    std::memcpy(&data[286], &desc_size, sizeof(desc_size));
}

#endif /* EXT4_FFI_HELPERS_H */
