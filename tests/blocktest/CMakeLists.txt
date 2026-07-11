# tests/blocktest/CMakeLists.txt
# Block device tests using Catch2 (migrated from ATF blocktest)
#
# Phase 9.4: C Test Migration (ATF → Catch2)
# See planning/28_testing_framework_migration.md
#
# These tests validate block device read/write operation
# semantics on the host. MINIX-dependent block device access
# tests will skip.

set(CATCH2_INCLUDE_DIR "${PROJECT_SOURCE_DIR}/external/mit/catch2")

add_executable(blocktest
    ${CMAKE_CURRENT_SOURCE_DIR}/blocktest.cpp
)
target_include_directories(blocktest PRIVATE ${CATCH2_INCLUDE_DIR})
target_include_directories(blocktest PRIVATE ${PROJECT_SOURCE_DIR}/include)

add_test(
    NAME blocktest
    COMMAND blocktest
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)
set_tests_properties(blocktest PROPERTIES
    TIMEOUT 10
    LABELS "block;storage;phase9;standalone"
)

add_custom_target(check-blocktest
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -L block
    DEPENDS blocktest
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)
