/* tests/ext4_ffi/test_inode.cpp
 *
 * Phase 9.1: C FFI integration tests for ext4-core inode operations.
 * Tests: ext4_read_inode, ext4_stat, ext4_chown, ext4_chmod, ext4_utime
 *
 * These tests construct a minimal in-memory ext4 filesystem image with:
 *   - Valid superblock (reusing fill_valid_superblock from test_superblock.cpp)
 *   - One group descriptor (group 0)
 *   - Block and inode bitmaps
 *   - Inode table with root inode (ino 2) and a test regular file (ino 12)
 *
 * Byte offsets verified against rust/ext4-core/src/:
 *   superblock.rs, group_desc.rs, inode.rs
 */

#include <catch.hpp>
#include <cstring>
#include <cstdint>
#include <cstdio>
#include <map>
#include <vector>
#include <algorithm>

extern "C" {
#include <ext4.h>
}

/* =================================================================
 * MockBlockDev — in-memory block device backed by a map of blocks
 * ================================================================= */

/**
 * Stores a map of block_number → block_data (block_size bytes each).
 * Provides static C callbacks for ext4 FFI read/write.
 */
class MockBlockDev {
public:
    explicit MockBlockDev(uint32_t block_size)
        : block_size_(block_size) {}

    /** Store a block (must be exactly block_size bytes). */
    void write_block(uint64_t block_nr, const uint8_t *data) {
        std::vector<uint8_t> block(data, data + block_size_);
        blocks_[block_nr] = std::move(block);
    }

    /** Read a block into a buffer. Returns 0 on success. */
    int read_block(uint64_t block_nr, uint8_t *buf) const {
        auto it = blocks_.find(block_nr);
        if (it == blocks_.end()) {
            // Block not stored → treat as zeroed (sparse)
            std::memset(buf, 0, block_size_);
            return 0;
        }
        std::memcpy(buf, it->second.data(), block_size_);
        return 0;
    }

    /** Static C callback for ext4_read_block_cb. */
    static int read_cb(void *ctx, uint64_t block_nr,
                       uint8_t *buf, uint32_t block_size) {
        auto *dev = static_cast<MockBlockDev *>(ctx);
        if (block_size != dev->block_size_) {
            return -1; // Size mismatch
        }
        return dev->read_block(block_nr, buf);
    }

    /** Static C callback for ext4_write_block_cb. */
    static int write_cb(void *ctx, uint64_t block_nr,
                        const uint8_t *buf, uint32_t block_size) {
        auto *dev = static_cast<MockBlockDev *>(ctx);
        if (block_size != dev->block_size_) {
            return -1;
        }
        std::vector<uint8_t> block(buf, buf + block_size);
        dev->blocks_[block_nr] = std::move(block);
        return 0;
    }

private:
    uint32_t block_size_;
    std::map<uint64_t, std::vector<uint8_t>> blocks_;
};

#include "helpers.h"

/* =================================================================
 * Ext4 constants (from rust/ext4-core/src/types.rs)
 * ================================================================= */

static constexpr uint16_t S_IFDIR  = 0x4000;
static constexpr uint16_t S_IFREG  = 0x8000;
static constexpr uint16_t S_IRWXU  = 0x01C0; /* rwx------ (0700) */
static constexpr uint16_t S_IRUSR  = 0x0100; /* r-------- */
static constexpr uint16_t S_IWUSR  = 0x0080; /* -w------- */
static constexpr uint16_t S_IXUSR  = 0x0040; /* --x------ */
static constexpr uint16_t S_IRGRP  = 0x0020; /* ---r----- */
static constexpr uint16_t S_IWGRP  = 0x0010; /* ----w---- */
static constexpr uint16_t S_IXGRP  = 0x0008; /* -----x--- */
static constexpr uint16_t S_IROTH  = 0x0004; /* ------r-- */
static constexpr uint16_t S_IWOTH  = 0x0002; /* -------w- */
static constexpr uint16_t S_IXOTH  = 0x0001; /* --------x */

