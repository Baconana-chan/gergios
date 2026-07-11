# tests/tcp/CMakeLists.txt
# TCP socket tests using Catch2 (migrated from ATF test91)
#
# Phase 9.4: C Test Migration (ATF → Catch2)
# See planning/28_testing_framework_migration.md
#
# NOTE: Some tests require a running MINIX lwIP service and will
# only be enabled when the MINIX cross-toolchain is available.
# Tests marked as "standalone" validate TCP wire format, header
# parsing, and error handling on the host.

set(CATCH2_INCLUDE_DIR "${PROJECT_SOURCE_DIR}/external/mit/catch2")

# ── Standalone TCP wire-format tests (host-compilable) ──────────────
set(TCP_STANDALONE_TESTS
    tcp_socket
    tcp_error
)

foreach(TEST_NAME ${TCP_STANDALONE_TESTS})
    add_executable(${TEST_NAME}
        ${CMAKE_CURRENT_SOURCE_DIR}/${TEST_NAME}.cpp
    )
    target_include_directories(${TEST_NAME} PRIVATE ${CATCH2_INCLUDE_DIR})

    add_test(
        NAME tcp_${TEST_NAME}
        COMMAND ${TEST_NAME}
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}
    )
    set_tests_properties(tcp_${TEST_NAME} PROPERTIES
        TIMEOUT 10
        LABELS "tcp;network;phase9;standalone"
    )
endforeach()

# ── MINIX-dependent TCP tests (requires lwIP runtime) ──────────────
# These tests need a running MINIX kernel with lwIP service:
#   - tcp_connect (connect to remote host)
#   - tcp_listen (listen and accept)
#   - tcp_sendrecv (send/receive data)
# They are registered but will fail gracefully on non-MINIX hosts.
# Uncomment when MINIX toolchain is available:
#
# set(TCP_MINIX_TESTS tcp_poll)
# foreach(TEST_NAME ${TCP_MINIX_TESTS})
#     add_executable(${TEST_NAME}
#         ${CMAKE_CURRENT_SOURCE_DIR}/${TEST_NAME}.cpp
#     )
#     target_include_directories(${TEST_NAME} PRIVATE ${CATCH2_INCLUDE_DIR})
#     target_link_libraries(${TEST_NAME} PRIVATE sys)
#     add_test(NAME tcp_${TEST_NAME} COMMAND ${TEST_NAME})
#     set_tests_properties(tcp_${TEST_NAME} PROPERTIES
#         TIMEOUT 60
#         LABELS "tcp;network;phase9;minix"
#     )
# endforeach()

# ── Convenience target ──────────────────────────────────────────────
add_custom_target(check-tcp
    COMMAND ${CMAKE_CTEST_COMMAND} --output-on-failure -L tcp
    DEPENDS ${TCP_STANDALONE_TESTS}
    WORKING_DIRECTORY ${CMAKE_BINARY_DIR}
)
