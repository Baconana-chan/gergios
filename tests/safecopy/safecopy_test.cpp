//! Catch2 Safecopy tests — grant-based interprocess memory copy
//!
//! Phase 9.4b: C Test Migration — component tests
//! See planning/28_testing_framework_migration.md
//!
//! Validates grant ID encoding/decoding, flag combinations,
//! cp_grant_t struct layout, and grant type semantics on the host.
//! Based on MINIX safecopies.h (minix/include/minix/safecopies.h)
//! and the kernel grant table implementation.
//!
//! Safecopy allows MINIX processes to grant access to portions of
//! their address space to other processes, with read/write permissions.
//! Grants can be DIRECT (to another process), INDIRECT (via another
//! grant), or MAGIC (any-to-any).

#include "catch.hpp"
#include <cstdint>
#include <cstring>
#include <climits>

// ============================================================================
// Safecopy constants (from minix/include/minix/safecopies.h)
// ============================================================================

// Grant types
static constexpr int CPF_READ        = 0x000001;
static constexpr int CPF_WRITE       = 0x000002;
static constexpr int CPF_TRY         = 0x000010;

// Internal flags
static constexpr int CPF_USED        = 0x000100;
static constexpr int CPF_DIRECT      = 0x000200;
static constexpr int CPF_INDIRECT    = 0x000400;
static constexpr int CPF_MAGIC       = 0x000800;
static constexpr int CPF_VALID       = 0x001000;

// Grant ID encoding
static constexpr int GRANT_SHIFT     = 20;
static constexpr int GRANT_MAX_SEQ   = (1 << (31 - GRANT_SHIFT));  // 2048
static constexpr int GRANT_MAX_IDX   = (1 << GRANT_SHIFT);         // 1048576

// Special grant ID
using cp_grant_id_t = int;

static constexpr cp_grant_id_t GRANT_INVALID = -1;

// Grant fault return
static constexpr int GRANT_FAULTED   = 1;

// ============================================================================
// Grant ID encoding/decoding helpers
// ============================================================================

// Encode grant ID from index and sequence number
static cp_grant_id_t grant_id(int idx, int seq) {
    return (cp_grant_id_t)((seq << GRANT_SHIFT) | (idx));
}

// Extract sequence number from grant ID
static int grant_seq(cp_grant_id_t g) {
    return (g >> GRANT_SHIFT) & (GRANT_MAX_SEQ - 1);
}

// Extract index from grant ID
static int grant_idx(cp_grant_id_t g) {
    return g & (GRANT_MAX_IDX - 1);
}

// Check if grant ID is valid
static bool grant_valid(cp_grant_id_t g) {
    return g > GRANT_INVALID;
}

// ============================================================================
// cp_grant_t structure (from minix/include/minix/safecopies.h)
// ============================================================================

// Endpoint type
using endpoint_t = int;

// Grant structure — each entry in the grant table
struct cp_grant_t {
    int         cp_flags;           // CPF_* above
    int         cp_seq;             // sequence number

    union {
        struct {
            // CPF_DIRECT
            endpoint_t  cp_who_to;    // grantee
            uintptr_t   cp_start;     // memory start (vir_bytes)
            size_t      cp_len;       // size in bytes
        } cp_direct;

        struct {
            // CPF_INDIRECT
            endpoint_t       cp_who_to;     // grantee
            endpoint_t       cp_who_from;   // previous granter
            cp_grant_id_t    cp_grant;      // previous grant
        } cp_indirect;

        struct {
            // CPF_MAGIC
            endpoint_t   cp_who_from;   // granter
            endpoint_t   cp_who_to;     // grantee
            uintptr_t    cp_start;      // memory
            size_t       cp_len;        // size in bytes
        } cp_magic;

        struct {
            // free slot
            int cp_next;             // next free or -1
        } cp_free;
    } cp_u;

    cp_grant_id_t cp_faulted;       // soft fault marker (CPF_TRY only)
};

// ============================================================================
// Vectored safecopy structure
// ============================================================================

struct vscp_vec {
    endpoint_t      v_from;         // source (must be SELF)
    endpoint_t      v_to;           // destination (must be SELF)
    cp_grant_id_t   v_gid;          // grant id of other process
    size_t          v_offset;       // offset in other grant
    uintptr_t       v_addr;         // address in copier's space
    size_t          v_bytes;        // no. of bytes
};

// ============================================================================
// Test cases — Grant ID encoding/decoding
// ============================================================================

