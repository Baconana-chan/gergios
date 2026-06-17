# minix/servers/input/CMakeLists.txt
# Input server
#
# Replicates minix/servers/input/Makefile

add_minix_service(input
    SOURCES
        input.c
    LIBS chardriver sys
)
