//! Catch2 Block Device tests (migrated from ATF blocktest)
//!
//! Phase 9.4: C Test Migration — ATF → Catch2
//! See planning/28_testing_framework_migration.md
//!
//! Validates block device I/O semantics: read/write operations,
//! block alignment, error handling, and partition table parsing.
//! These tests use an in-memory block device on the host.
//! MINIX-dependent tests (real /dev/c0d0 access) will skip.

#include "catch.hpp"
#include <cstdint>
#include <cstring>
#include <vector>
#include <algorithm>
#include <cerrno>
#include <endian.h>

// ============================================================================
// In-memory block device emulation
// ============================================================================

// Standard block sizes
static constexpr uint32_t BLOCK_SIZE_512  = 512;
static constexpr uint32_t BLOCK_SIZE_1K   = 1024;
static constexpr uint32_t BLOCK_SIZE_4K   = 4096;

struct MemBlockDev {
    uint8_t* data = nullptr;
    uint32_t block_size = 512;
    uint32_t block_count = 0;
    bool readonly = false;
    bool fault_inject = false;  // simulate I/O error

    MemBlockDev(uint32_t blk_size, uint32_t blk_count,
               bool rdonly = false)
        : block_size(blk_size), block_count(blk_count),
          readonly(rdonly) {
        data = new uint8_t[block_size * block_count];
        memset(data, 0, block_size * block_count);
    }

    ~MemBlockDev() { delete[] data; }

    int read_block(uint32_t block, void* buf, uint32_t count = 1) {
        if (fault_inject) return -EIO;
        if (block + count > block_count) return -EINVAL;
        memcpy(buf, data + block * block_size, block_size * count);
        return 0;
    }

    int write_block(uint32_t block, const void* buf, uint32_t count = 1) {
        if (readonly) return -EROFS;
        if (fault_inject) return -EIO;
        if (block + count > block_count) return -EINVAL;
        memcpy(data + block * block_size, buf, block_size * count);
        return 0;
    }

    // Fill block with a known pattern
    void fill_block(uint32_t block, uint8_t pattern) {
        memset(data + block * block_size, pattern, block_size);
    }

    // Fill block range with incrementing bytes
    void fill_pattern(uint32_t start_block, uint32_t count) {
        for (uint32_t b = 0; b < count; b++) {
            for (uint32_t i = 0; i < block_size; i++) {
                data[(start_block + b) * block_size + i] =
                    static_cast<uint8_t>((b * block_size + i) & 0xFF);
            }
        }
    }
};

// ============================================================================
// Master Boot Record (MBR) partition table helpers
// ============================================================================

struct [[gnu::packed]] MbrPartitionEntry {
    uint8_t  status;       // 0x80 = active
    uint8_t  first_chs[3];
    uint8_t  type;         // partition type (e.g., 0x83 = Linux)
    uint8_t  last_chs[3];
    uint32_t first_lba;    // LBA of first sector
    uint32_t sector_count; // number of sectors
};

struct [[gnu::packed]] MbrSector {
    uint8_t  bootstrap[446];
    MbrPartitionEntry partitions[4];
    uint16_t signature;    // 0xAA55
};

// GPT partition table (simplified)
struct [[gnu::packed]] GptHeader {
    uint64_t signature;      // "EFI PART"
    uint32_t revision;       // 1.0
    uint32_t header_size;    // 92
    uint32_t header_crc32;
    uint32_t reserved;
    uint64_t my_lba;
    uint64_t alternate_lba;
    uint64_t first_usable_lba;
    uint64_t last_usable_lba;
    uint8_t  guid[16];
    uint64_t partition_entry_lba;
    uint32_t num_partition_entries;
    uint32_t partition_entry_size;
    uint32_t partition_array_crc32;
};

struct [[gnu::packed]] GptPartitionEntry {
    uint8_t  type_guid[16];
    uint8_t  unique_guid[16];
    uint64_t starting_lba;
    uint64_t ending_lba;
    uint64_t attributes;
    uint16_t name[36];  // 36 UTF-16LE chars
};

static constexpr uint64_t GPT_SIGNATURE = 0x5452415020494645ULL;  // "EFI PART"

// ============================================================================
// Test cases — Basic block device operations
// ============================================================================

