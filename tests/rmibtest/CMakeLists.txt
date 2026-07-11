# tests/rmibtest/CMakeLists.txt
# RMIB sysctl node tree tests using Catch2
#
# Phase 9.4b: C Test Migration — component tests
# See planning/28_testing_framework_migration.md
#
# These tests validate RMIB node initialization macros, indirect node
# arrays, and sysctl name resolution on the host.
# MINIX-dependent tests (actual MIB service IPC) will skip.

set(CATCH2_INCLUDE_DIR "${PROJECT_SOURCE_DIR}/external/mit/catch2")

add_executable(rmibtest
    ${CMAKE_CURRENT_SOURCE_DIR}/rmib_test.cpp
)
target_include_directories(rmibtest PRIVATE ${CATCH2_INCLUDE_DIR})
target_include_directories(rmibtest PRIVATE ${PROJECT_SOURCE_DIR}/include)

add_test(
    NAME rmibtest
    COMMAND rmibtest
    WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
)
set_tests_properties(rmibtest PROPERTIES
    TIMEOUT 10
    LABELS "rmib;sysctl;phase9;standalone"
)

add_custom_target(check-rmib
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -L rmib
    DEPENDS rmibtest
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)
