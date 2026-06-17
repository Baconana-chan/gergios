# minix/fs/isofs/CMakeLists.txt
# ISO 9660 filesystem server
#
# TODO: Migrate from minix/fs/isofs/Makefile

add_minix_service(isofs
    SOURCES
        # Add sources from minix/fs/isofs/Makefile here
    LIBS fsdriver bdev sys
)
