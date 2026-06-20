# minix/lib/liblwip/CMakeLists.txt
# lwIP network library
#
# TODO: Add actual sources from minix/lib/liblwip/Makefile

file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/liblwip_stub.c"
    "// liblwip stub - replace with real sources\n")

# Renamed to lwip_lib to avoid name collision with net/lwip executable
add_minix_library(lwip_lib
    SOURCES "${CMAKE_CURRENT_BINARY_DIR}/liblwip_stub.c"
    INSTALL_DIR /usr/lib
)
