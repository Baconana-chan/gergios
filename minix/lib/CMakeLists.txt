# minix/lib/CMakeLists.txt
# Library build
#
# Replicates minix/lib/Makefile and minix/lib/Makefile.inc

# ============================================================================
# Core libraries (always built, with ordering)
# ============================================================================

# libsys: system library for servers/drivers (built first)
add_subdirectory(libsys)

# libmthread: threading support (built after libsys)
add_subdirectory(libmthread)

# ============================================================================
# All other libraries (order-independent)
# ============================================================================

add_subdirectory_if_exists(libasyn)
add_subdirectory_if_exists(libaudiodriver)
add_subdirectory_if_exists(libbdev)
add_subdirectory_if_exists(libblockdriver)
add_subdirectory_if_exists(libchardriver)
add_subdirectory_if_exists(libddekit)
add_subdirectory_if_exists(libdevman)
add_subdirectory_if_exists(libexec)
add_subdirectory_if_exists(libfsdriver)
add_subdirectory_if_exists(libgergios_driver)
add_subdirectory_if_exists(libinputdriver)
add_subdirectory_if_exists(liblwip)
add_subdirectory_if_exists(libminc)
add_subdirectory_if_exists(libminixfs)
add_subdirectory_if_exists(libnetdriver)
add_subdirectory_if_exists(libsffs)
add_subdirectory_if_exists(libsockdriver)
add_subdirectory_if_exists(libsockevent)
add_subdirectory_if_exists(libtimers)
add_subdirectory_if_exists(libusb)
add_subdirectory_if_exists(libvtreefs)

# Empty libgcc_s for systems without libgcc
if(NOT HAVE_LIBGCC)
    add_subdirectory_if_exists(libgcc_s_empty)
endif()

# ============================================================================
# Architecture-specific libraries
# ============================================================================

if(MACHINE_ARCH STREQUAL "earm")
    add_subdirectory(libclkconf)
    add_subdirectory(libgpio)
    add_subdirectory(libi2cdriver)
endif()

# Magic runtime library (optional)
if(MKMAGIC)
    add_subdirectory(libmagicrt)
endif()