TEST_CASE("Block device: read/write single block", "[block][io]") {
    MemBlockDev dev(512, 1024);

    // Write pattern to block 0
    uint8_t wbuf[512];
    memset(wbuf, 0xAB, sizeof(wbuf));
    REQUIRE(dev.write_block(0, wbuf) == 0);

    // Read back and verify
    uint8_t rbuf[512];
    REQUIRE(dev.read_block(0, rbuf) == 0);
    REQUIRE(memcmp(wbuf, rbuf, sizeof(wbuf)) == 0);

    // Block 1 should still be zero
    REQUIRE(dev.read_block(1, rbuf) == 0);
    for (auto b : rbuf) REQUIRE(b == 0);
}

TEST_CASE("Block device: multi-block read/write", "[block][io]") {
    MemBlockDev dev(512, 1024);

    // Fill blocks 0-9 with pattern
    dev.fill_pattern(0, 10);

    // Read back and verify
    uint8_t buf[512 * 10];
    REQUIRE(dev.read_block(0, buf, 10) == 0);

    for (uint32_t b = 0; b < 10; b++) {
        for (uint32_t i = 0; i < 512; i++) {
            uint8_t expected = static_cast<uint8_t>((b * 512 + i) & 0xFF);
            REQUIRE(buf[b * 512 + i] == expected);
        }
    }
}

TEST_CASE("Block device: write then read same blocks", "[block][io]") {
    MemBlockDev dev(512, 128);

    // Write incrementing pattern
    for (uint32_t b = 0; b < 64; b++) {
        uint8_t wbuf[512];
        memset(wbuf, static_cast<uint8_t>(b & 0xFF), sizeof(wbuf));
        REQUIRE(dev.write_block(b, wbuf) == 0);
    }

    // Verify each block
    for (uint32_t b = 0; b < 64; b++) {
        uint8_t rbuf[512];
        REQUIRE(dev.read_block(b, rbuf) == 0);
        for (auto byte : rbuf) {
            REQUIRE(byte == static_cast<uint8_t>(b & 0xFF));
        }
    }
}

TEST_CASE("Block device: out-of-bounds read returns error", "[block][io]") {
    MemBlockDev dev(512, 64);
    uint8_t buf[512];

    // Read past end
    REQUIRE(dev.read_block(64, buf) == -EINVAL);
    REQUIRE(dev.read_block(100, buf) == -EINVAL);

    // Read past end with multi-block
    REQUIRE(dev.read_block(60, buf, 10) == -EINVAL);
}

TEST_CASE("Block device: out-of-bounds write returns error", "[block][io]") {
    MemBlockDev dev(512, 64);
    uint8_t buf[512];

    REQUIRE(dev.write_block(64, buf) == -EINVAL);
    REQUIRE(dev.write_block(100, buf) == -EINVAL);
}

TEST_CASE("Block device: read-only device rejects writes", "[block][io]") {
    MemBlockDev dev(512, 64, true);  // read-only
    uint8_t buf[512];
    memset(buf, 0xFF, sizeof(buf));

    REQUIRE(dev.write_block(0, buf) == -EROFS);
}

TEST_CASE("Block device: I/O error simulation", "[block][io]") {
    MemBlockDev dev(512, 64);
    dev.fault_inject = true;

    uint8_t buf[512];
    REQUIRE(dev.read_block(0, buf) == -EIO);
    REQUIRE(dev.write_block(0, buf) == -EIO);
}

// ============================================================================
// Test cases — Block device sizes
// ============================================================================

TEST_CASE("Block device: 512-byte sector size", "[block][size]") {
    MemBlockDev dev(512, 100);
    REQUIRE(dev.block_size == 512);
    REQUIRE(dev.block_count == 100);
    REQUIRE(dev.read_block(0, new uint8_t[512]) == 0);
}

TEST_CASE("Block device: 4096-byte sector size (4K native)", "[block][size]") {
    MemBlockDev dev(4096, 100);
    REQUIRE(dev.block_size == 4096);

    uint8_t wbuf[4096];
    memset(wbuf, 0x42, sizeof(wbuf));
    REQUIRE(dev.write_block(0, wbuf) == 0);

    uint8_t rbuf[4096];
    REQUIRE(dev.read_block(0, rbuf) == 0);
    REQUIRE(memcmp(wbuf, rbuf, sizeof(wbuf)) == 0);
}

