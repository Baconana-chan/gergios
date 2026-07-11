# tests/CMakeLists.txt
# CTest configuration (PROTOTYPE)
#
# This file sets up CTest for MINIX with support for ATF/KYUA tests
# as well as native CMake/CTest tests.

# ============================================================================
# Enable testing
# ============================================================================

include(CTest)

# ============================================================================
# Phase 9.4: Catch2 standalone tests (host-compilable, no MINIX deps)
# These can run WITHOUT MKATF because they use Catch2, not ATF/Kyua.
# ============================================================================

# Enable Catch2 standalone tests even without MKATF
option(MK_CATCH2 "Build Catch2 standalone tests (Phase 9.4+)" ON)

if(MK_CATCH2)
    message(STATUS "Catch2 standalone tests enabled (MK_CATCH2=ON)")
endif()

# Only ATF/Kyua tests require MKATF
if(NOT MKATF)
    message(STATUS "ATF/Kyua tests disabled (MKATF=OFF); Catch2 standalone tests are still enabled if MK_CATCH2=ON")
endif()

# ============================================================================
# Test properties
# ============================================================================

# Default timeout for MINIX tests (some may run on slow emulated hardware)
set(MINIX_TEST_TIMEOUT 300 CACHE STRING "Default test timeout in seconds")

# ============================================================================
# ATF/KYUA test integration
# ============================================================================

# ATF tests are shell-based and run via atf-run or kyua
# This module adds ATF test scripts as CTest tests

# Helper: discover ATF test programs recursively
function(add_minix_atf_test TEST_NAME TEST_SCRIPT)
    add_test(
        NAME ${TEST_NAME}
        COMMAND ${CMAKE_COMMAND} -E env
            ATF_SHELL=${CMAKE_COMMAND}
            ${CMAKE_CURRENT_SOURCE_DIR}/${TEST_SCRIPT}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    )
    set_tests_properties(${TEST_NAME} PROPERTIES
        TIMEOUT ${MINIX_TEST_TIMEOUT}
        ENVIRONMENT "TESTDIR=${CMAKE_CURRENT_SOURCE_DIR}"
    )
endfunction()

# ============================================================================
# Kernel unit tests
# ============================================================================

if(TARGET kernel)
    # Basic kernel compilation test
    add_test(
        NAME kernel-compiles
        COMMAND cmake --build . --target kernel
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    )
    set_tests_properties(kernel-compiles PROPERTIES
        FIXTURES_SETUP KERNEL_BUILD
    )

    # Kernel size check
    add_test(
        NAME kernel-size
        COMMAND test -f "${CMAKE_BINARY_DIR}/minix/kernel/kernel"
    )
    set_tests_properties(kernel-size PROPERTIES
        FIXTURES_REQUIRED KERNEL_BUILD
        TIMEOUT 10
    )
endif()

# ============================================================================
# wolfSSL library tests
# ============================================================================

if(TARGET wolfssl)
    # Library compilation test
    add_test(
        NAME wolfssl-compiles
        COMMAND cmake --build . --target wolfssl
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    )
    set_tests_properties(wolfssl-compiles PROPERTIES
        FIXTURES_SETUP WOLFSSL_BUILD
    )

    # wolfSSL API header accessibility
    add_test(
        NAME wolfssl-headers
        COMMAND test -f "${MINIX_SOURCE_DIR}/crypto/external/gpl2/wolfssl/include/wolfssl/options.h"
    )
    set_tests_properties(wolfssl-headers PROPERTIES
        FIXTURES_REQUIRED WOLFSSL_BUILD
        TIMEOUT 10
    )
endif()

# ============================================================================
# Component build tests (check all migrated components compile)
# ============================================================================

# These are smoke tests that verify the CMake configuration is valid
# without actually performing a full build.

add_test(
    NAME cmake-config-check
    COMMAND ${CMAKE_COMMAND}
        -S "${CMAKE_SOURCE_DIR}"
        -B "${CMAKE_BINARY_DIR}/config-check"
        --graphviz="${CMAKE_BINARY_DIR}/config-check/graph.dot"
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)

# ============================================================================
# Phase 9.1: ext4-core C FFI integration tests (Catch2)
# ============================================================================

if(TARGET rust_ext4-core)
    add_subdirectory(ext4_ffi)
endif()

# Phase 9.1: Bluetooth IPC message encoding tests (standalone, no MINIX deps)
add_subdirectory(bt_ffi)

# Phase 9.1: Driver FFI tests (AHCI register layout via rust_minix-ahci)
if(TARGET rust_minix-ahci)
    add_subdirectory(driver_ffi)
endif()

# ============================================================================
# Phase 9.4: C Test Migration — Catch2 standalone tests
# ============================================================================

if(MK_CATCH2)
    # TCP socket wire-format tests (migrated from ATF test91)
    add_subdirectory(tcp)

    # RAW socket ICMP/IPv6 wire-format tests (migrated from ATF test92)
    add_subdirectory(raw)

    # IPv6 address/DAD semantics tests (migrated from ATF test93)
    add_subdirectory(ipv6)

    # BPF packet filter VM/program tests (migrated from ATF test94)
    add_subdirectory(bpf)

    # Block device I/O semantics tests (migrated from ATF blocktest)
    add_subdirectory(blocktest)

    # Phase 9.4b: Data Store key-value semantics tests
    add_subdirectory(ds)

    # Phase 9.4b: RMIB sysctl node tree tests
    add_subdirectory(rmibtest)

    # Phase 9.4b: Safecopy grant-based IPC tests
    add_subdirectory(safecopy)
endif()

# ============================================================================
# Convenience target: run all Phase 9.4 standalone tests
# ============================================================================

if(MK_CATCH2)
    add_custom_target(check-phase9
        COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -L phase9
        WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
    )
endif()
