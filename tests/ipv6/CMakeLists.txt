# tests/ipv6/CMakeLists.txt
# IPv6 address and routing tests using Catch2 (migrated from ATF test93)
#
# Phase 9.4: C Test Migration (ATF → Catch2)
# See planning/28_testing_framework_migration.md
#
# These tests validate IPv6 address manipulation and DAD (Duplicate
# Address Detection) address semantics on the host.

set(CATCH2_INCLUDE_DIR "${PROJECT_SOURCE_DIR}/external/mit/catch2")

set(IPV6_TESTS
    ipv6_addr
)

foreach(TEST_NAME ${IPV6_TESTS})
    add_executable(${TEST_NAME}
        ${CMAKE_CURRENT_SOURCE_DIR}/${TEST_NAME}.cpp
    )
    target_include_directories(${TEST_NAME} PRIVATE ${CATCH2_INCLUDE_DIR})
    target_include_directories(${TEST_NAME} PRIVATE ${PROJECT_SOURCE_DIR}/include)

    add_test(
        NAME ipv6_${TEST_NAME}
        COMMAND ${TEST_NAME}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    )
    set_tests_properties(ipv6_${TEST_NAME} PROPERTIES
        TIMEOUT 10
        LABELS "ipv6;network;phase9;standalone"
    )
endforeach()

add_custom_target(check-ipv6
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -L ipv6
    DEPENDS ${IPV6_TESTS}
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)