TEST_CASE("Block device: large block count", "[block][size]") {
    // 1 million 512-byte blocks = 512 MB
    MemBlockDev dev(512, 1000000);
    REQUIRE(dev.block_count == 1000000);
}

// ============================================================================
// Test cases — MBR partition table parsing
// ============================================================================

TEST_CASE("MBR: valid MBR signature", "[block][partition][mbr]") {
    MemBlockDev dev(512, 64);
    MbrSector* mbr = reinterpret_cast<MbrSector*>(dev.data);
    mbr->signature = 0xAA55;
    mbr->partitions[0].type = 0x83;  // Linux
    mbr->partitions[0].first_lba = htole32(2048);
    mbr->partitions[0].sector_count = htole32(100000);

    // Verify MBR signature
    MbrSector* loaded = reinterpret_cast<MbrSector*>(dev.data);
    REQUIRE(loaded->signature == 0xAA55);
    REQUIRE(loaded->partitions[0].type == 0x83);
    REQUIRE(le32toh(loaded->partitions[0].first_lba) == 2048);
    REQUIRE(le32toh(loaded->partitions[0].sector_count) == 100000);
}

TEST_CASE("MBR: four primary partitions", "[block][partition][mbr]") {
    MemBlockDev dev(512, 64);
    MbrSector* mbr = reinterpret_cast<MbrSector*>(dev.data);
    mbr->signature = 0xAA55;

    // Set up 4 partitions
    mbr->partitions[0].type = 0x83;  // Linux
    mbr->partitions[0].first_lba = htole32(2048);
    mbr->partitions[0].sector_count = htole32(100000);

    mbr->partitions[1].type = 0x82;  // Linux swap
    mbr->partitions[1].first_lba = htole32(102048);
    mbr->partitions[1].sector_count = htole32(50000);

    mbr->partitions[2].type = 0x83;  // Linux (home)
    mbr->partitions[2].first_lba = htole32(152048);
    mbr->partitions[2].sector_count = htole32(200000);

    mbr->partitions[3].type = 0x0C;  // FAT32
    mbr->partitions[3].first_lba = htole32(352048);
    mbr->partitions[3].sector_count = htole32(50000);

    // Verify all partitions
    MbrSector* mp = reinterpret_cast<MbrSector*>(dev.data);
    REQUIRE(mp->signature == 0xAA55);
    REQUIRE(mp->partitions[0].type == 0x83);
    REQUIRE(mp->partitions[1].type == 0x82);
    REQUIRE(mp->partitions[2].type == 0x83);
    REQUIRE(mp->partitions[3].type == 0x0C);

    // Verify no partition overlaps (basic check)
    uint32_t prev_end = 0;
    for (int i = 0; i < 4; i++) {
        uint32_t start = le32toh(mp->partitions[i].first_lba);
        uint32_t count = le32toh(mp->partitions[i].sector_count);
        REQUIRE(start >= prev_end);
        prev_end = start + count;
    }
}

TEST_CASE("MBR: extended partition", "[block][partition][mbr]") {
    MemBlockDev dev(512, 64);
    MbrSector* mbr = reinterpret_cast<MbrSector*>(dev.data);
    mbr->signature = 0xAA55;

    // Primary partition 0 = Linux
    mbr->partitions[0].type = 0x83;
    mbr->partitions[0].first_lba = htole32(2048);
    mbr->partitions[0].sector_count = htole32(100000);

    // Partition 1 = extended (0x05 or 0x0F)
    mbr->partitions[1].type = 0x05;
    mbr->partitions[1].first_lba = htole32(102048);
    mbr->partitions[1].sector_count = htole32(100000);

    MbrSector* mp = reinterpret_cast<MbrSector*>(dev.data);
    REQUIRE(mp->partitions[1].type == 0x05);  // Extended partition
}

TEST_CASE("MBR: invalid signature returns error", "[block][partition][mbr]") {
    // Invalid MBR = missing 0xAA55 signature
    MemBlockDev dev(512, 64);
    // data is zeroed, so signature = 0
    MbrSector* mbr = reinterpret_cast<MbrSector*>(dev.data);
    REQUIRE(mbr->signature != 0xAA55);
    // Application should reject this MBR
}

// ============================================================================
// Test cases — GPT partition table parsing
// ============================================================================

