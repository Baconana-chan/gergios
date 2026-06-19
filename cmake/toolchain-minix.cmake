# cmake/toolchain-minix.cmake
# Toolchain file for building MINIX (native or cross-compilation)
#
# Usage:
#   cmake -DCMAKE_TOOLCHAIN_FILE=cmake/toolchain-minix.cmake \
#         -DMACHINE_ARCH=i386 \
#         -DMINIX_DESTDIR=/path/to/destdir \
#         ..
#
# Native build (on MINIX):
#   cmake ..
#
# Cross-compilation (on Linux/macOS/Windows):
#   cmake -DCMAKE_TOOLCHAIN_FILE=cmake/toolchain-minix.cmake \
#         -DMACHINE_ARCH=i386 \
#         -DMINIX_TOOLCHAIN=/opt/minix-tools \
#         ..

# Target system identification
set(CMAKE_SYSTEM_NAME "Minix")
set(CMAKE_SYSTEM_VERSION "3.4.0")

# Parse MACHINE_ARCH from cache or command line
set(MACHINE_ARCH "x86_64" CACHE STRING "MINIX target architecture")

# Set CMAKE_SYSTEM_PROCESSOR based on MACHINE_ARCH
if(MACHINE_ARCH STREQUAL "i386")
    set(CMAKE_SYSTEM_PROCESSOR "i686")
elseif(MACHINE_ARCH STREQUAL "earm")
    set(CMAKE_SYSTEM_PROCESSOR "armv7a")
elseif(MACHINE_ARCH STREQUAL "x86_64")
    set(CMAKE_SYSTEM_PROCESSOR "x86_64")
elseif(MACHINE_ARCH STREQUAL "aarch64")
    set(CMAKE_SYSTEM_PROCESSOR "aarch64")
else()
    message(FATAL_ERROR "Unknown MACHINE_ARCH: ${MACHINE_ARCH}")
endif()

# MINIX DESTDIR for sysroot
set(MINIX_DESTDIR "" CACHE PATH "MINIX DESTDIR (sysroot)")

if(MINIX_DESTDIR)
    set(CMAKE_FIND_ROOT_PATH "${MINIX_DESTDIR}")
    set(CMAKE_FIND_ROOT_PATH_MODE_PROGRAM NEVER)
    set(CMAKE_FIND_ROOT_PATH_MODE_LIBRARY ONLY)
    set(CMAKE_FIND_ROOT_PATH_MODE_INCLUDE ONLY)
    set(CMAKE_FIND_ROOT_PATH_MODE_PACKAGE ONLY)
endif()

# Helper macro: set toolchain prefix based on architecture
macro(_set_tool_prefix PREFIX)
    set(_TOOL_PREFIX "${PREFIX}")
endmacro()

# Select ELF format and tool prefix based on architecture
if(MACHINE_ARCH STREQUAL "x86_64")
    _set_tool_prefix("x86_64-elf64-minix")
elseif(MACHINE_ARCH STREQUAL "aarch64")
    _set_tool_prefix("aarch64-elf64-minix")
else()
    # i386, earm, and others use 32-bit ELF
    _set_tool_prefix("${MACHINE_ARCH}-elf32-minix")
endif()

# MINIX cross-compilation toolchain prefix
set(MINIX_TOOLCHAIN "" CACHE PATH "MINIX cross-toolchain prefix directory")

if(MINIX_TOOLCHAIN)
    # Cross-compilation: use prefixed tools
    set(CMAKE_C_COMPILER "${MINIX_TOOLCHAIN}/bin/${_TOOL_PREFIX}-clang")
    set(CMAKE_CXX_COMPILER "${MINIX_TOOLCHAIN}/bin/${_TOOL_PREFIX}-clang++")
    set(CMAKE_ASM_COMPILER "${CMAKE_C_COMPILER}")
    set(CMAKE_AR "${MINIX_TOOLCHAIN}/bin/${_TOOL_PREFIX}-ar")
    set(CMAKE_RANLIB "${MINIX_TOOLCHAIN}/bin/${_TOOL_PREFIX}-ranlib")
    set(CMAKE_LINKER "${MINIX_TOOLCHAIN}/bin/${_TOOL_PREFIX}-ld")
    set(CMAKE_OBJCOPY "${MINIX_TOOLCHAIN}/bin/${_TOOL_PREFIX}-objcopy")
    set(CMAKE_OBJDUMP "${MINIX_TOOLCHAIN}/bin/${_TOOL_PREFIX}-objdump")
    set(CMAKE_STRIP "${MINIX_TOOLCHAIN}/bin/${_TOOL_PREFIX}-strip")
else()
    # Native build (running on MINIX): use system tools
    # CMAKE_C_COMPILER and CMAKE_ASM_COMPILER are auto-detected
    # Post-process object names to match MINIX toolchain conventions
    set(CMAKE_AR "ar" CACHE FILEPATH "Path to ar")
    set(CMAKE_RANLIB "ranlib" CACHE FILEPATH "Path to ranlib")
endif()

# Always use static linking for MINIX
set(CMAKE_FIND_LIBRARY_SUFFIXES ".a")
set(BUILD_SHARED_LIBS OFF)
set(CMAKE_EXE_LINKER_FLAGS "-static")

# MINIX uses ELF format
set(CMAKE_EXECUTABLE_FORMAT "ELF")

# Hardware floating-point ABI for ARM
if(MACHINE_ARCH STREQUAL "earm")
    set(CMAKE_C_FLAGS "${CMAKE_C_FLAGS} -mno-unaligned-access")
endif()
