# minix/lib/libvtreefs/CMakeLists.txt
# Virtual tree filesystem library
#
# TODO: Add actual sources from minix/lib/libvtreefs/Makefile

file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/libvtreefs_stub.c"
    "// libvtreefs stub - replace with real sources\n")

add_minix_library(vtreefs
    SOURCES "${CMAKE_CURRENT_BINARY_DIR}/libvtreefs_stub.c"
    INSTALL_DIR /usr/lib
)
