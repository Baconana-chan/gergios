# minix/lib/libblockdriver/CMakeLists.txt
# Block driver library
#
# TODO: Add actual sources from minix/lib/libblockdriver/Makefile

# Stub library placeholder
file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/libblockdriver_stub.c"
    "// libblockdriver stub - replace with actual sources\n")

add_minix_library(blockdriver
    SOURCES "${CMAKE_CURRENT_BINARY_DIR}/libblockdriver_stub.c"
    INSTALL_DIR /usr/lib
)