TEST_CASE("Grant ID encoding from index and sequence", "[safecopy][grantid]") {
    // Basic encoding
    cp_grant_id_t g = grant_id(0, 0);
    REQUIRE(g == 0);
    REQUIRE(grant_valid(g));
    REQUIRE(grant_seq(g) == 0);
    REQUIRE(grant_idx(g) == 0);
}

TEST_CASE("Grant ID index in lower 20 bits", "[safecopy][grantid]") {
    cp_grant_id_t g = grant_id(1, 0);
    REQUIRE(g == 1);
    REQUIRE(grant_idx(g) == 1);
    REQUIRE(grant_seq(g) == 0);

    g = grant_id(GRANT_MAX_IDX - 1, 0);
    REQUIRE(g == GRANT_MAX_IDX - 1);
    REQUIRE(grant_idx(g) == GRANT_MAX_IDX - 1);
}

TEST_CASE("Grant ID sequence in upper 11 bits", "[safecopy][grantid]") {
    cp_grant_id_t g = grant_id(0, 1);
    REQUIRE(g == (1 << GRANT_SHIFT));
    REQUIRE(grant_seq(g) == 1);
    REQUIRE(grant_idx(g) == 0);

    g = grant_id(0, 5);
    REQUIRE(g == (5 << GRANT_SHIFT));
    REQUIRE(grant_seq(g) == 5);
}

TEST_CASE("Grant ID combined index and sequence", "[safecopy][grantid]") {
    cp_grant_id_t g = grant_id(42, 7);
    REQUIRE(grant_idx(g) == 42);
    REQUIRE(grant_seq(g) == 7);
    REQUIRE(grant_valid(g));
}

TEST_CASE("Grant ID roundtrip", "[safecopy][grantid]") {
    for (int idx = 0; idx < 100; idx++) {
        for (int seq = 0; seq < 10; seq++) {
            cp_grant_id_t g = grant_id(idx, seq);
            REQUIRE(grant_idx(g) == idx);
            REQUIRE(grant_seq(g) == seq);
        }
    }
}

TEST_CASE("Grant ID maximum values", "[safecopy][grantid]") {
    // Max idx = 2^20 - 1 = 1048575
    cp_grant_id_t g = grant_id(GRANT_MAX_IDX - 1, 0);
    REQUIRE(grant_idx(g) == GRANT_MAX_IDX - 1);
    REQUIRE(grant_seq(g) == 0);

    // Max seq = 2^11 - 1 = 2047
    g = grant_id(0, GRANT_MAX_SEQ - 1);
    REQUIRE(grant_idx(g) == 0);
    REQUIRE(grant_seq(g) == GRANT_MAX_SEQ - 1);

    // Max both
    g = grant_id(GRANT_MAX_IDX - 1, GRANT_MAX_SEQ - 1);
    REQUIRE(grant_idx(g) == GRANT_MAX_IDX - 1);
    REQUIRE(grant_seq(g) == GRANT_MAX_SEQ - 1);
}

TEST_CASE("Grant ID negative is invalid", "[safecopy][grantid]") {
    REQUIRE_FALSE(grant_valid(-1));
    REQUIRE_FALSE(grant_valid(-100));
    REQUIRE_FALSE(grant_valid(GRANT_INVALID));
}

TEST_CASE("Grant ID has no overlap between idx and seq bits", "[safecopy][grantid]") {
    // The shift ensures no overlap in bit positions
    int idx_mask = (1 << GRANT_SHIFT) - 1;
    int seq_mask = (GRANT_MAX_SEQ - 1) << GRANT_SHIFT;
    REQUIRE((idx_mask & seq_mask) == 0);
}

// ============================================================================
// Test cases — Grant flags
// ============================================================================

TEST_CASE("Grant access flags are distinct", "[safecopy][flags]") {
    REQUIRE(CPF_READ != CPF_WRITE);
    REQUIRE(CPF_READ != CPF_TRY);
    REQUIRE(CPF_WRITE != CPF_TRY);
    REQUIRE((CPF_READ & CPF_WRITE) == 0);
    REQUIRE((CPF_READ & CPF_TRY) == 0);
}

TEST_CASE("Grant internal flags are distinct", "[safecopy][flags]") {
    REQUIRE(CPF_USED != CPF_DIRECT);
    REQUIRE(CPF_USED != CPF_INDIRECT);
    REQUIRE(CPF_USED != CPF_MAGIC);
    REQUIRE(CPF_USED != CPF_VALID);
    REQUIRE(CPF_DIRECT != CPF_INDIRECT);
    REQUIRE(CPF_DIRECT != CPF_MAGIC);
    REQUIRE(CPF_INDIRECT != CPF_MAGIC);

    // Internal flags don't overlap with access flags
    REQUIRE((CPF_USED & CPF_READ) == 0);
    REQUIRE((CPF_DIRECT & CPF_WRITE) == 0);
    REQUIRE((CPF_INDIRECT & CPF_TRY) == 0);
}

