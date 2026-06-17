# minix/fs/hgfs/CMakeLists.txt
# Host-Guest filesystem server (i386 only)
#
# TODO: Migrate from minix/fs/hgfs/Makefile

add_minix_service(hgfs
    SOURCES
        # Add sources from minix/fs/hgfs/Makefile here
    LIBS fsdriver bdev sys
)
