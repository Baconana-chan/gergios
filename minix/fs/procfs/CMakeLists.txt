# minix/fs/procfs/CMakeLists.txt
# Process filesystem server
#
# TODO: Migrate from minix/fs/procfs/Makefile

add_minix_service(procfs
    SOURCES
        # Add sources from minix/fs/procfs/Makefile here
    LIBS fsdriver bdev sys
)
