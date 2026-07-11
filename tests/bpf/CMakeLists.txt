# tests/bpf/CMakeLists.txt
# BPF packet filter tests using Catch2 (migrated from ATF test94)
#
# Phase 9.4: C Test Migration (ATF → Catch2)
# See planning/28_testing_framework_migration.md
#
# These tests validate BPF program execution, filter match/mismatch,
# and attach/detach semantics on the host. MINIX-dependent tests
# (attach to real network interface) will skip.

set(CATCH2_INCLUDE_DIR "${PROJECT_SOURCE_DIR}/external/mit/catch2")

set(BPF_TESTS
    bpf_filter
    bpf_attach
)

foreach(TEST_NAME ${BPF_TESTS})
    add_executable(${TEST_NAME}
        ${CMAKE_CURRENT_SOURCE_DIR}/${TEST_NAME}.cpp
    )
    target_include_directories(${TEST_NAME} PRIVATE ${CATCH2_INCLUDE_DIR})
    target_include_directories(${TEST_NAME} PRIVATE ${PROJECT_SOURCE_DIR}/include)

    add_test(
        NAME bpf_${TEST_NAME}
        COMMAND ${TEST_NAME}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    )
    set_tests_properties(bpf_${TEST_NAME} PROPERTIES
        TIMEOUT 10
        LABELS "bpf;network;phase9;standalone"
    )
endforeach()

add_custom_target(check-bpf
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -L bpf
    DEPENDS ${BPF_TESTS}
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)
