//! Catch2 Data Store (DS) tests — key-value store semantics
//!
//! Phase 9.4b: C Test Migration — component tests
//! See planning/28_testing_framework_migration.md
//!
//! Validates DS flag encoding, key validation, data_store struct layout,
//! and subscription matching on the host. Based on the MINIX DS server
//! (minix/servers/ds/) and libsys interface (minix/lib/libsys/ds.c).
//!
//! The DS is a key-value store that MINIX services use to publish
//! and subscribe to system data (driver up events, I/O ranges, labels).

#include "catch.hpp"
#include <cstdint>
#include <cstring>
#include <string>
#include <regex>

// ============================================================================
// DS flag constants (from minix/include/minix/ds.h)
// ============================================================================

static constexpr int DSF_IN_USE           = 0x001;
static constexpr int DSF_PRIV_RETRIEVE    = 0x002;
static constexpr int DSF_PRIV_OVERWRITE   = 0x004;
static constexpr int DSF_PRIV_SUBSCRIBE   = 0x008;
static constexpr int DSF_TYPE_U32         = 0x010;
static constexpr int DSF_TYPE_STR         = 0x020;
static constexpr int DSF_TYPE_MEM         = 0x040;
static constexpr int DSF_TYPE_LABEL       = 0x100;
static constexpr int DSF_MASK_TYPE        = 0xFF0;
static constexpr int DSF_MASK_INTERNAL    = 0xFFF;
static constexpr int DSF_OVERWRITE        = 0x01000;
static constexpr int DSF_INITIAL          = 0x02000;

static constexpr int DS_MAX_KEYLEN        = 80;

// DS event codes
static constexpr int DS_DRIVER_UP         = 1;

// ============================================================================
// Data store entry struct (from minix/servers/ds/store.h)
// ============================================================================

struct DataStoreEntry {
    int         flags;
    char        key[80];            // DS_MAX_KEYLEN
    char        owner[80];          // DS_MAX_KEYLEN

    union {
        unsigned u32_val;
        struct {
            void*   data;
            size_t  length;
            size_t  reallen;
        } mem;
    } u;
};

// ============================================================================
// DS key validation helpers
// ============================================================================

// Validate a DS key: must be non-empty, null-terminated, max DS_MAX_KEYLEN
static bool ds_valid_key(const char* key) {
    if (key == nullptr) return false;
    size_t len = std::strlen(key);
    if (len == 0 || len > 79) return false;  // leave room for null
    // DS keys are ASCII strings (printable, no spaces)
    for (size_t i = 0; i < len; i++) {
        if (key[i] <= 0x20 || key[i] > 0x7E) return false;
    }
    return true;
}

// Get type from flags
static int ds_type_from_flags(int flags) {
    return flags & DSF_MASK_TYPE;
}

// Check if a flag combination is valid
static bool ds_valid_flags(int flags) {
    int type = ds_type_from_flags(flags);
    // Must have exactly one type
    int type_bits = 0;
    if (type & DSF_TYPE_U32)   type_bits++;
    if (type & DSF_TYPE_STR)   type_bits++;
    if (type & DSF_TYPE_MEM)   type_bits++;
    if (type & DSF_TYPE_LABEL) type_bits++;
    // Zero types means compatibility mode (no type enforcement)
    // More than one type is invalid
    if (type_bits > 1) return false;

    // DSF_IN_USE is set by the server, not user
    // DSF_OVERWRITE only valid with publish
    // DSF_INITIAL only valid with subscribe
    return true;
}

// ============================================================================
// Test cases — DS flags
// ============================================================================

TEST_CASE("DS flag type values match MINIX definitions", "[ds][flags]") {
    // Verify type flags are disjoint
    REQUIRE((DSF_TYPE_U32 & DSF_TYPE_STR) == 0);
    REQUIRE((DSF_TYPE_U32 & DSF_TYPE_MEM) == 0);
    REQUIRE((DSF_TYPE_U32 & DSF_TYPE_LABEL) == 0);
    REQUIRE((DSF_TYPE_STR & DSF_TYPE_MEM) == 0);
    REQUIRE((DSF_TYPE_STR & DSF_TYPE_LABEL) == 0);
    REQUIRE((DSF_TYPE_MEM & DSF_TYPE_LABEL) == 0);

    // Type mask covers all type flags
    REQUIRE((DSF_MASK_TYPE & DSF_TYPE_U32) == DSF_TYPE_U32);
    REQUIRE((DSF_MASK_TYPE & DSF_TYPE_STR) == DSF_TYPE_STR);
    REQUIRE((DSF_MASK_TYPE & DSF_TYPE_MEM) == DSF_TYPE_MEM);
    REQUIRE((DSF_MASK_TYPE & DSF_TYPE_LABEL) == DSF_TYPE_LABEL);

    // Internal mask covers all internal flags
    REQUIRE((DSF_MASK_INTERNAL & DSF_IN_USE) == DSF_IN_USE);
    REQUIRE((DSF_MASK_INTERNAL & DSF_PRIV_RETRIEVE) == DSF_PRIV_RETRIEVE);
    REQUIRE((DSF_MASK_INTERNAL & DSF_PRIV_OVERWRITE) == DSF_PRIV_OVERWRITE);
    REQUIRE((DSF_MASK_INTERNAL & DSF_PRIV_SUBSCRIBE) == DSF_PRIV_SUBSCRIBE);
}

