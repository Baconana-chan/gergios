# minix/fs/vbfs/CMakeLists.txt
# VirtualBox filesystem server (i386 only)
#
# TODO: Migrate from minix/fs/vbfs/Makefile

add_minix_service(vbfs
    SOURCES
        # Add sources from minix/fs/vbfs/Makefile here
    LIBS fsdriver bdev sys
)
