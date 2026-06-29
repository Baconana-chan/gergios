# minix/fs/ext4/CMakeLists.txt
# ext4 filesystem server — Rust core + C FFI bridge
# Phase 1: read-only foundation

# TODO: add_rust_staticlib once the Rust build integration is finalized.
# For now, the ext4 server is built in C-only mode with stubs.

add_minix_service(ext4
    SOURCES
        main.c
        table.c
        ffi_bridge.c
    LIBS minixfs fsdriver bdev sys
)

# When Rust integration is ready:
# add_rust_staticlib(ext4-core
#     CRATE ${CMAKE_SOURCE_DIR}/rust/ext4-core
#     OUTPUT ext4_core
# )
# target_link_libraries(ext4 PRIVATE ${CMAKE_CURRENT_BINARY_DIR}/libext4_core.a)
