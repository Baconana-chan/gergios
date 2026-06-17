# minix/servers/mib/CMakeLists.txt
# Management Information Base (MIB) server
#
# Replicates minix/servers/mib/Makefile

add_minix_service(mib
    SOURCES
        main.c
        tree.c
        remote.c
        kern.c
        vm.c
        hw.c
        proc.c
        minix.c
    LIBS sys
)

target_include_directories(mib PRIVATE ${MINIX_SOURCE_DIR}/minix)
