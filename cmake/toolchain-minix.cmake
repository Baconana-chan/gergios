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
    # CMAKE_C_COMPILER and CMAKE_ASM_COMPILER are auto-detected.
    # For cross-compilation on non-MINIX hosts, set the target triple.
    # Using CMAKE_C_COMPILER_TARGET is the proper CMake way (avoids
    # duplication issues with CMAKE_C_FLAGS).

    # Auto-detect Clang when no compiler is set (e.g., on Windows/MSVC hosts).
    # On Windows, CMake defaults to MSVC cl.exe; on Linux/macOS, CC/CMAKE_C_COMPILER
    # may already be set. Only search when the user hasn't explicitly chosen.
    if(NOT CMAKE_C_COMPILER)
        find_program(CMAKE_C_COMPILER
            NAMES clang
            PATHS "C:/Program Files/LLVM/bin" "/usr/lib/llvm/bin" "/usr/bin"
            NO_CMAKE_PATH NO_CMAKE_ENVIRONMENT_PATH
            DOC "C compiler for MINIX cross-build"
        )
        if(CMAKE_C_COMPILER)
            # Derive CXX compiler from the same directory
            get_filename_component(_CLANG_DIR "${CMAKE_C_COMPILER}" DIRECTORY)
            find_program(CMAKE_CXX_COMPILER
                NAMES clang++
                PATHS "${_CLANG_DIR}" "C:/Program Files/LLVM/bin"
                NO_CMAKE_PATH NO_CMAKE_ENVIRONMENT_PATH
                DOC "C++ compiler for MINIX cross-build"
            )
            mark_as_advanced(CMAKE_C_COMPILER CMAKE_CXX_COMPILER)
        endif()
    endif()

    if(MACHINE_ARCH STREQUAL "x86_64")
        set(CMAKE_C_COMPILER_TARGET "x86_64-elf")
        set(CMAKE_ASM_COMPILER_TARGET "x86_64-elf")
    elseif(MACHINE_ARCH STREQUAL "aarch64")
        set(CMAKE_C_COMPILER_TARGET "aarch64-elf")
        set(CMAKE_ASM_COMPILER_TARGET "aarch64-elf")
    elseif(MACHINE_ARCH STREQUAL "earm")
        set(CMAKE_C_COMPILER_TARGET "armv7a-unknown-none-eabi")
        set(CMAKE_ASM_COMPILER_TARGET "armv7a-unknown-none-eabi")
    endif()
    # Use LLVM's lld for cross-linking (supports all targets via -fuse-ld)
    # Clang ignores CMAKE_LINKER and searches for prefixed linker names.
    # -fuse-ld=lld tells Clang to use lld which supports any target triple.
    find_program(LLD_LINKER ld.lld)
    if(LLD_LINKER)
        set(CMAKE_EXE_LINKER_FLAGS "${CMAKE_EXE_LINKER_FLAGS} -fuse-ld=lld")
        set(CMAKE_SHARED_LINKER_FLAGS "${CMAKE_SHARED_LINKER_FLAGS} -fuse-ld=lld")
        # CMake compiler test doesn't use CMAKE_EXE_LINKER_FLAGS.
        # Set try_compile target type to STATIC_LIBRARY to skip the
        # linker test (compilation-only check still validates the compiler).
        set(CMAKE_TRY_COMPILE_TARGET_TYPE "STATIC_LIBRARY")
    else()
        message(WARNING "ld.lld not found! Cross-linker for aarch64 may not work.")
    endif()
    # Find LLVM archiver tools (ar/ranlib not available on non-MINIX hosts)
    find_program(LLVM_AR llvm-ar)
    find_program(LLVM_RANLIB llvm-ranlib)
    if(LLVM_AR)
        set(CMAKE_AR "${LLVM_AR}" CACHE FILEPATH "Path to archiver")
    else()
        set(CMAKE_AR "ar" CACHE FILEPATH "Path to ar")
    endif()
    if(LLVM_RANLIB)
        set(CMAKE_RANLIB "${LLVM_RANLIB}" CACHE FILEPATH "Path to ranlib")
    else()
        set(CMAKE_RANLIB "ranlib" CACHE FILEPATH "Path to ranlib")
    endif()
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
