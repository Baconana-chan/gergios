# minix/lib/libchardriver/CMakeLists.txt
# Character driver library
#
# TODO: Add actual sources from minix/lib/libchardriver/Makefile

file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/libchardriver_stub.c"
    "// libchardriver stub - replace with real sources\n")

add_minix_library(chardriver
    SOURCES "${CMAKE_CURRENT_BINARY_DIR}/libchardriver_stub.c"
    INSTALL_DIR /usr/lib
)
