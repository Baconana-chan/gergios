# tests/ds/CMakeLists.txt
# Data Store (DS) key-value semantics tests using Catch2
#
# Phase 9.4b: C Test Migration — component tests
# See planning/28_testing_framework_migration.md
#
# These tests validate DS flag encoding, key validation, data_store
# struct layout, and subscription matching on the host.
# MINIX-dependent tests (actual DS server IPC) will skip.

set(CATCH2_INCLUDE_DIR "${PROJECT_SOURCE_DIR}/external/mit/catch2")

add_executable(ds_test
    ${CMAKE_CURRENT_SOURCE_DIR}/ds_test.cpp
)
target_include_directories(ds_test PRIVATE ${CATCH2_INCLUDE_DIR})
target_include_directories(ds_test PRIVATE ${PROJECT_SOURCE_DIR}/include)

add_test(
    NAME ds_test
    COMMAND ds_test
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)
set_tests_properties(ds_test PROPERTIES
    TIMEOUT 10
    LABELS "ds;datastore;phase9;standalone"
)

add_custom_target(check-ds
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -L ds
    DEPENDS ds_test
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)
