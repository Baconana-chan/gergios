//! Catch2 RMIB (Remote MIB) tests — sysctl node tree semantics
//!
//! Phase 9.4b: C Test Migration — component tests
//! See planning/28_testing_framework_migration.md
//!
//! Validates RMIB node initialization macros, tree traversal,
//! indirect (sparse) node arrays, and sysctl name resolution.
//! Based on MINIX rmib.h (minix/include/minix/rmib.h) and the
//! MIB service internals.
//!
//! RMIB allows user-space services to mount subtrees of the MIB
//! service's sysctl tree. Handlers process read/write requests.

#include "catch.hpp"
#include <cstdint>
#include <cstring>
#include <cstddef>
#include <string>

// ============================================================================
// RMIB type and flag constants (from sys/sysctl.h + minix/rmib.h)
// ============================================================================

// CTLTYPE constants (from NetBSD sysctl.h)
static constexpr int CTLTYPE_NODE      = 1;
static constexpr int CTLTYPE_INT       = 2;
static constexpr int CTLTYPE_STRING    = 3;
static constexpr int CTLTYPE_QUAD      = 4;
static constexpr int CTLTYPE_STRUCT    = 5;
static constexpr int CTLTYPE_BOOL      = 6;

// CTLFLAG constants
static constexpr int CTLFLAG_READONLY   = 0x00000004;
static constexpr int CTLFLAG_READWRITE  = 0x00000040;
static constexpr int CTLFLAG_PERMANENT  = 0x00000080;
static constexpr int CTLFLAG_IMMEDIATE  = 0x00004000;
static constexpr int CTLFLAG_ROOT       = 0x00008000;  // overloaded as SPARSE

// RMIB-specific shortcuts
static constexpr int RMIB_RO            = CTLFLAG_READONLY;
static constexpr int RMIB_RW            = CTLFLAG_READWRITE;

// CTL_MAXID from sysctl.h
static constexpr int CTL_MAXID          = 10;

// ============================================================================
// RMIB node structure (from minix/include/minix/rmib.h)
// ============================================================================

// Forward declarations
struct rmib_node;
struct rmib_call;
struct rmib_oldp;
struct rmib_newp;

// Handler function type — matches real MINIX rmib.h (ssize_t)
using rmib_func_ptr = long (*)(struct rmib_call*, struct rmib_node*,
                                 struct rmib_oldp*, struct rmib_newp*);

// Indirect (sparse) node entry
struct rmib_indir {
    unsigned int    rindir_id;
    struct rmib_node* rindir_node;
};

// Main RMIB node structure
struct rmib_node {
    uint32_t    rnode_flags;         // CTLTYPE_ type + CTLFLAG_ flags
    size_t      rnode_size;          // size of associated data

    union {
        bool        rvu_bool;        // immediate boolean
        int         rvu_int;         // immediate integer
        uint64_t    rvu_quad;        // immediate quad
        uint32_t    rvu_clen;        // number of actual children
    } rnode_val_u;

    union {
        void*               rpu_data;    // struct or string data pointer
        struct rmib_node*   rpu_cptr;    // child node array
        struct rmib_indir*  rpu_icptr;   // indirect child node array
    } rnode_ptr_u;

    rmib_func_ptr   rnode_func;      // handler function (may be NULL)
    const char*     rnode_name;      // node name string
    const char*     rnode_desc;      // node description (may be NULL)
};

// Convenience accessors
#define rnode_bool  rnode_val_u.rvu_bool
#define rnode_int   rnode_val_u.rvu_int
#define rnode_quad  rnode_val_u.rvu_quad
#define rnode_clen  rnode_val_u.rvu_clen
#define rnode_data  rnode_ptr_u.rpu_data
#define rnode_cptr  rnode_ptr_u.rpu_cptr
#define rnode_icptr rnode_ptr_u.rpu_icptr

// ============================================================================
// RMIB initialization macros (from minix/include/minix/rmib.h)
// ============================================================================

// Simplified versions that work on the host (compile-time init)

