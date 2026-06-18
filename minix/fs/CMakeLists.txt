# minix/fs/CMakeLists.txt
# Filesystem server build
#
# Replicates minix/fs/Makefile and minix/fs/Makefile.inc

# Core filesystems (always built)
add_subdirectory(mfs)
add_subdirectory(pfs)

# Optional filesystems (when MKIMAGEONLY is OFF)
if(NOT MKIMAGEONLY)
    add_subdirectory(ext2)
    add_subdirectory(isofs)
    add_subdirectory(procfs)
    add_subdirectory(ptyfs)

    # Architecture-specific filesystems (none yet for earm)
endif()
