# minix/drivers/audio/CMakeLists.txt
# Audio drivers
#
# Replicates minix/drivers/audio/Makefile

if(NOT MKIMAGEONLY AND MACHINE_ARCH STREQUAL "i386")
    add_subdirectory(als4000)
    add_subdirectory(cmi8738)
    add_subdirectory(cs4281)
    add_subdirectory(es1370)
    add_subdirectory(es1371)
    add_subdirectory(sb16)
    add_subdirectory(trident)
endif()
