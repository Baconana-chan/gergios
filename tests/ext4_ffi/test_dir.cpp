/* tests/ext4_ffi/test_dir.cpp
 *
 * Phase 9.1: C FFI integration tests for ext4-core directory operations.
 * Tests: ext4_lookup, ext4_readdir
 *
 * Builds a minimal ext4 image with:
 *   — Valid superblock + GDT + bitmaps
 *   — Inode table with root dir (ino 2) having an extent tree
 *   — Data block with directory entries (".", "..", "test.txt", "subdir")
 *
 * Byte offsets verified against rust/ext4-core/src/:
 *   superblock.rs, group_desc.rs, inode.rs, extent.rs, dir.rs
 */

#include <catch.hpp>
#include <cstring>
#include <cstdint>
#include <map>
#include <vector>

extern "C" {
#include <ext4.h>
}

#include "helpers.h"

/* =================================================================
 * MockBlockDev — in-memory block device (same pattern as test_inode.cpp)
 * ================================================================= */

class MockBlockDev {
public:
    explicit MockBlockDev(uint32_t block_size)
        : block_size_(block_size) {}

    void write_block(uint64_t block_nr, const uint8_t *data) {
        std::vector<uint8_t> block(data, data + block_size_);
        blocks_[block_nr] = std::move(block);
    }

    int read_block(uint64_t block_nr, uint8_t *buf) const {
        auto it = blocks_.find(block_nr);
        if (it == blocks_.end()) {
            std::memset(buf, 0, block_size_);
            return 0;
        }
        std::memcpy(buf, it->second.data(), block_size_);
        return 0;
    }

    static int read_cb(void *ctx, uint64_t block_nr,
                       uint8_t *buf, uint32_t block_size) {
        auto *dev = static_cast<MockBlockDev *>(ctx);
        if (block_size != dev->block_size_) return -1;
        return dev->read_block(block_nr, buf);
    }

private:
    uint32_t block_size_;
    std::map<uint64_t, std::vector<uint8_t>> blocks_;
};

/* =================================================================
 * Ext4 constants (from rust/ext4-core/src/types.rs)
 * ================================================================= */

static constexpr uint16_t S_IFDIR   = 0x4000;
static constexpr uint16_t S_IFREG   = 0x8000;
static constexpr uint32_t EXT4_EXTENTS_FL = 0x00080000;
static constexpr uint16_t EXT4_EXTENT_MAGIC = 0xF30A;

static constexpr uint16_t ROOT_DIR_MODE = S_IFDIR | 0755;  // 0x41ED
static constexpr uint16_t REG_FILE_MODE = S_IFREG | 0644;  // 0x81A4

/* File types for directory entries */
static constexpr uint8_t FT_DIR      = 2;
static constexpr uint8_t FT_REG_FILE = 1;

/* Inode field offsets (within 256-byte inode) */
static constexpr int OFF_MODE       = 0;
static constexpr int OFF_UID        = 2;
static constexpr int OFF_SIZE_LO    = 4;
static constexpr int OFF_ATIME      = 8;
static constexpr int OFF_CTIME      = 12;
static constexpr int OFF_MTIME      = 16;
static constexpr int OFF_DTIME      = 20;
static constexpr int OFF_GID        = 24;
static constexpr int OFF_LINKS      = 26;
static constexpr int OFF_BLOCKS_LO  = 28;
static constexpr int OFF_FLAGS      = 32;
static constexpr int OFF_IBLOCK     = 40;   // 60 bytes
static constexpr int OFF_EXTRA_ISIZE = 128;
static constexpr int OFF_CSUM       = 130;

/* =================================================================
 * Helpers: serialise integers in little-endian
 * ================================================================= */

static void put16(uint8_t *buf, uint16_t val) {
    buf[0] =  val        & 0xFF;
    buf[1] = (val >> 8)  & 0xFF;
}
static void put32(uint8_t *buf, uint32_t val) {
    buf[0] =  val        & 0xFF;
    buf[1] = (val >> 8)  & 0xFF;
    buf[2] = (val >> 16) & 0xFF;
    buf[3] = (val >> 24) & 0xFF;
}

/* =================================================================
 * Build a minimal ext4 image with directory data
 *
 * Block layout (4096-byte blocks):
 *    0: Boot padding + Superblock (offset 1024)
 *    1: Group Descriptor Table  (64 bytes for group 0)
 *    2: Block bitmap
 *    3: Inode bitmap
 *  4–35: Inode table (512 inodes × 256 B = 128 KiB = 32 blocks)
 *     36: Root directory data block
 * ================================================================= */