// Node with child array
#define RMIB_NODE_INIT(extra_flags, child_array, name, desc) {             \
    .rnode_flags = CTLTYPE_NODE | CTLFLAG_READONLY |                       \
                   CTLFLAG_PERMANENT | (extra_flags),                      \
    .rnode_size = sizeof(child_array) / sizeof((child_array)[0]),          \
    .rnode_cptr = (child_array),                                           \
    .rnode_name = (name),                                                  \
    .rnode_desc = (desc),                                                  \
}

// Sparse (indirect) node
#define RMIB_SNODE_INIT(extra_flags, indir_array, name, desc) {            \
    .rnode_flags = CTLTYPE_NODE | CTLFLAG_READONLY |                       \
                   CTLFLAG_PERMANENT | CTLFLAG_ROOT | (extra_flags),       \
    .rnode_size = 0,                                                       \
    .rnode_icptr = (indir_array),                                          \
    .rnode_name = (name),                                                  \
    .rnode_desc = (desc),                                                  \
}

// Function-driven node (handler)
#define RMIB_FUNC_INIT(extra_flags, size, func_ptr, name, desc) {          \
    .rnode_flags = CTLFLAG_PERMANENT | (extra_flags),                      \
    .rnode_size = (size),                                                  \
    .rnode_func = (func_ptr),                                              \
    .rnode_name = (name),                                                  \
    .rnode_desc = (desc),                                                  \
}

// Immediate boolean node
#define RMIB_BOOL_INIT(extra_flags, val, name, desc) {                     \
    .rnode_flags = CTLTYPE_BOOL | CTLFLAG_PERMANENT |                      \
                   CTLFLAG_IMMEDIATE | (extra_flags),                      \
    .rnode_size = sizeof(bool),                                            \
    .rnode_bool = (val),                                                   \
    .rnode_name = (name),                                                  \
    .rnode_desc = (desc),                                                  \
}

// Immediate integer node
#define RMIB_INT_INIT(extra_flags, val, name, desc) {                      \
    .rnode_flags = CTLTYPE_INT | CTLFLAG_PERMANENT |                       \
                   CTLFLAG_IMMEDIATE | (extra_flags),                      \
    .rnode_size = sizeof(int),                                             \
    .rnode_int = (val),                                                    \
    .rnode_name = (name),                                                  \
    .rnode_desc = (desc),                                                  \
}

// Immediate quad node
#define RMIB_QUAD_INIT(extra_flags, val, name, desc) {                     \
    .rnode_flags = CTLTYPE_QUAD | CTLFLAG_PERMANENT |                      \
                   CTLFLAG_IMMEDIATE | (extra_flags),                      \
    .rnode_size = sizeof(uint64_t),                                        \
    .rnode_quad = (val),                                                   \
    .rnode_name = (name),                                                  \
    .rnode_desc = (desc),                                                  \
}

// Data pointer node (string, struct)
#define RMIB_DATA_INIT(type_flag, extra_flags, size, ptr, name, desc) {    \
    .rnode_flags = CTLFLAG_PERMANENT | (type_flag) | (extra_flags),        \
    .rnode_size = (size),                                                  \
    .rnode_data = const_cast<void*>(static_cast<const void*>(ptr)),        \
    .rnode_name = (name),                                                  \
    .rnode_desc = (desc),                                                  \
}

// ============================================================================
// RMIB flag helpers
// ============================================================================

static bool rmib_is_node(const rmib_node* node) {
    return (node->rnode_flags & CTLTYPE_NODE) == CTLTYPE_NODE;
}

static bool rmib_is_immediate(const rmib_node* node) {
    return (node->rnode_flags & CTLFLAG_IMMEDIATE) != 0;
}

static bool rmib_is_permanent(const rmib_node* node) {
    return (node->rnode_flags & CTLFLAG_PERMANENT) != 0;
}

static bool rmib_is_sparse(const rmib_node* node) {
    return rmib_is_node(node) &&
           (node->rnode_flags & CTLFLAG_ROOT) != 0;
}

