# minix/drivers/CMakeLists.txt
# Driver component build
#
# Replicates minix/drivers/Makefile and minix/drivers/Makefile.inc

# Optional drivers (when MKIMAGEONLY is OFF)
if(NOT MKIMAGEONLY)
    add_subdirectory(audio)
    add_subdirectory(clock)
    add_subdirectory(eeprom)
    add_subdirectory(examples)
    add_subdirectory(iommu)
    add_subdirectory(net)
    add_subdirectory(power)
    add_subdirectory(printer)
    add_subdirectory(sensors)
    add_subdirectory(system)
    add_subdirectory(usb)
    add_subdirectory(video)
    add_subdirectory(vmm_guest)
endif()

# Required drivers (always built)
add_subdirectory(bus)
add_subdirectory(tty)
add_subdirectory(hid)

# Storage must be last (ramdisk dependency)
add_subdirectory(storage)
