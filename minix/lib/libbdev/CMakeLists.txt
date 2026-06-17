# minix/lib/libbdev/CMakeLists.txt
# Block device library (libbdev)
#
# Replicates minix/lib/libbdev/Makefile

add_minix_library(bdev
    SOURCES
        bdev.c
        driver.c
        call.c
        ipc.c
        minor.c
    INSTALL_DIR /usr/lib
)

target_compile_definitions(bdev PRIVATE _MINIX_SYSTEM)