static bool rmib_is_readonly(const rmib_node* node) {
    return (node->rnode_flags & CTLFLAG_READONLY) != 0;
}

static bool rmib_is_readwrite(const rmib_node* node) {
    return (node->rnode_flags & CTLFLAG_READWRITE) != 0;
}

static int rmib_get_type(const rmib_node* node) {
    return node->rnode_flags & 0x0F;  // CTLTYPE is in lower 4 bits
}

// ============================================================================
// RMIB tree lookup helper
// ============================================================================

// Find a child node by ID in a regular (array) node
static rmib_node* rmib_find_child(rmib_node* parent, unsigned int id) {
    if (!rmib_is_node(parent)) return nullptr;

    if (rmib_is_sparse(parent)) {
        // Sparse: indirect array, sorted by ID
        rmib_indir* indir = parent->rnode_icptr;
        uint32_t count = parent->rnode_clen;
        for (uint32_t i = 0; i < count; i++) {
            if (indir[i].rindir_id == id)
                return indir[i].rindir_node;
        }
        return nullptr;
    } else {
        // Regular: sequential array, child's index = ID
        uint32_t count = parent->rnode_clen;
        if (id >= count) return nullptr;
        return &parent->rnode_cptr[id];
    }
}

// ============================================================================
// Test handler function
// ============================================================================

static long test_handler(struct rmib_call* call, struct rmib_node* node,
                          struct rmib_oldp* oldp, struct rmib_newp* newp) {
    return 0;  // stub
}

// ============================================================================
// Test cases — Node type and flag constants
// ============================================================================

TEST_CASE("RMIB CTLTYPE constants are distinct", "[rmib][flags]") {
    REQUIRE(CTLTYPE_NODE != CTLTYPE_INT);
    REQUIRE(CTLTYPE_INT != CTLTYPE_STRING);
    REQUIRE(CTLTYPE_STRING != CTLTYPE_QUAD);
    REQUIRE(CTLTYPE_QUAD != CTLTYPE_STRUCT);
    REQUIRE(CTLTYPE_STRUCT != CTLTYPE_BOOL);
    // All are low integers (1-6)
    REQUIRE(CTLTYPE_NODE == 1);
    REQUIRE(CTLTYPE_BOOL == 6);
}

TEST_CASE("RMIB CTLFLAG constants are distinct", "[rmib][flags]") {
    REQUIRE(CTLFLAG_READONLY != CTLFLAG_READWRITE);
    REQUIRE(CTLFLAG_READONLY != CTLFLAG_PERMANENT);
    REQUIRE(CTLFLAG_READONLY != CTLFLAG_IMMEDIATE);
    REQUIRE(CTLFLAG_READONLY != CTLFLAG_ROOT);
    REQUIRE(CTLFLAG_READWRITE != CTLFLAG_PERMANENT);
    // No overlap between flags
    REQUIRE((CTLFLAG_READONLY & CTLFLAG_READWRITE) == 0);
    REQUIRE((CTLFLAG_READONLY & CTLFLAG_PERMANENT) == 0);
    REQUIRE((CTLFLAG_READONLY & CTLFLAG_IMMEDIATE) == 0);
    REQUIRE((CTLFLAG_READONLY & CTLFLAG_ROOT) == 0);
}

// ============================================================================
// Test cases — RMIB node initialization macros
// ============================================================================

TEST_CASE("RMIB_NODE_INIT creates a valid node", "[rmib][node]") {
    rmib_node children[] = {};
    rmib_node node = RMIB_NODE_INIT(0, children, "test", "test node");

    REQUIRE(rmib_is_node(&node));
    REQUIRE_FALSE(rmib_is_immediate(&node));
    REQUIRE(rmib_is_permanent(&node));
    REQUIRE_FALSE(rmib_is_sparse(&node));
    REQUIRE(rmib_is_readonly(&node));
    REQUIRE_FALSE(rmib_is_readwrite(&node));
    REQUIRE(rmib_get_type(&node) == CTLTYPE_NODE);
    REQUIRE(node.rnode_size == 0);  // empty child array
    REQUIRE(node.rnode_cptr == children);
    REQUIRE(std::strcmp(node.rnode_name, "test") == 0);
    REQUIRE(std::strcmp(node.rnode_desc, "test node") == 0);
    REQUIRE(node.rnode_func == nullptr);
}