TEST_CASE("DS type flag extraction from flags", "[ds][flags]") {
    // U32 type
    int flags = DSF_TYPE_U32 | DSF_PRIV_RETRIEVE;
    REQUIRE(ds_type_from_flags(flags) == DSF_TYPE_U32);
    REQUIRE(ds_valid_flags(flags));

    // String type
    flags = DSF_TYPE_STR | DSF_OVERWRITE;
    REQUIRE(ds_type_from_flags(flags) == DSF_TYPE_STR);
    REQUIRE(ds_valid_flags(flags));

    // Memory type
    flags = DSF_TYPE_MEM;
    REQUIRE(ds_type_from_flags(flags) == DSF_TYPE_MEM);
    REQUIRE(ds_valid_flags(flags));

    // Label type
    flags = DSF_TYPE_LABEL | DSF_PRIV_SUBSCRIBE;
    REQUIRE(ds_type_from_flags(flags) == DSF_TYPE_LABEL);
    REQUIRE(ds_valid_flags(flags));
}

TEST_CASE("DS multiple type flags is invalid", "[ds][flags]") {
    // Two types simultaneously
    int flags = DSF_TYPE_U32 | DSF_TYPE_STR;
    REQUIRE_FALSE(ds_valid_flags(flags));

    // Three types
    flags = DSF_TYPE_U32 | DSF_TYPE_STR | DSF_TYPE_MEM;
    REQUIRE_FALSE(ds_valid_flags(flags));
}

TEST_CASE("DS zero type flags is valid (compat mode)", "[ds][flags]") {
    // No type flags — compatibility mode
    REQUIRE(ds_valid_flags(0));
    REQUIRE(ds_valid_flags(DSF_PRIV_RETRIEVE));
    REQUIRE(ds_valid_flags(DSF_OVERWRITE | DSF_INITIAL));
}

TEST_CASE("DS OVERWRITE and INITIAL flags are outside type mask", "[ds][flags]") {
    // These are publish/subscribe flags, not type flags
    REQUIRE((DSF_OVERWRITE & DSF_MASK_TYPE) == 0);
    REQUIRE((DSF_INITIAL & DSF_MASK_TYPE) == 0);
    REQUIRE((DSF_OVERWRITE & DSF_MASK_INTERNAL) == 0);
    REQUIRE((DSF_INITIAL & DSF_MASK_INTERNAL) == 0);
}

// ============================================================================
// Test cases — DS key validation
// ============================================================================

TEST_CASE("DS valid keys", "[ds][key]") {
    REQUIRE(ds_valid_key("test.key"));
    REQUIRE(ds_valid_key("driver.pci"));
    REQUIRE(ds_valid_key("a"));          // single char
    REQUIRE(ds_valid_key("0123456789012345678901234567890123456789"
                         "0123456789012345678901234567890123456789"));  // 79 chars
}

TEST_CASE("DS invalid keys", "[ds][key]") {
    REQUIRE_FALSE(ds_valid_key(nullptr));
    REQUIRE_FALSE(ds_valid_key(""));     // empty
    REQUIRE_FALSE(ds_valid_key("key\nwith\nnewlines"));
    REQUIRE_FALSE(ds_valid_key("key\twith\ttabs"));
    REQUIRE_FALSE(ds_valid_key("key with spaces"));
    REQUIRE_FALSE(ds_valid_key("binary\x00key"));  // embedded null
}

TEST_CASE("DS key length limits", "[ds][key]") {
    REQUIRE(DS_MAX_KEYLEN == 80);

    // 79 chars is ok (DS_MAX_KEYLEN - 1 for null terminator)
    std::string maxKey(79, 'x');
    REQUIRE(ds_valid_key(maxKey.c_str()));

    // 80 chars without null -> not checked by strlen, but the buffer has 80
    // In MINIX DS, writing a key longer than DS_MAX_KEYLEN-1 would truncate
}

