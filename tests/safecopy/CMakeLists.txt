# tests/safecopy/CMakeLists.txt
# Safecopy grant-based IPC tests using Catch2
#
# Phase 9.4b: C Test Migration — component tests
# See planning/28_testing_framework_migration.md
#
# These tests validate grant ID encoding/decoding, flag combinations,
# and cp_grant_t struct layout on the host.
# MINIX-dependent tests (actual kernel safecopy syscalls) will skip.

set(CATCH2_INCLUDE_DIR "${PROJECT_SOURCE_DIR}/external/mit/catch2")

add_executable(safecopy_test
    ${CMAKE_CURRENT_SOURCE_DIR}/safecopy_test.cpp
)
target_include_directories(safecopy_test PRIVATE ${CATCH2_INCLUDE_DIR})
target_include_directories(safecopy_test PRIVATE ${PROJECT_SOURCE_DIR}/include)

add_test(
    NAME safecopy_test
    COMMAND safecopy_test
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)
set_tests_properties(safecopy_test PROPERTIES
    TIMEOUT 10
    LABELS "safecopy;ipc;phase9;standalone"
)

add_custom_target(check-safecopy
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -L safecopy
    DEPENDS safecopy_test
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)
