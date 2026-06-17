# minix/fs/ext2/CMakeLists.txt
# ext2 filesystem server

add_minix_service(ext2
    SOURCES
        balloc.c
        link.c
        mount.c
        misc.c
        open.c
        protect.c
        read.c
        stadir.c
        table.c
        time.c
        utility.c
        write.c
        ialloc.c
        inode.c
        main.c
        path.c
        super.c
    LIBS minixfs fsdriver bdev sys
)