TEST_CASE("DS key dot notation", "[ds][key]") {
    // DS keys use dot notation for hierarchy: "driver.pci.00:01.0"
    REQUIRE(ds_valid_key("driver"));
    REQUIRE(ds_valid_key("driver.pci"));
    REQUIRE(ds_valid_key("driver.pci.00:01.0"));
    REQUIRE(ds_valid_key("service.vfs"));
    REQUIRE(ds_valid_key("service.pm.data_store"));
}

// ============================================================================
// Test cases — DataStoreEntry struct layout
// ============================================================================

TEST_CASE("DS entry struct size", "[ds][struct]") {
    // flags (4) + key (80) + owner (80) = 164 bytes base
    // union: u32 (4) or mem struct (ptr 8 + size_t*2 = 24 on 64-bit)
    // So total = 4 + 80 + 80 + max(4, 24) = 188 on 64-bit
    // With alignment: key aligned to 4, owner to 4, union to 8
    REQUIRE(sizeof(int) == 4);
    REQUIRE(sizeof(DataStoreEntry) >= 4 + 80 + 80);
}

TEST_CASE("DS entry key offset", "[ds][struct]") {
    DataStoreEntry entry;
    std::memset(&entry, 0, sizeof(entry));

    // Verify key field is at offset sizeof(int) (= 4)
    // by checking that writing to key doesn't overwrite flags
    entry.flags = DSF_TYPE_U32 | DSF_IN_USE;
    std::strcpy(entry.key, "test.key");
    REQUIRE(entry.flags == (DSF_TYPE_U32 | DSF_IN_USE));
    REQUIRE(std::strcmp(entry.key, "test.key") == 0);
}

TEST_CASE("DS entry owner field is separate from key", "[ds][struct]") {
    DataStoreEntry entry;
    std::memset(&entry, 0, sizeof(entry));

    std::strcpy(entry.key, "some.service");
    std::strcpy(entry.owner, "vfs");
    REQUIRE(std::strcmp(entry.key, "some.service") == 0);
    REQUIRE(std::strcmp(entry.owner, "vfs") == 0);

    // Write to owner doesn't corrupt key
    std::strcpy(entry.owner, "pm");
    REQUIRE(std::strcmp(entry.key, "some.service") == 0);
    REQUIRE(std::strcmp(entry.owner, "pm") == 0);
}

TEST_CASE("DS entry U32 value storage", "[ds][struct]") {
    DataStoreEntry entry;
    std::memset(&entry, 0, sizeof(entry));

    entry.flags = DSF_TYPE_U32 | DSF_IN_USE;
    entry.u.u32_val = 0xDEADBEEF;
    REQUIRE(entry.u.u32_val == 0xDEADBEEF);

    // Changing flags doesn't affect value
    entry.flags = DSF_TYPE_U32 | DSF_IN_USE | DSF_PRIV_RETRIEVE;
    REQUIRE(entry.u.u32_val == 0xDEADBEEF);
}

TEST_CASE("DS entry MEM value storage", "[ds][struct]") {
    DataStoreEntry entry;
    std::memset(&entry, 0, sizeof(entry));

    char test_data[] = "hello world";
    entry.flags = DSF_TYPE_MEM | DSF_IN_USE;
    entry.u.mem.data = test_data;
    entry.u.mem.length = 12;
    entry.u.mem.reallen = 12;

    REQUIRE(entry.u.mem.data == test_data);
    REQUIRE(entry.u.mem.length == 12);
    REQUIRE(entry.u.mem.reallen == 12);

    // MEM and U32 share the same union
    // Reading u32 after setting mem is undefined by union rules
}

TEST_CASE("DS entry zero-initialized flags are 'not in use'", "[ds][struct]") {
    DataStoreEntry entry;
    std::memset(&entry, 0, sizeof(entry));
    REQUIRE((entry.flags & DSF_IN_USE) == 0);
}

// ============================================================================
// Test cases — DS subscription regex matching
// ============================================================================

TEST_CASE("DS subscription regex: simple wildcard", "[ds][sub]") {
    // DS uses POSIX regex for subscriptions (from store.c)
    std::regex re("driver\\..*");

    REQUIRE(std::regex_match("driver.pci", re));
    REQUIRE(std::regex_match("driver.virtio_blk", re));
    REQUIRE(std::regex_match("driver.ahci", re));
    REQUIRE_FALSE(std::regex_match("service.pm", re));
    REQUIRE_FALSE(std::regex_match("driver", re));  // single word, no dot
}

TEST_CASE("DS subscription regex: exact match", "[ds][sub]") {
    // Exact subscription
    std::regex re("service\\.vfs");

    REQUIRE(std::regex_match("service.vfs", re));
    REQUIRE_FALSE(std::regex_match("service.pm", re));
    REQUIRE_FALSE(std::regex_match("service.vfs.extra", re));
}

