# minix/drivers/storage/CMakeLists.txt
# Storage drivers
#
# Replicates minix/drivers/storage/Makefile

# Always-required drivers
if(MACHINE_ARCH STREQUAL "i386")
    add_subdirectory(at_wini)
    add_subdirectory(floppy)
endif()

# Optional drivers
if(NOT MKIMAGEONLY)
    if(MACHINE_ARCH STREQUAL "i386")
        add_subdirectory(ahci)
        add_subdirectory(fbd)
        add_subdirectory(filter)
        add_subdirectory(virtio_blk)
    elseif(MACHINE_ARCH STREQUAL "earm")
        add_subdirectory(mmc)
    endif()
    add_subdirectory(vnd)
endif()

# Memory driver (must be last, depends on ramdisk image)
add_subdirectory(memory)
