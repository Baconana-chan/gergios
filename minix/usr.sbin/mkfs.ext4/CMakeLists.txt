# minix/usr.sbin/mkfs.ext4/CMakeLists.txt
# mkfs.ext4 — create ext4 filesystem (thin C wrapper around Rust ext4-core)

# ─── Rust staticlib detection ────────────────────────────
# The Rust ext4-core staticlib must be built first:
#   ./releasetools/build_ext4.sh cross x86_64
set(EXT4_RUST_STATICLIB "${CMAKE_CURRENT_SOURCE_DIR}/../../rust/ext4-core/target/release/libext4_core.a")

if(NOT EXISTS "${EXT4_RUST_STATICLIB}")
    # Try debug build
    set(EXT4_RUST_STATICLIB "${CMAKE_CURRENT_SOURCE_DIR}/../../rust/ext4-core/target/debug/libext4_core.a")
endif()

if(EXISTS "${EXT4_RUST_STATICLIB}")
    message(STATUS "mkfs.ext4: Rust staticlib found: ${EXT4_RUST_STATICLIB}")
else()
    # Fallback: look in CMake build directory (where build_ext4.sh may copy it)
    set(EXT4_RUST_STATICLIB "${CMAKE_CURRENT_BINARY_DIR}/libext4_core.a")
    if(NOT EXISTS "${EXT4_RUST_STATICLIB}")
        message(WARNING "mkfs.ext4: Rust staticlib NOT found. Build manually:\n"
            "  cd rust && cargo build --release -p ext4-core\n"
            "  or: ./releasetools/build_ext4.sh native")
    endif()
endif()

# ─── Executable definition ───────────────────────────────
add_executable(mkfs.ext4 main.c)

if(EXISTS "${EXT4_RUST_STATICLIB}")
    target_link_libraries(mkfs.ext4 PRIVATE "${EXT4_RUST_STATICLIB}")
    target_link_libraries(mkfs.ext4 PRIVATE m c)
endif()

# Install to /sbin (where mkfs.mfs lives)
install(TARGETS mkfs.ext4 RUNTIME DESTINATION sbin)