TEST_CASE("Grant type flags are mutually exclusive", "[safecopy][flags]") {
    int type_flags = CPF_DIRECT | CPF_INDIRECT | CPF_MAGIC;

    // Each type is a single bit
    REQUIRE((CPF_DIRECT & CPF_INDIRECT) == 0);
    REQUIRE((CPF_DIRECT & CPF_MAGIC) == 0);
    REQUIRE((CPF_INDIRECT & CPF_MAGIC) == 0);

    // A grant should have exactly one type
    REQUIRE((CPF_DIRECT & type_flags) == CPF_DIRECT);
    REQUIRE((CPF_INDIRECT & type_flags) == CPF_INDIRECT);
    REQUIRE((CPF_MAGIC & type_flags) == CPF_MAGIC);
}

TEST_CASE("Grant flags valid combination: direct + read", "[safecopy][flags]") {
    int flags = CPF_USED | CPF_VALID | CPF_DIRECT | CPF_READ;
    // Direct grant with read permission
    REQUIRE((flags & CPF_DIRECT) == CPF_DIRECT);
    REQUIRE((flags & CPF_READ) == CPF_READ);
    REQUIRE((flags & CPF_WRITE) == 0);  // no write
}

TEST_CASE("Grant flags valid combination: magic + read/write", "[safecopy][flags]") {
    int flags = CPF_USED | CPF_VALID | CPF_MAGIC | CPF_READ | CPF_WRITE;
    REQUIRE((flags & CPF_MAGIC) == CPF_MAGIC);
    REQUIRE((flags & CPF_READ) == CPF_READ);
    REQUIRE((flags & CPF_WRITE) == CPF_WRITE);
}

TEST_CASE("Grant flags valid combination: indirect", "[safecopy][flags]") {
    int flags = CPF_USED | CPF_VALID | CPF_INDIRECT | CPF_READ;
    REQUIRE((flags & CPF_INDIRECT) == CPF_INDIRECT);
    REQUIRE((flags & CPF_TRY) == 0);  // try is optional
}

TEST_CASE("Grant flags: TRY for soft fault", "[safecopy][flags]") {
    int flags = CPF_USED | CPF_VALID | CPF_DIRECT | CPF_READ | CPF_TRY;
    REQUIRE((flags & CPF_TRY) == CPF_TRY);
}

// ============================================================================
// Test cases — Grant structure layout
// ============================================================================

TEST_CASE("cp_grant_t struct size is reasonable", "[safecopy][struct]") {
    // flags (4) + seq (4) = 8
    // union: direct/indirect/magic (each has endpoint_t + ptr + size)
    //   direct:   endpoint(4) + ptr(8) + size_t(8) = 20
    //   indirect: endpoint(4) + endpoint(4) + grant_id(4) = 12
    //   magic:    endpoint(4) + endpoint(4) + ptr(8) + size_t(8) = 24
    // Max union = 24, with padding for alignment
    // cp_faulted (4)
    // Total: 8 + 24 + 4 = 36 on 64-bit, but alignment pads to 40
    REQUIRE(sizeof(cp_grant_t) >= 36);
    REQUIRE(sizeof(cp_grant_t) <= 48);  // allow some padding
}

TEST_CASE("cp_grant_t union sizes match", "[safecopy][struct]") {
    // The three grant types share the same union
    // The largest determines overall union size
    REQUIRE(sizeof(cp_grant_t::cp_u) >= sizeof(cp_grant_t::cp_direct));
    REQUIRE(sizeof(cp_grant_t::cp_u) >= sizeof(cp_grant_t::cp_indirect));
    REQUIRE(sizeof(cp_grant_t::cp_u) >= sizeof(cp_grant_t::cp_magic));
}

