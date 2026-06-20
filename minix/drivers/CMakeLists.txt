# minix/drivers/CMakeLists.txt
# Driver component build
#
# Replicates minix/drivers/Makefile and minix/drivers/Makefile.inc

# Optional drivers (when MKIMAGEONLY is OFF)
if(NOT MKIMAGEONLY)
    add_subdirectory_if_exists(audio)
    add_subdirectory_if_exists(clock)
    add_subdirectory_if_exists(eeprom)
    add_subdirectory_if_exists(examples)
    add_subdirectory_if_exists(iommu)
    add_subdirectory_if_exists(net)
    add_subdirectory_if_exists(power)
    add_subdirectory_if_exists(printer)
    add_subdirectory_if_exists(sensors)
    add_subdirectory_if_exists(system)
    add_subdirectory_if_exists(usb)
    add_subdirectory_if_exists(video)
    add_subdirectory_if_exists(vmm_guest)
endif()

# Required drivers (always built)
add_subdirectory_if_exists(bus)
add_subdirectory_if_exists(tty)
add_subdirectory_if_exists(hid)

# Storage must be last (ramdisk dependency)
add_subdirectory_if_exists(storage)
