# minix/lib/libfsdriver/CMakeLists.txt
# Filesystem driver library
#
# TODO: Add actual sources from minix/lib/libfsdriver/Makefile

file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/libfsdriver_stub.c"
    "// libfsdriver stub - replace with real sources\n")

add_minix_library(fsdriver
    SOURCES "${CMAKE_CURRENT_BINARY_DIR}/libfsdriver_stub.c"
    INSTALL_DIR /usr/lib
)
