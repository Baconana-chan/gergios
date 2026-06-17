# minix/servers/devman/CMakeLists.txt
# Device Manager server
#
# Replicates minix/servers/devman/Makefile

add_minix_service(devman
    SOURCES
        main.c
        device.c
        buf.c
        bind.c
    LIBS vtreefs fsdriver sys
)
