# tests/ext4_ffi/CMakeLists.txt
# ext4-core C FFI integration tests using Catch2
#
# Phase 9.1: CTest Integration & C FFI Tests
# See planning/28_testing_framework_migration.md

# ── Dependencies ────────────────────────────────────────────────────
# Requires: Catch2 (vendored at ${PROJECT_SOURCE_DIR}/external/mit/catch2/)
# Requires: rust_ext4-core (CMake imported target)

# ── Catch2 header (vendored single-header) ──────────────────────────
set(CATCH2_INCLUDE_DIR "${PROJECT_SOURCE_DIR}/external/mit/catch2")

# ── ext4-core Rust staticlib ────────────────────────────────────────
# The target rust_ext4-core is created by add_rust_library(ext4-core)
# in the root CMakeLists.txt. We link against it here.
if(NOT TARGET rust_ext4-core)
    message(WARNING "Target rust_ext4-core not found — ext4 FFI tests will be skipped")
    return()
endif()

# ── Test source files ───────────────────────────────────────────────
set(EXT4_FFI_TESTS
    test_superblock
    test_inode
    test_dir
)

# ── Build each test ─────────────────────────────────────────────────
foreach(TEST_NAME ${EXT4_FFI_TESTS})
    add_executable(${TEST_NAME}
        ${CMAKE_CURRENT_SOURCE_DIR}/${TEST_NAME}.cpp
    )

    target_include_directories(${TEST_NAME} PRIVATE
        ${CATCH2_INCLUDE_DIR}
    )

    target_link_libraries(${TEST_NAME} PRIVATE
        rust_ext4-core
    )

    # Register with CTest
    add_test(
        NAME ext4_ffi_${TEST_NAME}
        COMMAND ${TEST_NAME}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    )

    set_tests_properties(ext4_ffi_${TEST_NAME} PROPERTIES
        TIMEOUT 30
        LABELS "ext4;ffi;phase9"
    )
endforeach()

# ── Convenience target to run all ext4 FFI tests ───────────────────
add_custom_target(check-ext4-ffi
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -L ext4
    DEPENDS ${EXT4_FFI_TESTS}
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)
