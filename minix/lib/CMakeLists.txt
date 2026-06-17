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

add_subdirectory(libasyn)
add_subdirectory(libaudiodriver)
add_subdirectory(libbdev)
add_subdirectory(libblockdriver)
add_subdirectory(libchardriver)
add_subdirectory(libddekit)
add_subdirectory(libdevman)
add_subdirectory(libexec)
add_subdirectory(libfsdriver)
add_subdirectory(libinputdriver)
add_subdirectory(liblwip)
add_subdirectory(libminc)
add_subdirectory(libminixfs)
add_subdirectory(libnetdriver)
add_subdirectory(libsffs)
add_subdirectory(libsockdriver)
add_subdirectory(libsockevent)
add_subdirectory(libtimers)
add_subdirectory(libusb)
add_subdirectory(libvtreefs)

# Empty libgcc_s for systems without libgcc
if(NOT HAVE_LIBGCC)
    add_subdirectory(libgcc_s_empty)
endif()

# ============================================================================
# Architecture-specific libraries
# ============================================================================

if(MACHINE_ARCH STREQUAL "i386")
    add_subdirectory(libacpi)
    add_subdirectory(libhgfs)
    add_subdirectory(libvassert)
    add_subdirectory(libvboxfs)
    add_subdirectory(libvirtio)
elseif(MACHINE_ARCH STREQUAL "earm")
    add_subdirectory(libclkconf)
    add_subdirectory(libgpio)
    add_subdirectory(libi2cdriver)
endif()

# Magic runtime library (optional)
if(MKMAGIC)
    add_subdirectory(libmagicrt)
endif()