static void build_dir_ext4_image(MockBlockDev &dev, ext4_sb_info *sbi) {
    const uint32_t bs  = 4096;           // block size
    const uint32_t ipg = 512;            // inodes per group
    const uint32_t bpg = 32768;          // blocks per group
    const uint32_t is  = 256;            // inode size
    const uint16_t ds  = 64;             // descriptor size
    const uint32_t ipb = bs / is;        // inodes per block = 16

    // How many blocks our test image occupies:
    // blocks 0–5 (SB, GDT, bitmaps) + itable (32) + data (1) = 37
    // + a few extra for itable rounding = 38
    const uint64_t DATA_BLOCK = 4 + (ipg + ipb - 1) / ipb;  // = 36

    // ── Block 0: Boot + Superblock ──────────────────────────────
    {
        std::vector<uint8_t> blk(bs, 0);
        fill_valid_superblock(blk.data() + 1024, 1024);
        // Override sizes to match our small image
        uint32_t total_blocks = 8192;
        std::memcpy(&blk[1024 + 4], &total_blocks, sizeof(total_blocks));
        // Free blocks = total - used (blocks 0..36 = 37 blocks, plus hint)
        uint32_t free_blocks = 8192 - 38;
        std::memcpy(&blk[1024 + 12], &free_blocks, sizeof(free_blocks));
        uint32_t free_inodes = 509;
        std::memcpy(&blk[1024 + 16], &free_inodes, sizeof(free_inodes));
        dev.write_block(0, blk.data());
    }

    // ── Parse SB → sbi ──────────────────────────────────────────
    {
        std::vector<uint8_t> tmp(bs, 0);
        dev.read_block(0, tmp.data());
        int ret = ext4_parse_superblock(tmp.data() + 1024, sbi);
        REQUIRE(ret == 0);
    }

    // ── Block 1: GDT ────────────────────────────────────────────
    {
        std::vector<uint8_t> blk(bs, 0);
        put32(&blk[0],  2);   // bg_block_bitmap_lo
        put32(&blk[4],  3);   // bg_inode_bitmap_lo
        put32(&blk[8],  4);   // bg_inode_table_lo
        put16(&blk[12], 8156);// bg_free_blocks_count_lo
        put16(&blk[14], 509); // bg_free_inodes_count_lo
        put16(&blk[16], 1);   // bg_used_dirs_count_lo
        dev.write_block(1, blk.data());
    }

    // ── Block 2: Block bitmap ───────────────────────────────────
    {
        std::vector<uint8_t> bm(bs, 0xFF);
        for (uint64_t i = 0; i <= DATA_BLOCK; i++) {
            bm[i / 8] &= ~(uint8_t)(1 << (i % 8));
        }
        dev.write_block(2, bm.data());
    }

    // ── Block 3: Inode bitmap ───────────────────────────────────
    {
        std::vector<uint8_t> bm(bs, 0xFF);
        bm[0] &= ~0x03;  // bits 0,1 = inode 1,2
        dev.write_block(3, bm.data());
    }

    // ── Blocks 4–35: Inode table ────────────────────────────────
    {
        uint32_t itblk = (ipg + ipb - 1) / ipb;  // 32
        std::vector<uint8_t> itab(itblk * bs, 0);

        auto poke_inode = [&](uint32_t idx, const uint8_t *raw) {
            uint64_t bo = idx / ipb;
            uint64_t io = (idx % ipb) * is;
            uint64_t off = bo * bs + io;
            if (off + is <= itab.size())
                std::memcpy(&itab[off], raw, is);
        };

        // ── Inode 2: root dir with 1 extent ────────────────────
        {
            uint8_t raw[256] = {0};
            put16(&raw[OFF_MODE],      ROOT_DIR_MODE);
            put32(&raw[OFF_SIZE_LO],   bs);            // size = 4096
            put32(&raw[OFF_ATIME],     1000000);
            put32(&raw[OFF_CTIME],     1000000);
            put32(&raw[OFF_MTIME],     1000000);
            put16(&raw[OFF_LINKS],     2);
            put32(&raw[OFF_BLOCKS_LO], 8);             // 4096/512 sectors
            put32(&raw[OFF_FLAGS],     EXT4_EXTENTS_FL);
            // Extent tree header + 1 extent in i_block
            // Header (12 bytes): magic, entries=1, max=4, depth=0, gen=0
            put16(&raw[OFF_IBLOCK],     EXT4_EXTENT_MAGIC);
            put16(&raw[OFF_IBLOCK + 2], 1);             // entries = 1
            put16(&raw[OFF_IBLOCK + 4], 4);             // max = 4
            // Extent leaf (12 bytes): ee_block=0, ee_len=1, ee_start_hi=0, ee_start_lo=DATA_BLOCK
            put32(&raw[OFF_IBLOCK + 12], 0);            // ee_block = 0
            put16(&raw[OFF_IBLOCK + 16], 1);            // ee_len = 1
            put16(&raw[OFF_IBLOCK + 18], 0);            // ee_start_hi = 0
            put32(&raw[OFF_IBLOCK + 20], DATA_BLOCK);   // ee_start_lo = 36
            put16(&raw[OFF_EXTRA_ISIZE], 32);
            poke_inode(1, raw);  // ino 2 = idx 1
        }

        for (uint32_t b = 0; b < itblk; b++)
            dev.write_block(4 + b, &itab[b * bs]);
    }

    // ── Block DATA_BLOCK (36): directory entries ────────────────
    {
        std::vector<uint8_t> blk(bs, 0);
        uint32_t off = 0;

        auto write_dirent = [&](uint32_t ino, uint8_t ft,
                                const char *name, bool last) {
            uint8_t nlen = static_cast<uint8_t>(std::strlen(name));
            // Entry size: 8 bytes header + name_len, rounded up to 4
            uint16_t reclen = static_cast<uint16_t>((8 + nlen + 3) & ~3u);
            if (last) {
                // Last entry fills the rest of the block
                reclen = static_cast<uint16_t>(bs - off);
            }
            put32(&blk[off],     ino);
            put16(&blk[off + 4], reclen);
            blk[off + 6] = nlen;
            blk[off + 7] = ft;
            std::memcpy(&blk[off + 8], name, nlen);
            off += reclen;
        };

        write_dirent(2,  FT_DIR,      ".",        false); // off  0, rec_len=12
        write_dirent(2,  FT_DIR,      "..",       false); // off 12, rec_len=12
        write_dirent(42, FT_REG_FILE, "test.txt", false); // off 24, rec_len=16
        write_dirent(99, FT_DIR,      "subdir",   true);  // off 40, rec_len=4056

        dev.write_block(DATA_BLOCK, blk.data());
    }
}