static constexpr uint16_t ROOT_DIR_MODE  = S_IFDIR | S_IRWXU | S_IRGRP | S_IXGRP | S_IROTH | S_IXOTH; // 0x41ED
static constexpr uint16_t TEST_FILE_MODE = S_IFREG | S_IRUSR | S_IWUSR | S_IRGRP | S_IROTH;           // 0x81A4

static constexpr uint32_t EXT4_EXTENTS_FL = 0x00080000;
static constexpr uint16_t EXT4_EXTENT_MAGIC = 0xF30A;

/* Inode field offsets (within 256-byte inode) */
static constexpr int INODE_OFF_MODE        = 0;
static constexpr int INODE_OFF_UID         = 2;
static constexpr int INODE_OFF_SIZE_LO     = 4;
static constexpr int INODE_OFF_ATIME       = 8;
static constexpr int INODE_OFF_CTIME       = 12;
static constexpr int INODE_OFF_MTIME       = 16;
static constexpr int INODE_OFF_DTIME       = 20;
static constexpr int INODE_OFF_GID         = 24;
static constexpr int INODE_OFF_LINKS       = 26;
static constexpr int INODE_OFF_BLOCKS_LO   = 28;
static constexpr int INODE_OFF_FLAGS       = 32;
static constexpr int INODE_OFF_OSD1        = 36;
static constexpr int INODE_OFF_IBLOCK      = 40;   // 60 bytes
static constexpr int INODE_OFF_GEN         = 100;
static constexpr int INODE_OFF_FILE_ACL    = 104;
static constexpr int INODE_OFF_SIZE_HI     = 108;
static constexpr int INODE_OFF_BLOCKS_HI   = 116;  // inside i_osd2
static constexpr int INODE_EXTRA_ISIZE     = 128;  // 256-byte inode: extra isize at 128
static constexpr int INODE_EXTRA_CSUM      = 130;

/* =================================================================
 * Helpers: Build a minimal ext4 image in MockBlockDev
 * ================================================================= */

/**
 * Build a minimal ext4 filesystem image with one block group.
 *
 * Layout (block_size = 4096):
 *   Block 0:  Boot code (padding) + superblock at offset 1024
 *   Block 1:  Group descriptor table (64 bytes at offset 0)
 *   Block 2:  Block bitmap (all 1s = allocated)
 *   Block 3:  Inode bitmap (first 16 inodes allocated)
 *   Block 4:  Inode table (512 inodes × 256 bytes = 32 blocks)
 *     — Inode 2  (root dir)  at offset 256 within block 4
 *     — Inode 12 (test file) at offset 2816 within block 4
 *
 * @param dev     MockBlockDev to populate.
 * @param sbi     Output: parsed superblock info.
 */
