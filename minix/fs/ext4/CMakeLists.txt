# minix/fs/ext4/CMakeLists.txt
# ext4 filesystem server — Rust core + C FFI bridge
#
# Build methods:
#   1. make ext4          — C-only mode (Rust FFI functions are weak stubs)
#   2. Full Rust build    — Build via releasetools/build_ext4.sh (requires MINIX toolchain)
#
# Prerequisites for full Rust build:
#   - Rust toolchain (rustc, cargo)
#   - MINIX_TOOLCHAIN + MINIX_DESTDIR env vars (for cross-compilation)
#   - See: releasetools/build_ext4.sh

cmake_minimum_required(VERSION 3.20)

# ─── Rust staticlib detection ────────────────────────────
#
# The Rust ext4-core crate produces:
#   - Linux:  libext4_core.a
#   - Windows: ext4_core.lib
#
# Build the staticlib manually before running CMake:
#   # Native (for host testing):
#   cd rust/ext4-core && cargo build --release --lib
#
#   # Cross-compile for MINIX (requires toolchain):
#   ./releasetools/build_ext4.sh cross x86_64
#
# The resulting library is expected at CMAKE_CURRENT_BINARY_DIR/libext4_core.a
# (after build_ext4.sh copies it there).

set(EXT4_RUST_STATICLIB "${CMAKE_CURRENT_BINARY_DIR}/libext4_core.a")
set(HAS_RUST_STATICLIB FALSE)

if(EXISTS "${EXT4_RUST_STATICLIB}")
    set(HAS_RUST_STATICLIB TRUE)
    message(STATUS "ext4-core: Rust staticlib found: ${EXT4_RUST_STATICLIB}")
else()
    message(STATUS "ext4-core: Rust staticlib NOT found (build C-only with weak stubs)")
    message(STATUS "ext4-core: Run './releasetools/build_ext4.sh cross ${MACHINE_ARCH}' to build it")
endif()

# ─── Service definition ──────────────────────────────────
add_minix_service(ext4
    SOURCES
        main.c
        table.c
        ffi_bridge.c
    LIBS minixfs fsdriver bdev sys
)

# ─── Rust staticlib linkage ─────────────────────────────
if(HAS_RUST_STATICLIB)
    target_link_libraries(ext4 PRIVATE "${EXT4_RUST_STATICLIB}")

    # LTO reduces binary size by eliminating dead Rust code
    set_target_properties(ext4 PROPERTIES
        LINK_FLAGS "-Wl,--gc-sections"
    )

    message(STATUS "ext4-core: Full build with Rust staticlib")
else()
    # Weak stub mode: provide empty implementations for Rust FFI functions
    # so the ext4 server compiles and links as C-only fallback.
    # When the Rust staticlib is available, these are overridden.
    set_source_files_properties(ffi_bridge.c PROPERTIES
        COMPILE_DEFINITIONS "EXT4_C_ONLY=1"
    )
    message(STATUS "ext4-core: C-only fallback (Rust FFI = weak stubs)")
endif()

# ─── Install ─────────────────────────────────────────────
install(TARGETS ext4 RUNTIME DESTINATION sbin)
