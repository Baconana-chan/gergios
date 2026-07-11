# tests/bt_ffi/CMakeLists.txt
# Bluetooth IPC message encoding tests using Catch2
#
# Phase 9.1: CTest Integration & C FFI Tests
# See planning/28_testing_framework_migration.md
#
# These are standalone tests that verify the MINIX IPC message encoding
# format (byte offsets, LE byte order, BD_ADDR packing, name encoding)
# without requiring a running MINIX kernel or bluetoothd.

# ── Catch2 header (vendored single-header) ──────────────────────────
set(CATCH2_INCLUDE_DIR "${PROJECT_SOURCE_DIR}/external/mit/catch2")

# ── Test source files ───────────────────────────────────────────────
set(BT_FFI_TESTS
    test_bt_ipc
    test_sdp
)

# ── Build each test ─────────────────────────────────────────────────
foreach(TEST_NAME ${BT_FFI_TESTS})
    add_executable(${TEST_NAME}
        ${CMAKE_CURRENT_SOURCE_DIR}/${TEST_NAME}.cpp
    )

    target_include_directories(${TEST_NAME} PRIVATE
        ${CATCH2_INCLUDE_DIR}
    )

    # No external dependencies — standalone byte-level tests

    # Register with CTest
    add_test(
        NAME bt_ffi_${TEST_NAME}
        COMMAND ${TEST_NAME}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    )

    set_tests_properties(bt_ffi_${TEST_NAME} PROPERTIES
        TIMEOUT 10
        LABELS "bluetooth;ffi;phase9"
    )
endforeach()

# ── Convenience target ──────────────────────────────────────────────
add_custom_target(check-bt-ffi
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -L bluetooth
    DEPENDS ${BT_FFI_TESTS}
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)
