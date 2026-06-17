# minix/fs/ptyfs/CMakeLists.txt
# PTY filesystem server
#
# TODO: Migrate from minix/fs/ptyfs/Makefile

add_minix_service(ptyfs
    SOURCES
        # Add sources from minix/fs/ptyfs/Makefile here
    LIBS fsdriver bdev sys
)
