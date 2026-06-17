# minix/servers/CMakeLists.txt
# Server component build
#
# Replicates minix/servers/Makefile and minix/servers/Makefile.inc

# All servers install to /service by default, no man pages
set(BINDIR "/service")

# Core servers (always built)
add_subdirectory(pm)
add_subdirectory(vfs)
add_subdirectory(vm)
add_subdirectory(rs)
add_subdirectory(sched)
add_subdirectory(ds)
add_subdirectory(mib)
add_subdirectory(input)

# Optional servers (only when MKIMAGEONLY is OFF)
if(NOT MKIMAGEONLY)
    add_subdirectory(ipc)
    add_subdirectory(is)
    add_subdirectory(devman)
endif()
