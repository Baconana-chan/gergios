# minix/lib/libexec/CMakeLists.txt
# Exec library (libexec)
#
# TODO: Add actual sources from minix/lib/libexec/Makefile
# Currently a stub to maintain CMake target dependency chain.

# Always use stub for now (TODO: add real sources)
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/libexec_stub.c"
    "// libexec stub - replace with actual sources\n")
add_minix_library(exec
    SOURCES "${CMAKE_CURRENT_BINARY_DIR}/libexec_stub.c"
    INSTALL_DIR /usr/lib
)