TEST_CASE("cp_grant_t direct field layout", "[safecopy][struct]") {
    cp_grant_t grant;
    std::memset(&grant, 0, sizeof(grant));

    grant.cp_flags = CPF_USED | CPF_VALID | CPF_DIRECT | CPF_READ;
    grant.cp_seq = 1;
    grant.cp_u.cp_direct.cp_who_to = 0x1234;  // endpoint
    grant.cp_u.cp_direct.cp_start = 0xDEADBEEF;
    grant.cp_u.cp_direct.cp_len = 4096;
    grant.cp_faulted = GRANT_INVALID;

    // Verify fields
    REQUIRE(grant.cp_flags == (CPF_USED | CPF_VALID | CPF_DIRECT | CPF_READ));
    REQUIRE(grant.cp_seq == 1);
    REQUIRE(grant.cp_u.cp_direct.cp_who_to == 0x1234);
    REQUIRE(grant.cp_u.cp_direct.cp_start == 0xDEADBEEF);
    REQUIRE(grant.cp_u.cp_direct.cp_len == 4096);
    REQUIRE(grant.cp_faulted == GRANT_INVALID);
}

TEST_CASE("cp_grant_t indirect field layout", "[safecopy][struct]") {
    cp_grant_t grant;
    std::memset(&grant, 0, sizeof(grant));

    grant.cp_flags = CPF_USED | CPF_VALID | CPF_INDIRECT | CPF_READ;
    grant.cp_seq = 5;
    grant.cp_u.cp_indirect.cp_who_to = 0x5678;
    grant.cp_u.cp_indirect.cp_who_from = 0x9ABC;
    grant.cp_u.cp_indirect.cp_grant = grant_id(42, 3);
    grant.cp_faulted = GRANT_INVALID;

    REQUIRE(grant.cp_flags == (CPF_USED | CPF_VALID | CPF_INDIRECT | CPF_READ));
    REQUIRE(grant.cp_seq == 5);
    REQUIRE(grant.cp_u.cp_indirect.cp_who_to == 0x5678);
    REQUIRE(grant.cp_u.cp_indirect.cp_who_from == 0x9ABC);
    REQUIRE(grant.cp_u.cp_indirect.cp_grant == grant_id(42, 3));
    REQUIRE(grant.cp_faulted == GRANT_INVALID);
}

TEST_CASE("cp_grant_t magic field layout", "[safecopy][struct]") {
    cp_grant_t grant;
    std::memset(&grant, 0, sizeof(grant));

    grant.cp_flags = CPF_USED | CPF_VALID | CPF_MAGIC | CPF_READ | CPF_WRITE;
    grant.cp_seq = 10;
    grant.cp_u.cp_magic.cp_who_from = 0x1111;
    grant.cp_u.cp_magic.cp_who_to = 0x2222;
    grant.cp_u.cp_magic.cp_start = 0xCAFEBABE;
    grant.cp_u.cp_magic.cp_len = 8192;
    grant.cp_faulted = GRANT_INVALID;

    REQUIRE(grant.cp_flags == (CPF_USED | CPF_VALID | CPF_MAGIC | CPF_READ | CPF_WRITE));
    REQUIRE(grant.cp_seq == 10);
    REQUIRE(grant.cp_u.cp_magic.cp_who_from == 0x1111);
    REQUIRE(grant.cp_u.cp_magic.cp_who_to == 0x2222);
    REQUIRE(grant.cp_u.cp_magic.cp_start == 0xCAFEBABE);
    REQUIRE(grant.cp_u.cp_magic.cp_len == 8192);
    REQUIRE(grant.cp_faulted == GRANT_INVALID);
}

TEST_CASE("cp_grant_t free slot layout", "[safecopy][struct]") {
    cp_grant_t grant;
    std::memset(&grant, 0, sizeof(grant));

    // Free slot: no flags set, cp_next points to next free
    grant.cp_flags = 0;
    grant.cp_u.cp_free.cp_next = -1;  // end of free list

    REQUIRE(grant.cp_flags == 0);
    REQUIRE(grant.cp_u.cp_free.cp_next == -1);

    // Free chain: grant[5] -> grant[10] -> end
    grant.cp_u.cp_free.cp_next = 10;
    REQUIRE(grant.cp_u.cp_free.cp_next == 10);
}

// ============================================================================
// Test cases — vscp_vec structure
// ============================================================================

TEST_CASE("vscp_vec struct size and layout", "[safecopy][struct]") {
    vscp_vec vec;
    std::memset(&vec, 0, sizeof(vec));

    vec.v_from = 0x1000;
    vec.v_to = 0x2000;
    vec.v_gid = grant_id(10, 2);
    vec.v_offset = 512;
    vec.v_addr = 0xBEEF0000;
    vec.v_bytes = 1024;

    REQUIRE(vec.v_from == 0x1000);
    REQUIRE(vec.v_to == 0x2000);
    REQUIRE(vec.v_gid == grant_id(10, 2));
    REQUIRE(vec.v_offset == 512);
    REQUIRE(vec.v_addr == 0xBEEF0000);
    REQUIRE(vec.v_bytes == 1024);

    // Reasonable size
    REQUIRE(sizeof(vscp_vec) >= 24);   // 6 fields × 4-8 bytes each
    REQUIRE(sizeof(vscp_vec) <= 40);   // with alignment
}

