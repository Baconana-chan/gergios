# minix/drivers/hid/CMakeLists.txt
# Human Interface Device drivers
#
# Replicates minix/drivers/hid/Makefile

if(MACHINE_ARCH STREQUAL "i386")
    add_subdirectory(pckbd)
endif()
