# minix/fs/CMakeLists.txt
# Filesystem server build
#
# Replicates minix/fs/Makefile and minix/fs/Makefile.inc

# Core filesystems (always built)
add_subdirectory_if_exists(mfs)
add_subdirectory_if_exists(pfs)

# Optional filesystems (when MKIMAGEONLY is OFF)
if(NOT MKIMAGEONLY)
    add_subdirectory_if_exists(ext2)
    add_subdirectory_if_exists(isofs)
    add_subdirectory_if_exists(procfs)
    add_subdirectory_if_exists(ptyfs)

    # Architecture-specific filesystems (none yet for earm)
endif()
