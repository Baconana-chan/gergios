# tests/raw/CMakeLists.txt
# RAW socket tests using Catch2 (migrated from ATF test92)
#
# Phase 9.4: C Test Migration (ATF → Catch2)
# See planning/28_testing_framework_migration.md
#
# These tests validate ICMP and IPv6 raw socket wire format and
# header parsing on the host. MINIX-dependent tests will skip.

set(CATCH2_INCLUDE_DIR "${PROJECT_SOURCE_DIR}/external/mit/catch2")

set(RAW_TESTS
    raw_icmp
    raw_ipv6
)

foreach(TEST_NAME ${RAW_TESTS})
    add_executable(${TEST_NAME}
        ${CMAKE_CURRENT_SOURCE_DIR}/${TEST_NAME}.cpp
    )
    target_include_directories(${TEST_NAME} PRIVATE ${CATCH2_INCLUDE_DIR})
    target_include_directories(${TEST_NAME} PRIVATE ${PROJECT_SOURCE_DIR}/include)

    add_test(
        NAME raw_${TEST_NAME}
        COMMAND ${TEST_NAME}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    )
    set_tests_properties(raw_${TEST_NAME} PROPERTIES
        TIMEOUT 10
        LABELS "raw;network;phase9;standalone"
    )
endforeach()

add_custom_target(check-raw
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -L raw
    DEPENDS ${RAW_TESTS}
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)
