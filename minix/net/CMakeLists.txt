# minix/net/CMakeLists.txt
# Network service build
#
# Replicates minix/net/Makefile

if(NOT MKIMAGEONLY)
    add_subdirectory_if_exists(lwip)
    add_subdirectory_if_exists(uds)
endif()
