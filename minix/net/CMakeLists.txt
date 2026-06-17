# minix/net/CMakeLists.txt
# Network service build
#
# Replicates minix/net/Makefile

if(NOT MKIMAGEONLY)
    add_subdirectory(lwip)
    add_subdirectory(uds)
endif()