// ============================================================================
// Test cases — Grant limits and bounds
// ============================================================================

TEST_CASE("Grant shift/exponent relationship", "[safecopy][limits]") {
    // As per MINIX: GRANT_SHIFT = 20
    // GRANT_MAX_SEQ = 1 << (31 - GRANT_SHIFT) = 1 << 11 = 2048
    // GRANT_MAX_IDX = 1 << GRANT_SHIFT = 1 << 20 = 1048576
    REQUIRE(GRANT_SHIFT == 20);
    REQUIRE(GRANT_MAX_SEQ == 2048);
    REQUIRE(GRANT_MAX_IDX == 1048576);

    // Total grant slots: GRANT_MAX_IDX = ~1M
    // Total sequences: GRANT_MAX_SEQ = 2048
    // This allows 2^31 unique grant IDs (positive int range)
}

TEST_CASE("Grant ID fits in positive signed int", "[safecopy][limits]") {
    // Grant ID is a signed int (31-bit positive)
    // Max value should be < INT_MAX / 2
    cp_grant_id_t max_id = grant_id(GRANT_MAX_IDX - 1, GRANT_MAX_SEQ - 1);
    REQUIRE(max_id > 0);
    REQUIRE(max_id < INT_MAX);
}

// ============================================================================
// Test cases — Grant fault handling
// ============================================================================

TEST_CASE("Grant faulted marker", "[safecopy][fault]") {
    // GRANT_FAULTED is returned by cpf_revoke when a CPF_TRY grant
    // encountered a soft fault
    REQUIRE(GRANT_FAULTED == 1);

    cp_grant_t grant;
    std::memset(&grant, 0, sizeof(grant));

    // No fault
    grant.cp_faulted = GRANT_INVALID;
    REQUIRE(grant.cp_faulted == GRANT_INVALID);

    // Fault occurred
    grant.cp_faulted = grant_id(5, 1);
    REQUIRE(grant_valid(grant.cp_faulted));
}

// ============================================================================
// Test cases — MINIX-dependent (kernel safecopy syscalls)
// ============================================================================

TEST_CASE("Safecopy grant direct via cpf_grant_direct (MINIX)",
          "[safecopy][runtime][minix]") {
    SKIP("MINIX runtime required — cpf_grant_direct allocates grant in kernel");
}

TEST_CASE("Safecopy grant indirect via cpf_grant_indirect (MINIX)",
          "[safecopy][runtime][minix]") {
    SKIP("MINIX runtime required — cpf_grant_indirect chains grants");
}

TEST_CASE("Safecopy grant magic via cpf_grant_magic (MINIX)",
          "[safecopy][runtime][minix]") {
    SKIP("MINIX runtime required — cpf_grant_magic any-to-any grant");
}

TEST_CASE("Safecopy revoke grant via cpf_revoke (MINIX)",
          "[safecopy][runtime][minix]") {
    SKIP("MINIX runtime required — cpf_revoke invalidates grant");
}

TEST_CASE("Safecopy syscall SYS_SAFECOPYFROM (MINIX)",
          "[safecopy][runtime][minix]") {
    SKIP("MINIX runtime required — kernel safecopyfrom system call");
}

TEST_CASE("Safecopy syscall SYS_SAFECOPYTO (MINIX)",
          "[safecopy][runtime][minix]") {
    SKIP("MINIX runtime required — kernel safecopyto system call");
}

TEST_CASE("Safecopy vectored copy SYS_VSAFECOPY (MINIX)",
          "[safecopy][runtime][minix]") {
    SKIP("MINIX runtime required — kernel vectored safecopy system call");
}

TEST_CASE("Safecopy prealloc grants via cpf_prealloc (MINIX)",
          "[safecopy][runtime][minix]") {
    SKIP("MINIX runtime required — cpf_prealloc initializes kernel grant table");
}

TEST_CASE("Safecopy reload via cpf_reload (MINIX)",
          "[safecopy][runtime][minix]") {
    SKIP("MINIX runtime required — cpf_reload reloads grants after fork");
}

TEST_CASE("Safecopy test: grantor-requestor pattern (MINIX)",
          "[safecopy][runtime][minix]") {
    SKIP("MINIX runtime required — grantor/requestor FIFO-based test pattern");
}