/* =================================================================
 * Test fixture
 * ================================================================= */

struct DirTestFixture {
    MockBlockDev dev;
    ext4_sb_info sbi;

    DirTestFixture() : dev(4096), sbi{} {
        build_dir_ext4_image(dev, &sbi);
    }
};

/* =================================================================
 * ext4_lookup tests
 * ================================================================= */

TEST_CASE_METHOD(DirTestFixture,
    "ext4_lookup finds '.' entry in root directory",
    "[ext4][dir][lookup]") {
    uint32_t out_ino = 0;
    uint8_t  out_type = 0xFF;
    int ret = ext4_lookup(&sbi, 2, ".", &out_ino, &out_type,
                          &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(out_ino == 2);
    REQUIRE(out_type == FT_DIR);
}

TEST_CASE_METHOD(DirTestFixture,
    "ext4_lookup finds 'test.txt' entry in root directory",
    "[ext4][dir][lookup]") {
    uint32_t out_ino = 0;
    uint8_t  out_type = 0xFF;
    int ret = ext4_lookup(&sbi, 2, "test.txt", &out_ino, &out_type,
                          &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(out_ino == 42);
    REQUIRE(out_type == FT_REG_FILE);
}

TEST_CASE_METHOD(DirTestFixture,
    "ext4_lookup finds 'subdir' entry in root directory",
    "[ext4][dir][lookup]") {
    uint32_t out_ino = 0;
    uint8_t  out_type = 0xFF;
    int ret = ext4_lookup(&sbi, 2, "subdir", &out_ino, &out_type,
                          &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(out_ino == 99);
    REQUIRE(out_type == FT_DIR);
}

TEST_CASE_METHOD(DirTestFixture,
    "ext4_lookup returns ENOENT for non-existent name",
    "[ext4][dir][lookup]") {
    uint32_t out_ino = 0;
    uint8_t  out_type = 0xFF;
    int ret = ext4_lookup(&sbi, 2, "nosuchfile.txt", &out_ino, &out_type,
                          &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 2);  // ENOENT
}

TEST_CASE_METHOD(DirTestFixture,
    "ext4_lookup returns ENOENT for empty string name",
    "[ext4][dir][lookup]") {
    uint32_t out_ino = 0;
    uint8_t  out_type = 0xFF;
    int ret = ext4_lookup(&sbi, 2, "", &out_ino, &out_type,
                          &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 2);  // ENOENT — empty name not found
}

TEST_CASE_METHOD(DirTestFixture,
    "ext4_lookup rejects null pointers",
    "[ext4][dir][lookup]") {
    uint32_t out_ino;
    uint8_t  out_type;
    int ret;

    ret = ext4_lookup(nullptr, 2, ".", &out_ino, &out_type,
                      &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 22);  // EINVAL

    ret = ext4_lookup(&sbi, 2, nullptr, &out_ino, &out_type,
                      &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 22);

    ret = ext4_lookup(&sbi, 2, ".", nullptr, &out_type,
                      &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 22);
}

/* =================================================================
 * ext4_readdir tests
 * ================================================================= */

TEST_CASE_METHOD(DirTestFixture,
    "ext4_readdir reads all entries from root directory starting at pos=0",
    "[ext4][dir][readdir]") {
    ext4_dirent entries[8];
    uint64_t pos = 0;
    uint32_t count = 0;

    int ret = ext4_readdir(&sbi, 2, &pos, entries, 8, &count,
                           &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(count >= 3);  // at least 3 entries (excluding the fake htree one)

    // Check that '.' is among the entries
    bool found_dot = false;
    bool found_dotdot = false;
    bool found_test = false;
    bool found_subdir = false;
    for (uint32_t i = 0; i < count; i++) {
        std::string name(entries[i].name, entries[i].name_len);
        if (name == ".") {
            found_dot = true;
            REQUIRE(entries[i].ino == 2);
            REQUIRE(entries[i].file_type == FT_DIR);
        } else if (name == "..") {
            found_dotdot = true;
            REQUIRE(entries[i].ino == 2);
            REQUIRE(entries[i].file_type == FT_DIR);
        } else if (name == "test.txt") {
            found_test = true;
            REQUIRE(entries[i].ino == 42);
            REQUIRE(entries[i].file_type == FT_REG_FILE);
        } else if (name == "subdir") {
            found_subdir = true;
            REQUIRE(entries[i].ino == 99);
            REQUIRE(entries[i].file_type == FT_DIR);
        }
    }
    REQUIRE(found_dot);
    REQUIRE(found_dotdot);
    REQUIRE(found_test);
    REQUIRE(found_subdir);
}

TEST_CASE_METHOD(DirTestFixture,
    "ext4_readdir with max_entries=1 returns one entry at a time",
    "[ext4][dir][readdir]") {
    ext4_dirent entries[1];
    uint64_t pos = 0;
    uint32_t count = 0;

    // Read first entry
    int ret = ext4_readdir(&sbi, 2, &pos, entries, 1, &count,
                           &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(count == 1);
    REQUIRE(pos > 0);

    // Read second entry
    ret = ext4_readdir(&sbi, 2, &pos, entries, 1, &count,
                       &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(count == 1);
}

TEST_CASE_METHOD(DirTestFixture,
    "ext4_readdir returns count=0 at EOF",
    "[ext4][dir][readdir]") {
    ext4_dirent entries[1];
    uint64_t pos = 0;
    uint32_t count = 0;

    // Read all entries by iterating past the block end
    pos = 10000;  // past the 4096-byte block
    int ret = ext4_readdir(&sbi, 2, &pos, entries, 1, &count,
                           &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 0);
    REQUIRE(count == 0);
}

TEST_CASE_METHOD(DirTestFixture,
    "ext4_readdir rejects null pointers",
    "[ext4][dir][readdir]") {
    ext4_dirent entries[4];
    uint64_t pos = 0;
    uint32_t count;

    int ret = ext4_readdir(nullptr, 2, &pos, entries, 4, &count,
                           &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 22);

    ret = ext4_readdir(&sbi, 2, nullptr, entries, 4, &count,
                       &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 22);

    ret = ext4_readdir(&sbi, 2, &pos, nullptr, 4, &count,
                       &dev, MockBlockDev::read_cb);
    REQUIRE(ret == 22);
}
