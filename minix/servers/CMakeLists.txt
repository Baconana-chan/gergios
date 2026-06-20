# minix/servers/CMakeLists.txt
# Server component build
#
# Replicates minix/servers/Makefile and minix/servers/Makefile.inc

# All servers install to /service by default, no man pages
set(BINDIR "/service")

# Core servers (always built)
add_subdirectory_if_exists(pm)
add_subdirectory_if_exists(vfs)
add_subdirectory_if_exists(vm)
add_subdirectory_if_exists(rs)
add_subdirectory_if_exists(sched)
add_subdirectory_if_exists(ds)
add_subdirectory_if_exists(mib)
add_subdirectory_if_exists(input)

# Optional servers (only when MKIMAGEONLY is OFF)
if(NOT MKIMAGEONLY)
    add_subdirectory_if_exists(ipc)
    add_subdirectory_if_exists(is)
    add_subdirectory_if_exists(devman)
endif()