static void build_minimal_ext4_image(MockBlockDev &dev, ext4_sb_info *sbi) {
    const uint32_t block_size = 4096;
    const uint32_t inodes_per_group = 512;
    const uint32_t blocks_per_group = 32768;
    const uint32_t inode_size = 256;
    const uint16_t desc_size = 64;
    const uint32_t inodes_per_block = block_size / inode_size;  // 16

    // ── Block 0: Boot sector + Superblock ───────────────────────
    {
        std::vector<uint8_t> block(block_size, 0);
        // Boot code (bytes 0-1023): zeros
        // Superblock at offset 1024:
        fill_valid_superblock(block.data() + 1024, 1024);
        // Override the superblock fields that fill_valid_superblock
        // doesn't know about our specific layout:
        //   s_inodes_per_group = 512 (already set by fill_valid_superblock)
        //   s_blocks_per_group = 32768 (already set)
        //   s_inode_size = 256 (already set)
        //   s_desc_size = 64 (already set)
        //   s_first_data_block = 0 (already set)
        //   s_blocks_count_lo = make it small for our image
        uint32_t small_blocks = 8192;  // ~32 MB image
        std::memcpy(&block[1024 + 4], &small_blocks, sizeof(small_blocks));
        //   s_free_blocks_count_lo: 8192 - 36 (blocks 0-35 allocated)
        uint32_t free_blocks = 8192 - 36;
        std::memcpy(&block[1024 + 12], &free_blocks, sizeof(free_blocks));
        //   s_free_inodes_count: 512 - 3 = 509 (inodes 1, 2, 12 + reserved)
        uint32_t free_inodes = 509;
        std::memcpy(&block[1024 + 16], &free_inodes, sizeof(free_inodes));

        dev.write_block(0, block.data());
    }

    // ── Parse superblock to get sbi ─────────────────────────────
    {
        std::vector<uint8_t> tmp(4096, 0);
        dev.read_block(0, tmp.data());
        int ret = ext4_parse_superblock(tmp.data() + 1024, sbi);
        REQUIRE(ret == 0);
        REQUIRE(sbi->block_size == 4096);
        REQUIRE(sbi->inodes_per_group == 512);
    }

    // ── Block 1: Group Descriptor Table ─────────────────────────
    {
        std::vector<uint8_t> gdt_block(block_size, 0);
        // Group 0 at offset 0 (64 bytes):
        // offset 0: bg_block_bitmap_lo = 2
        uint32_t block_bitmap = 2;
        std::memcpy(&gdt_block[0], &block_bitmap, sizeof(block_bitmap));
        // offset 4: bg_inode_bitmap_lo = 3
        uint32_t inode_bitmap = 3;
        std::memcpy(&gdt_block[4], &inode_bitmap, sizeof(inode_bitmap));
        // offset 8: bg_inode_table_lo = 4
        uint32_t inode_table = 4;
        std::memcpy(&gdt_block[8], &inode_table, sizeof(inode_table));
        // offset 12: bg_free_blocks_count_lo = 8192 - 36 = 8156
        uint16_t free_blks = static_cast<uint16_t>(8192 - 36);
        std::memcpy(&gdt_block[12], &free_blks, sizeof(free_blks));
        // offset 14: bg_free_inodes_count_lo = 509
        uint16_t free_inos = 509;
        std::memcpy(&gdt_block[14], &free_inos, sizeof(free_inos));
        // offset 16: bg_used_dirs_count_lo = 1 (root dir)
        uint16_t used_dirs = 1;
        std::memcpy(&gdt_block[16], &used_dirs, sizeof(used_dirs));
        // offset 18: bg_flags = 0
        // offset 30: bg_checksum = 0 (no csum feature)

        dev.write_block(1, gdt_block.data());
    }

    // ── Block 2: Block bitmap ──────────────────────────────────
    {
        std::vector<uint8_t> bitmap(block_size, 0xFF);
        // Mark blocks 0-35 as allocated (all zero in bitmap = allocated? no,
        // in ext4 bitmaps, 1 = free, 0 = allocated)
        for (uint64_t i = 0; i < 36; i++) {
            uint64_t byte = i / 8;
            uint64_t bit  = i % 8;
            if (byte < block_size) {
                bitmap[byte] &= ~(1 << bit);  // 0 = allocated
            }
        }
        dev.write_block(2, bitmap.data());
    }

    // ── Block 3: Inode bitmap ──────────────────────────────────
    {
        std::vector<uint8_t> bitmap(block_size, 0xFF);
        // Mark inodes 1, 2, 12 as allocated (1-based)
        // Inode 1  → bit 0  (index = 0)
        // Inode 2  → bit 1  (index = 1)
        // Inode 12 → bit 11 (index = 11)
        uint32_t allocated_inodes[] = {0, 1, 11}; // zero-based index
        for (auto idx : allocated_inodes) {
            uint64_t byte = idx / 8;
            uint64_t bit  = idx % 8;
            if (byte < block_size) {
                bitmap[byte] &= ~(1 << bit);  // 0 = allocated
            }
        }
        dev.write_block(3, bitmap.data());
    }

    // ── Blocks 4-35: Inode table ───────────────────────────────
    {
        const uint32_t itable_blocks = (inodes_per_group + inodes_per_block - 1) / inodes_per_block;
        std::vector<uint8_t> itable(itable_blocks * block_size, 0);

        // Helper: write a 256-byte inode at a given inode index (0-based)
        auto write_inode = [&](uint32_t index, const uint8_t *data) {
            uint64_t block_offset = index / inodes_per_block;
            uint64_t in_block_off  = (index % inodes_per_block) * inode_size;
            uint64_t abs_off = block_offset * block_size + in_block_off;
            if (abs_off + inode_size <= itable.size()) {
                std::memcpy(&itable[abs_off], data, inode_size);
            }
        };

        // Helper: serialize u16 LE into a buffer
        auto put16 = [](uint8_t *buf, uint16_t val) {
            buf[0] = val & 0xFF;
            buf[1] = (val >> 8) & 0xFF;
        };
        auto put32 = [](uint8_t *buf, uint32_t val) {
            buf[0] = val & 0xFF;
            buf[1] = (val >> 8) & 0xFF;
            buf[2] = (val >> 16) & 0xFF;
            buf[3] = (val >> 24) & 0xFF;
        };
        auto put64 = [](uint8_t *buf, uint64_t val) {
            for (int i = 0; i < 8; i++) {
                buf[i] = (val >> (i * 8)) & 0xFF;
            }
        };

        // ── Inode 1 (bad block inode, reserved) ────────────────
        // Leave zeroed — it's reserved but not used here.

        // ── Inode 2 (root directory) ────────────────────────────
        {
            uint8_t raw[256] = {0};
            put16(&raw[INODE_OFF_MODE], ROOT_DIR_MODE);      // 0x41ED
            put16(&raw[INODE_OFF_UID], 0);                    // uid = 0
            put32(&raw[INODE_OFF_SIZE_LO], 4096);             // size = 4096
            put32(&raw[INODE_OFF_ATIME], 1000000);            // atime
            put32(&raw[INODE_OFF_CTIME], 1000000);            // ctime
            put32(&raw[INODE_OFF_MTIME], 1000000);            // mtime
            put32(&raw[INODE_OFF_DTIME], 0);                  // dtime = 0 (active)
            put16(&raw[INODE_OFF_GID], 0);                    // gid = 0
            put16(&raw[INODE_OFF_LINKS], 2);                  // links_count = 2 (., ..)
            put32(&raw[INODE_OFF_BLOCKS_LO], 8);              // 4096 / 512 = 8 sectors
            put32(&raw[INODE_OFF_FLAGS], EXT4_EXTENTS_FL);    // extents flag
            // Extent tree header in i_block (empty, 4 slots)
            put16(&raw[INODE_OFF_IBLOCK], EXT4_EXTENT_MAGIC); // eh_magic
            put16(&raw[INODE_OFF_IBLOCK + 2], 0);             // eh_entries = 0
            put16(&raw[INODE_OFF_IBLOCK + 4], 4);             // eh_max = 4
            put16(&raw[INODE_OFF_IBLOCK + 6], 0);             // eh_depth = 0
            put32(&raw[INODE_OFF_IBLOCK + 8], 0);             // eh_generation = 0
            // Extra isize for 256-byte inode
            put16(&raw[INODE_EXTRA_ISIZE], 32);               // i_extra_isize = 32

            write_inode(1, raw);  // index 1 = inode 2
        }

        // ── Inode 12 (test regular file) ───────────────────────
        {
            uint8_t raw[256] = {0};
            put16(&raw[INODE_OFF_MODE], TEST_FILE_MODE);      // 0x81A4
            put16(&raw[INODE_OFF_UID], 1000);                  // uid = 1000
            put32(&raw[INODE_OFF_SIZE_LO], 1024);              // size = 1024
            put32(&raw[INODE_OFF_ATIME], 2000000);
            put32(&raw[INODE_OFF_CTIME], 2000000);
            put32(&raw[INODE_OFF_MTIME], 2000000);
            put16(&raw[INODE_OFF_GID], 100);                   // gid = 100
            put16(&raw[INODE_OFF_LINKS], 1);                   // links_count = 1
            put32(&raw[INODE_OFF_BLOCKS_LO], 2);               // 1024 / 512 = 2 sectors
            put32(&raw[INODE_OFF_FLAGS], EXT4_EXTENTS_FL);     // extents flag
            // Extent tree header (empty)
            put16(&raw[INODE_OFF_IBLOCK], EXT4_EXTENT_MAGIC);
            put16(&raw[INODE_OFF_IBLOCK + 2], 0);
            put16(&raw[INODE_OFF_IBLOCK + 4], 4);
            put16(&raw[INODE_OFF_IBLOCK + 6], 0);
            put32(&raw[INODE_OFF_IBLOCK + 8], 0);
            put16(&raw[INODE_EXTRA_ISIZE], 32);                // i_extra_isize = 32

            write_inode(11, raw);  // index 11 = inode 12
        }

        // Write all inode table blocks
        for (uint32_t b = 0; b < itable_blocks; b++) {
            dev.write_block(4 + b, &itable[b * block_size]);
        }
    }
}

