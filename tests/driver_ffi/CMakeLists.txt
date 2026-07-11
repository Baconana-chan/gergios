# tests/driver_ffi/CMakeLists.txt
# Driver FFI integration tests using Catch2
#
# Phase 9.1: CTest Integration & C FFI Tests
# See planning/28_testing_framework_migration.md

# ── Dependencies ────────────────────────────────────────────────────
# Requires: Catch2 (vendored at ${PROJECT_SOURCE_DIR}/external/mit/catch2/)
# Requires: rust_minix-ahci (CMake imported target from add_rust_library(minix-ahci))

set(CATCH2_INCLUDE_DIR "${PROJECT_SOURCE_DIR}/external/mit/catch2")

if(NOT TARGET rust_minix-ahci AND NOT TARGET rust_e1000)
    message(WARNING "No Rust driver targets found — driver FFI tests will be skipped")
    return()
endif()

# ── Test source files ───────────────────────────────────────────────
set(DRIVER_FFI_TESTS
    test_ahci
    test_e1000
    test_virtio_net
)

# ── Build each test ─────────────────────────────────────────────────
foreach(TEST_NAME ${DRIVER_FFI_TESTS})
    add_executable(${TEST_NAME}
        ${CMAKE_CURRENT_SOURCE_DIR}/${TEST_NAME}.cpp
    )

    target_include_directories(${TEST_NAME} PRIVATE
        ${CATCH2_INCLUDE_DIR}
    )

    # Link the appropriate Rust staticlib per test
    if(${TEST_NAME} STREQUAL "test_ahci" AND TARGET rust_minix-ahci)
        target_link_libraries(${TEST_NAME} PRIVATE rust_minix-ahci)
    elseif(${TEST_NAME} STREQUAL "test_e1000" AND TARGET rust_e1000)
        target_link_libraries(${TEST_NAME} PRIVATE rust_e1000)
    elseif(${TEST_NAME} STREQUAL "test_virtio_net" AND TARGET rust_virtio_net)
        target_link_libraries(${TEST_NAME} PRIVATE rust_virtio_net)
    else()
        message(WARNING "No Rust library target for ${TEST_NAME} — skipping link")
    endif()

    add_test(
        NAME driver_ffi_${TEST_NAME}
        COMMAND ${TEST_NAME}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    )

    set_tests_properties(driver_ffi_${TEST_NAME} PROPERTIES
        TIMEOUT 30
        LABELS "driver;ffi;phase9"
    )
endforeach()

# ── Convenience target ──────────────────────────────────────────────
add_custom_target(check-driver-ffi
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -L driver
    DEPENDS ${DRIVER_FFI_TESTS}
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)
