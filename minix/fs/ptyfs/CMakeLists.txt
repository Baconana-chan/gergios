# minix/fs/ptyfs/CMakeLists.txt
# PTY filesystem server

add_minix_service(ptyfs
    SOURCES
        ptyfs.c
        node.c
    LIBS fsdriver sys
)
