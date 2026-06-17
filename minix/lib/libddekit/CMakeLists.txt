# minix/lib/libddekit/CMakeLists.txt
# DDE kit library
#
# TODO: Add actual sources from minix/lib/libddekit/Makefile

file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/libddekit_stub.c"
    "// libddekit stub - replace with real sources\n")

add_minix_library(ddekit
    SOURCES "${CMAKE_CURRENT_BINARY_DIR}/libddekit_stub.c"
    INSTALL_DIR /usr/lib
)
