# minix/fs/isofs/CMakeLists.txt
# ISO 9660 filesystem server

add_minix_service(isofs
    SOURCES
        main.c
        table.c
        mount.c
        super.c
        inode.c
        link.c
        utility.c
        path.c
        read.c
        susp.c
        susp_rock_ridge.c
        stadir.c
    LIBS minixfs fsdriver bdev sys
)

target_compile_definitions(isofs PRIVATE NR_BUFS=100)
