# minix/drivers/net/CMakeLists.txt
# Network drivers
#
# Replicates minix/drivers/net/Makefile
#
# NOTE: i386-specific network drivers (3c90x, e1000, rtl8139, etc.)
# have been removed in Phase 4 i386 deprecation.

if(MACHINE_ARCH STREQUAL "earm")
    add_subdirectory(lan8710a)
endif()