/* =================================================================
 * Test fixture: builds a minimal ext4 image once per test case
 * ================================================================= */

struct InodeTestFixture {
    MockBlockDev dev;
    ext4_sb_info sbi;
    ext4_inode_info info;

    InodeTestFixture() : dev(4096), info{} {
        build_minimal_ext4_image(dev, &sbi);
    }
};

/* =================================================================
 * Test Cases
 * ================================================================= */

TEST_CASE_METHOD(InodeTestFixture,
    "ext4_read_inode reads root directory inode (ino 2)",
    "[ext4][inode]") {
    int ret = ext4_read_inode(&sbi, 2, &info,
                              &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(info.ino == 2);
    REQUIRE(info.mode == ROOT_DIR_MODE);
    REQUIRE(info.size == 4096);
    REQUIRE(info.uid == 0);
    REQUIRE(info.gid == 0);
    REQUIRE(info.is_dir == 1);
    REQUIRE(info.is_reg == 0);
    REQUIRE(info.is_lnk == 0);
    REQUIRE(info.has_extents == 1);
    REQUIRE(info.links_count == 2);
    REQUIRE(info.blocks == 8);
    REQUIRE(info.atime == 1000000);
    REQUIRE(info.ctime == 1000000);
    REQUIRE(info.mtime == 1000000);
    REQUIRE(info.dtime == 0);
}

TEST_CASE_METHOD(InodeTestFixture,
    "ext4_read_inode reads test regular file (ino 12)",
    "[ext4][inode]") {
    int ret = ext4_read_inode(&sbi, 12, &info,
                              &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(info.ino == 12);
    REQUIRE(info.mode == TEST_FILE_MODE);
    REQUIRE(info.size == 1024);
    REQUIRE(info.uid == 1000);
    REQUIRE(info.gid == 100);
    REQUIRE(info.is_dir == 0);
    REQUIRE(info.is_reg == 1);
    REQUIRE(info.has_extents == 1);
    REQUIRE(info.links_count == 1);
    REQUIRE(info.blocks == 2);
    REQUIRE(info.atime == 2000000);
}

TEST_CASE_METHOD(InodeTestFixture,
    "ext4_read_inode returns error for non-existent inode (ino 999)",
    "[ext4][inode]") {
    // Inode 999 is in group 1, but we only have group 0.
    // block::read_inode checks group >= groups.len() → NotFound (ENOENT=2).
    int ret = ext4_read_inode(&sbi, 999, &info,
                              &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 2); // ENOENT — inode not in any existing group
}

TEST_CASE_METHOD(InodeTestFixture,
    "ext4_read_inode rejects null pointers",
    "[ext4][inode]") {
    int ret;
    ret = ext4_read_inode(nullptr, 2, &info, &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 22); // EINVAL

    ret = ext4_read_inode(&sbi, 2, nullptr, &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 22);

    ret = ext4_read_inode(&sbi, 2, &info, nullptr, nullptr);
    REQUIRE(ret == 22);
}

TEST_CASE_METHOD(InodeTestFixture,
    "ext4_stat returns correct values for root inode",
    "[ext4][inode]") {
    uint16_t mode = 0, uid = 0, gid = 0;
    uint64_t size = 0;

    int ret = ext4_stat(&sbi, 2, &mode, &size, &uid, &gid,
                        &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(mode == ROOT_DIR_MODE);
    REQUIRE(size == 4096);
    REQUIRE(uid == 0);
    REQUIRE(gid == 0);
}

TEST_CASE_METHOD(InodeTestFixture,
    "ext4_stat allows null output pointers",
    "[ext4][inode]") {
    // Passing NULL for output pointers should be OK (function checks is_null)
    int ret = ext4_stat(&sbi, 2, nullptr, nullptr, nullptr, nullptr,
                        &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
}

TEST_CASE_METHOD(InodeTestFixture,
    "ext4_chown changes uid and gid on test file",
    "[ext4][inode]") {
    // Change ownership of inode 12 to uid=500, gid=200
    uint16_t mode_in = TEST_FILE_MODE;
    int ret = ext4_chown(&sbi, 12, 500, 200, &mode_in,
                         &dev, MockBlockDev::read_cb,
                         MockBlockDev::write_cb);
    REQUIRE(ret == 0);

    // Verify by re-reading the inode
    ext4_inode_info info2;
    ret = ext4_read_inode(&sbi, 12, &info2, &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(info2.uid == 500);
    REQUIRE(info2.gid == 200);
    REQUIRE(info2.mode == TEST_FILE_MODE);  // mode unchanged
}

TEST_CASE_METHOD(InodeTestFixture,
    "ext4_chmod changes permissions on root directory",
    "[ext4][inode]") {
    // Change root dir mode to 0700
    uint16_t new_mode = S_IFDIR | S_IRWXU;  // 0x41C0
    int ret = ext4_chmod(&sbi, 2, &new_mode,
                         &dev, MockBlockDev::read_cb,
                         MockBlockDev::write_cb);
    REQUIRE(ret == 0);

    // Verify the mode was updated (type bits preserved)
    uint16_t expected = S_IFDIR | S_IRWXU;  // 0x41C0
    REQUIRE(new_mode == expected);

    // Re-read to confirm persistence
    ext4_inode_info info2;
    ret = ext4_read_inode(&sbi, 2, &info2, &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(info2.mode == expected);
}

TEST_CASE_METHOD(InodeTestFixture,
    "ext4_utime updates timestamps on test file",
    "[ext4][inode]") {
    uint32_t new_atime = 999888777;
    uint32_t new_mtime = 111222333;

    int ret = ext4_utime(&sbi, 12, new_atime, new_mtime,
                         &dev, MockBlockDev::read_cb,
                         MockBlockDev::write_cb);
    REQUIRE(ret == 0);

    // Verify timestamps were updated
    ext4_inode_info info2;
    ret = ext4_read_inode(&sbi, 12, &info2, &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(info2.atime == new_atime);
    REQUIRE(info2.mtime == new_mtime);
    REQUIRE(info2.ctime == new_mtime);  // ctime is set to mtime value
}

TEST_CASE_METHOD(InodeTestFixture,
    "ext4_chown rejects null pointers",
    "[ext4][inode]") {
    uint16_t mode = TEST_FILE_MODE;

    int ret = ext4_chown(&sbi, 12, 500, 200, &mode, nullptr, nullptr, nullptr);
    REQUIRE(ret == 22); // EINVAL

    ret = ext4_chown(nullptr, 12, 500, 200, &mode,
                     &dev, MockBlockDev::read_cb, MockBlockDev::write_cb);
    REQUIRE(ret == 22);

    ret = ext4_chown(&sbi, 12, 500, 200, nullptr,
                     &dev, MockBlockDev::read_cb, MockBlockDev::write_cb);
    REQUIRE(ret == 22);
}

TEST_CASE_METHOD(InodeTestFixture,
    "ext4_chmod rejects null pointers",
    "[ext4][inode]") {
    int ret = ext4_chmod(nullptr, 2, nullptr, nullptr, nullptr, nullptr);
    REQUIRE(ret == 22);
}

TEST_CASE_METHOD(InodeTestFixture,
    "ext4_utime rejects null pointers",
    "[ext4][inode]") {
    int ret = ext4_utime(nullptr, 12, 0, 0, nullptr, nullptr, nullptr);
    REQUIRE(ret == 22);
}