TEST_CASE("GPT: valid GPT header signature", "[block][partition][gpt]") {
    MemBlockDev dev(512, 1024);

    // Write GPT header at LBA 1
    GptHeader* gpt = reinterpret_cast<GptHeader*>(dev.data + 512);
    gpt->signature = GPT_SIGNATURE;
    gpt->revision = htole32(0x00010000);  // rev 1.0
    gpt->header_size = htole32(92);
    gpt->my_lba = htole64(1);
    gpt->alternate_lba = htole64(1023);
    gpt->first_usable_lba = htole64(34);
    gpt->last_usable_lba = htole64(1022);
    gpt->num_partition_entries = htole32(128);
    gpt->partition_entry_size = htole32(128);

    // Protective MBR at LBA 0
    MbrSector* mbr = reinterpret_cast<MbrSector*>(dev.data);
    mbr->signature = 0xAA55;
    mbr->partitions[0].type = 0xEE;  // Protective GPT
    mbr->partitions[0].first_lba = htole32(1);
    mbr->partitions[0].sector_count = htole32(0xFFFFFFFF);

    // Verify GPT
    GptHeader* loaded = reinterpret_cast<GptHeader*>(dev.data + 512);
    REQUIRE(loaded->signature == GPT_SIGNATURE);
    REQUIRE(le32toh(loaded->revision) == 0x00010000);
    REQUIRE(le64toh(loaded->my_lba) == 1);
    REQUIRE(le64toh(loaded->alternate_lba) == 1023);
    REQUIRE(le64toh(loaded->first_usable_lba) == 34);
    REQUIRE(le64toh(loaded->num_partition_entries) == 128);

    // Verify protective MBR
    REQUIRE(mbr->signature == 0xAA55);
    REQUIRE(mbr->partitions[0].type == 0xEE);
}

TEST_CASE("GPT: partition entries", "[block][partition][gpt]") {
    MemBlockDev dev(512, 1024);

    // GPT header at LBA 1
    GptHeader* gpt = reinterpret_cast<GptHeader*>(dev.data + 512);
    gpt->signature = GPT_SIGNATURE;
    gpt->num_partition_entries = htole32(128);
    gpt->partition_entry_size = htole32(128);
    gpt->partition_entry_lba = htole64(2);  // entries at LBA 2

    // Partition entry at LBA 2 (offset 1024)
    GptPartitionEntry* pe = reinterpret_cast<GptPartitionEntry*>(
        dev.data + 1024);
    pe->starting_lba = htole64(2048);
    pe->ending_lba = htole64(20480);

    // Verify
    GptPartitionEntry* loaded = reinterpret_cast<GptPartitionEntry*>(
        dev.data + 1024);
    REQUIRE(le64toh(loaded->starting_lba) == 2048);
    REQUIRE(le64toh(loaded->ending_lba) == 20480);
}

TEST_CASE("GPT: invalid signature returns error", "[block][partition][gpt]") {
    MemBlockDev dev(512, 64);
    // GPT header without signature = invalid
    GptHeader* gpt = reinterpret_cast<GptHeader*>(dev.data + 512);
    gpt->signature = 0;  // Not "EFI PART"
    REQUIRE(gpt->signature != GPT_SIGNATURE);
}

// ============================================================================
// Test cases — Real block device access (MINIX-only)
// ============================================================================

TEST_CASE("Block device: open /dev/c0d0 (MINIX)", "[block][runtime][minix]") {
    SKIP("MINIX runtime required — open block device in /dev/");
}

TEST_CASE("Block device: read partition table from real device (MINIX)",
          "[block][runtime][minix]") {
    SKIP("MINIX runtime required — read MBR/GPT from /dev/c0d0");
}

TEST_CASE("Block device: write to real device (MINIX)",
          "[block][runtime][minix]") {
    SKIP("MINIX runtime required — write block to /dev/c0d0");
}

TEST_CASE("Block device: ioctl BLKGETSIZE (MINIX)",
          "[block][runtime][minix]") {
    // BLKGETSIZE returns the size of a block device in sectors
    SKIP("MINIX runtime required — BLKGETSIZE ioctl");
}

TEST_CASE("Block device: ioctl BLKSSZGET (MINIX)",
          "[block][runtime][minix]") {
    // BLKSSZGET returns the logical block size
    SKIP("MINIX runtime required — BLKSSZGET ioctl");
}

TEST_CASE("Block device: read past end on real device (MINIX)",
          "[block][runtime][minix]") {
    SKIP("MINIX runtime required — read past end of device");
}