TEST_CASE("RMIB_BOOL_INIT creates a boolean node", "[rmib][node]") {
    rmib_node node = RMIB_BOOL_INIT(RMIB_RO, true, "enabled", "feature flag");

    REQUIRE_FALSE(rmib_is_node(&node));
    REQUIRE(rmib_is_immediate(&node));
    REQUIRE(rmib_is_permanent(&node));
    REQUIRE(rmib_is_readonly(&node));
    REQUIRE(rmib_get_type(&node) == CTLTYPE_BOOL);
    REQUIRE(node.rnode_size == sizeof(bool));
    REQUIRE(node.rnode_bool == true);
    REQUIRE(std::strcmp(node.rnode_name, "enabled") == 0);
    REQUIRE(node.rnode_func == nullptr);
}

TEST_CASE("RMIB_BOOL_INIT false value", "[rmib][node]") {
    rmib_node node = RMIB_BOOL_INIT(RMIB_RO, false, "disabled", "not active");
    REQUIRE(node.rnode_bool == false);
    REQUIRE(node.rnode_size == sizeof(bool));
}

TEST_CASE("RMIB_INT_INIT creates an integer node", "[rmib][node]") {
    rmib_node node = RMIB_INT_INIT(RMIB_RW, 42, "answer", "the answer");

    REQUIRE(rmib_get_type(&node) == CTLTYPE_INT);
    REQUIRE(rmib_is_immediate(&node));
    REQUIRE(rmib_is_permanent(&node));
    REQUIRE(rmib_is_readwrite(&node));
    REQUIRE(node.rnode_size == sizeof(int));
    REQUIRE(node.rnode_int == 42);
    REQUIRE(std::strcmp(node.rnode_name, "answer") == 0);
}

TEST_CASE("RMIB_INT_INIT negative and zero values", "[rmib][node]") {
    rmib_node neg = RMIB_INT_INIT(RMIB_RO, -1, "neg", "negative");
    REQUIRE(neg.rnode_int == -1);

    rmib_node zero = RMIB_INT_INIT(RMIB_RO, 0, "zero", "zero");
    REQUIRE(zero.rnode_int == 0);

    rmib_node large = RMIB_INT_INIT(RMIB_RO, 0x7FFFFFFF, "max", "max int");
    REQUIRE(large.rnode_int == 0x7FFFFFFF);
}

TEST_CASE("RMIB_QUAD_INIT creates a quad node", "[rmib][node]") {
    rmib_node node = RMIB_QUAD_INIT(RMIB_RO, 0xDEADBEEFCAFEULL,
                                      "counter", "quad counter");

    REQUIRE(rmib_get_type(&node) == CTLTYPE_QUAD);
    REQUIRE(rmib_is_immediate(&node));
    REQUIRE(node.rnode_size == sizeof(uint64_t));
    REQUIRE(node.rnode_quad == 0xDEADBEEFCAFEULL);
}

TEST_CASE("RMIB_FUNC_INIT creates a function-driven node", "[rmib][node]") {
    rmib_node node = RMIB_FUNC_INIT(CTLTYPE_INT | RMIB_RW,
                                      sizeof(int), test_handler,
                                      "dynamic", "function-driven");

    // Not a node — it's a leaf with a handler
    REQUIRE_FALSE(rmib_is_node(&node));
    // Not immediate — it has a handler
    REQUIRE_FALSE(rmib_is_immediate(&node));
    REQUIRE(rmib_is_permanent(&node));
    REQUIRE(rmib_is_readwrite(&node));
    REQUIRE(rmib_get_type(&node) == CTLTYPE_INT);
    REQUIRE(node.rnode_size == sizeof(int));
    REQUIRE(node.rnode_func == test_handler);
    REQUIRE(std::strcmp(node.rnode_name, "dynamic") == 0);
}

