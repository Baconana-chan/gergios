# minix/drivers/bus/CMakeLists.txt
# Bus drivers
#
# Replicates minix/drivers/bus/Makefile
#
# NOTE: i386-specific bus drivers (pci, ti1225) were removed
# in Phase 4 i386 deprecation.

# Architecture-specific optional drivers
if(NOT MKIMAGEONLY)
    if(MACHINE_ARCH STREQUAL "earm")
        add_subdirectory(i2c)
    endif()
endif()
