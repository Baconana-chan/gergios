# minix/fs/mfs/CMakeLists.txt
# Minix File System (MFS) server

add_minix_service(mfs
    SOURCES
        cache.c
        link.c
        mount.c
        misc.c
        open.c
        protect.c
        read.c
        stadir.c
        stats.c
        table.c
        time.c
        utility.c
        write.c
        inode.c
        main.c
        path.c
        super.c
    LIBS minixfs fsdriver bdev sys
)

target_compile_definitions(mfs PRIVATE DEFAULT_NR_BUFS=1024)
