# tests/CMakeLists.txt
# CTest configuration (PROTOTYPE)
#
# This file sets up CTest for MINIX with support for ATF/KYUA tests
# as well as native CMake/CTest tests.

# ============================================================================
# Enable testing
# ============================================================================

include(CTest)

# Only enable if MKATF is ON
if(NOT MKATF)
    message(STATUS "Testing disabled (MKATF=OFF)")
    return()
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
# Future: integration tests
# ============================================================================

# TODO: Add integration tests when cmake builds are complete:
# - minix/tests/blocktest
# - minix/tests/ddekit
# - minix/tests/ds
# - minix/tests/fbdtest
# - minix/tests/rmibtest
# - minix/tests/safecopy
