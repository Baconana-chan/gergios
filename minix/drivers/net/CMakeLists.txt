# minix/drivers/net/CMakeLists.txt
# Network drivers
#
# Replicates minix/drivers/net/Makefile

if(MACHINE_ARCH STREQUAL "i386")
    add_subdirectory(3c90x)
    add_subdirectory(atl2)
    add_subdirectory(dec21140A)
    add_subdirectory(dp8390)
    add_subdirectory(dpeth)
    add_subdirectory(e1000)
    add_subdirectory(fxp)
    add_subdirectory(ip1000)
    add_subdirectory(lance)
    add_subdirectory(rtl8139)
    add_subdirectory(rtl8169)
    add_subdirectory(virtio_net)
    add_subdirectory(vt6105)
elseif(MACHINE_ARCH STREQUAL "earm")
    add_subdirectory(lan8710a)
endif()
