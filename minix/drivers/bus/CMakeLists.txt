# minix/drivers/bus/CMakeLists.txt
# Bus drivers
#
# Replicates minix/drivers/bus/Makefile

# PCI (required on i386)
if(MACHINE_ARCH STREQUAL "i386")
    add_subdirectory(pci)
endif()

# Architecture-specific optional drivers
if(NOT MKIMAGEONLY)
    if(MACHINE_ARCH STREQUAL "i386")
        add_subdirectory(ti1225)
    elseif(MACHINE_ARCH STREQUAL "earm")
        add_subdirectory(i2c)
    endif()
endif()