TEST_CASE("DS subscription regex: dot prefix wildcard", "[ds][sub]") {
    // Match any key starting with "ds."
    std::regex re("ds\\..*");

    REQUIRE(std::regex_match("ds.test", re));
    REQUIRE(std::regex_match("ds.keys", re));
    REQUIRE(std::regex_match("ds.subscriptions", re));

    // But not exact "ds" without dot
    REQUIRE_FALSE(std::regex_match("ds", re));
    REQUIRE_FALSE(std::regex_match("ds_alt", re));
}

TEST_CASE("DS subscription regex: driver up events", "[ds][sub]") {
    std::regex re("event\\..*");
    REQUIRE(std::regex_match("event.driver_up", re));
    REQUIRE(std::regex_match("event.driver_down", re));
    REQUIRE_FALSE(std::regex_match("driver.pci", re));
}

TEST_CASE("DS subscription regex: all keys", "[ds][sub]") {
    // Regex that matches any DS key
    std::regex re(".*");

    REQUIRE(std::regex_match("driver.pci", re));
    REQUIRE(std::regex_match("service.vfs", re));
    REQUIRE(std::regex_match("ds.keys", re));
    REQUIRE(std::regex_match("event.driver_up", re));
    REQUIRE(std::regex_match("a", re));
    REQUIRE(std::regex_match("", re));  // empty matches .*
}

TEST_CASE("DS subscription regex: prefix match", "[ds][sub]") {
    // Match keys that start with "service."
    std::regex re("service\\..*");

    REQUIRE(std::regex_match("service.vfs", re));
    REQUIRE(std::regex_match("service.pm", re));
    REQUIRE(std::regex_match("service.rs", re));
    REQUIRE(std::regex_match("service.vm", re));
    REQUIRE(std::regex_match("service.ds", re));
    REQUIRE(std::regex_match("service.sched", re));
    REQUIRE(std::regex_match("service.mib", re));
    REQUIRE(std::regex_match("service.auditd", re));
    REQUIRE(std::regex_match("service.macd", re));

    REQUIRE_FALSE(std::regex_match("driver.pci", re));
    REQUIRE_FALSE(std::regex_match("driver.ahci", re));
}

// ============================================================================
// Test cases — DS constants
// ============================================================================

TEST_CASE("DS key/owner field size constant", "[ds][const]") {
    // DS_MAX_KEYLEN = 80
    REQUIRE(DS_MAX_KEYLEN == 80);

    // Verify that key and owner fields in the struct match
    // the declared constant
    DataStoreEntry entry;
    REQUIRE(sizeof(entry.key) == DS_MAX_KEYLEN);
    REQUIRE(sizeof(entry.owner) == DS_MAX_KEYLEN);
    REQUIRE(sizeof(entry.key) >= 80);
    REQUIRE(sizeof(entry.owner) >= 80);
}

TEST_CASE("DS MAX_KEYLEN usage in struct", "[ds][const]") {
    // DS_MAX_KEYLEN = 80, must match struct field size
    REQUIRE(DS_MAX_KEYLEN >= 80);
}

// ============================================================================
// Test cases — DS server operations (MINIX-only)
// ============================================================================

TEST_CASE("DS publish U32 (MINIX)", "[ds][runtime][minix]") {
    SKIP("MINIX runtime required — ds_publish_u32 via IPC to DS server");
}

TEST_CASE("DS retrieve U32 (MINIX)", "[ds][runtime][minix]") {
    SKIP("MINIX runtime required — ds_retrieve_u32");
}

TEST_CASE("DS publish and retrieve string (MINIX)", "[ds][runtime][minix]") {
    SKIP("MINIX runtime required — ds_publish_str / ds_retrieve_str");
}

TEST_CASE("DS publish and retrieve memory (MINIX)", "[ds][runtime][minix]") {
    SKIP("MINIX runtime required — ds_publish_mem / ds_retrieve_mem");
}

TEST_CASE("DS label publish and lookup (MINIX)", "[ds][runtime][minix]") {
    SKIP("MINIX runtime required — ds_publish_label / ds_retrieve_label_endpt");
}

TEST_CASE("DS subscribe and check (MINIX)", "[ds][runtime][minix]") {
    SKIP("MINIX runtime required — ds_subscribe / ds_check");
}

TEST_CASE("DS delete entry (MINIX)", "[ds][runtime][minix]") {
    SKIP("MINIX runtime required — ds_delete_u32 / ds_delete_str");
}

TEST_CASE("DS snapshot map (MINIX)", "[ds][runtime][minix]") {
    SKIP("MINIX runtime required — ds_snapshot_map / ds_retrieve_map");
}
