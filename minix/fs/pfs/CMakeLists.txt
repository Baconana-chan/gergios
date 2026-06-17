# minix/fs/pfs/CMakeLists.txt
# Pipe File System (PFS) server

add_minix_service(pfs
    SOURCES pfs.c
    LIBS fsdriver sys
)
