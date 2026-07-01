# minix/drivers/storage/CMakeLists.txt
# Storage drivers
#
# Replicates minix/drivers/storage/Makefile

# i386-specific storage drivers have been removed (Phase 4 i386 deprecation).
# Only drivers that work across architectures are kept.

# Optional drivers
if(NOT MKIMAGEONLY)
    if(MACHINE_ARCH STREQUAL "earm")
        add_subdirectory(mmc)
    endif()
    add_subdirectory_if_exists(vnd)
endif()

# AHCI driver (Phase 5 Rust pilot) — uses C shim + minix-ahci staticlib
add_subdirectory_if_exists(ahci)

# Memory driver (must be last, depends on ramdisk image)
add_subdirectory_if_exists(memory)