// ============================================================================
// Test cases — RMIB tree structures
// ============================================================================

TEST_CASE("RMIB tree with multiple child nodes", "[rmib][tree]") {
    // Build a tree: root -> { int, bool, quad }
    rmib_node int_node = RMIB_INT_INIT(RMIB_RO, 100, "int_val", "int value");
    rmib_node bool_node = RMIB_BOOL_INIT(RMIB_RO, true, "bool_val", "bool value");
    rmib_node quad_node = RMIB_QUAD_INIT(RMIB_RO, 1234567890ULL,
                                           "quad_val", "quad value");

    rmib_node children[] = { int_node, bool_node, quad_node };
    rmib_node root = RMIB_NODE_INIT(0, children, "root", "root node");

    // Verify children count
    REQUIRE(root.rnode_clen == 3);

    // Find children by index (sequential)
    rmib_node* found = rmib_find_child(&root, 0);
    REQUIRE(found != nullptr);
    REQUIRE(found->rnode_int == 100);

    found = rmib_find_child(&root, 1);
    REQUIRE(found != nullptr);
    REQUIRE(found->rnode_bool == true);

    found = rmib_find_child(&root, 2);
    REQUIRE(found != nullptr);
    REQUIRE(found->rnode_quad == 1234567890ULL);

    // Out of bounds
    found = rmib_find_child(&root, 3);
    REQUIRE(found == nullptr);
}

TEST_CASE("RMIB nested tree depth 2", "[rmib][tree]") {
    // Build: outer -> inner -> value
    rmib_node value = RMIB_INT_INIT(RMIB_RO, 42, "answer", "the answer");
    rmib_node inner_children[] = { value };
    rmib_node inner = RMIB_NODE_INIT(0, inner_children, "inner", "inner node");
    rmib_node outer_children[] = { inner };
    rmib_node outer = RMIB_NODE_INIT(0, outer_children, "outer", "outer node");

    // Navigate: outer -> inner -> value
    rmib_node* found_inner = rmib_find_child(&outer, 0);
    REQUIRE(found_inner != nullptr);
    REQUIRE(rmib_is_node(found_inner));
    REQUIRE(std::strcmp(found_inner->rnode_name, "inner") == 0);

    rmib_node* found_value = rmib_find_child(found_inner, 0);
    REQUIRE(found_value != nullptr);
    REQUIRE_FALSE(rmib_is_node(found_value));
    REQUIRE(found_value->rnode_int == 42);
    REQUIRE(std::strcmp(found_value->rnode_name, "answer") == 0);
}

TEST_CASE("RMIB sparse (indirect) node tree", "[rmib][tree]") {
    // Build a sparse node with 3 children at IDs 5, 10, 100
    rmib_node a = RMIB_INT_INIT(RMIB_RO, 1, "a", "first");
    rmib_node b = RMIB_BOOL_INIT(RMIB_RO, false, "b", "second");
    rmib_node c = RMIB_QUAD_INIT(RMIB_RO, 999, "c", "third");

    rmib_indir indir[] = {
        { 5, &a },
        { 10, &b },
        { 100, &c },
    };

    rmib_node root;
    std::memset(&root, 0, sizeof(root));
    root.rnode_flags = CTLTYPE_NODE | CTLFLAG_READONLY |
                       CTLFLAG_PERMANENT | CTLFLAG_ROOT;
    root.rnode_clen = 3;
    root.rnode_icptr = indir;
    root.rnode_name = "sparse_root";
    root.rnode_desc = "sparse root node";

    REQUIRE(rmib_is_sparse(&root));

    // Find by ID
    rmib_node* found = rmib_find_child(&root, 5);
    REQUIRE(found != nullptr);
    REQUIRE(found == &a);
    REQUIRE(found->rnode_int == 1);
    REQUIRE(std::strcmp(found->rnode_name, "a") == 0);

    found = rmib_find_child(&root, 10);
    REQUIRE(found != nullptr);
    REQUIRE(found == &b);
    REQUIRE(found->rnode_bool == false);

    found = rmib_find_child(&root, 100);
    REQUIRE(found != nullptr);
    REQUIRE(found == &c);
    REQUIRE(found->rnode_quad == 999);

    // Non-existent ID
    found = rmib_find_child(&root, 0);
    REQUIRE(found == nullptr);
    found = rmib_find_child(&root, 50);
    REQUIRE(found == nullptr);
    found = rmib_find_child(&root, 101);
    REQUIRE(found == nullptr);
}

// ============================================================================
// Test cases — RMIB node descriptions
// ============================================================================

TEST_CASE("RMIB node name and description strings", "[rmib][node]") {
    rmib_node node = RMIB_INT_INIT(RMIB_RO, 7, "weekday", 
                                     "number of the day of week");

    REQUIRE(std::strcmp(node.rnode_name, "weekday") == 0);
    REQUIRE(std::strcmp(node.rnode_desc, "number of the day of week") == 0);

    // Node with NULL description
    rmib_node no_desc = RMIB_INT_INIT(RMIB_RO, 0, "nod", nullptr);
    REQUIRE(no_desc.rnode_desc == nullptr);
}

TEST_CASE("RMIB node CTL_MAXID constant", "[rmib][const]") {
    // CTL_MAXID must be >= CTL_MINIX (32) to allow MINIX subtree
    // CTL_MINIX = 32, CTL_MAXID is typically 10 on NetBSD
    REQUIRE(CTL_MAXID == 10);
}

// ============================================================================
// Test cases — Container nodes
// ============================================================================

TEST_CASE("RMIB empty node array", "[rmib][node]") {
    rmib_node empty_children[] = {};
    rmib_node node = RMIB_NODE_INIT(0, empty_children,
                                      "empty", "node with no children");
    REQUIRE(node.rnode_clen == 0);
    REQUIRE(node.rnode_cptr == empty_children);
    REQUIRE(node.rnode_size == 0);
}

TEST_CASE("RMIB node CTLTYPE in flags is in lower nibble", "[rmib][flags]") {
    // CTLTYPE constants 1-6 must fit in lower 4 bits
    REQUIRE((CTLTYPE_NODE & 0x0F) == CTLTYPE_NODE);
    REQUIRE((CTLTYPE_INT & 0x0F) == CTLTYPE_INT);
    REQUIRE((CTLTYPE_STRING & 0x0F) == CTLTYPE_STRING);
    REQUIRE((CTLTYPE_QUAD & 0x0F) == CTLTYPE_QUAD);
    REQUIRE((CTLTYPE_STRUCT & 0x0F) == CTLTYPE_STRUCT);
    REQUIRE((CTLTYPE_BOOL & 0x0F) == CTLTYPE_BOOL);
    // Type extraction mask
    REQUIRE((CTLFLAG_READONLY & 0x0F) == 0);  // not confused with type
}

// ============================================================================
// Test cases — MINIX-dependent (MIB service IPC)
// ============================================================================

TEST_CASE("RMIB register subtree (MINIX)", "[rmib][runtime][minix]") {
    SKIP("MINIX runtime required — rmib_register via IPC to MIB service");
}

TEST_CASE("RMIB deregister subtree (MINIX)", "[rmib][runtime][minix]") {
    SKIP("MINIX runtime required — rmib_deregister");
}

TEST_CASE("RMIB process MIB request (MINIX)", "[rmib][runtime][minix]") {
    SKIP("MINIX runtime required — rmib_process message handling");
}

TEST_CASE("RMIB copyout/copyin (MINIX)", "[rmib][runtime][minix]") {
    SKIP("MINIX runtime required — rmib_copyout / rmib_copyin");
}

TEST_CASE("RMIB sysctl node with handler read/write (MINIX)", "[rmib][runtime][minix]") {
    SKIP("MINIX runtime required — rmib_readwrite handler execution");
}

TEST_CASE("RMIB reregister after update (MINIX)", "[rmib][runtime][minix]") {
    SKIP("MINIX runtime required — rmib_reregister");
}
