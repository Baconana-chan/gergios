# CMakeLists.txt - Root build configuration for MINIX 3
#
# This is a PROTOTYPE for the CMake-based build system migration.
# See cmake/EVALUATION.md for the full analysis.
#
# Usage:
#   mkdir build && cd build
#   cmake .. -DCMAKE_TOOLCHAIN_FILE=../cmake/toolchain-minix.cmake
#   ninja kernel
#   ctest

cmake_minimum_required(VERSION 3.21)
project(MINIX
    VERSION 3.4.0
    DESCRIPTION "MINIX 3 Operating System"
    LANGUAGES C ASM
)

# ============================================================================
# Global configuration
# ============================================================================

# Include MINIX-specific CMake modules
list(INSERT CMAKE_MODULE_PATH 0 "${CMAKE_CURRENT_SOURCE_DIR}/cmake")

# C standard: MINIX uses C99/C11 (migrating from C89)
set(CMAKE_C_STANDARD 99)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)

# Default build type
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Release" CACHE STRING "Build type" FORCE)
endif()

# ============================================================================
# MINIX-specific options (mapped from MK* and USE* variables)
# ============================================================================

include(options)

# ============================================================================
# Architecture detection
# ============================================================================

# MACHINE_ARCH is set by the toolchain file or the presets
# Possible values: i386, x86_64, earm, aarch64 (future)
# x86_64 is currently BUILD INFRASTRUCTURE ONLY (Phase 2+)
if(NOT MACHINE_ARCH)
    if(CMAKE_SYSTEM_PROCESSOR MATCHES "^(x86_64|amd64)$")
        set(MACHINE_ARCH "x86_64")
    elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "^(i.86|x86|amd64)$")
        set(MACHINE_ARCH "i386")
    elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "^(arm|earm)")
        set(MACHINE_ARCH "earm")
    else()
        set(MACHINE_ARCH "i386" CACHE STRING "Target architecture")
    endif()
endif()

message(STATUS "Target architecture: ${MACHINE_ARCH}")

# Include architecture-specific configuration
# Note: include() with a / in the name looks for a file path.
# We use module-style naming via CMAKE_MODULE_PATH.
include(arch_${MACHINE_ARCH})

# ============================================================================
# Global compile definitions and flags
# ============================================================================

add_compile_definitions(
    $<$<CONFIG:Debug>:_DEBUG>
    __MINIX__
    _MINIX_SYSTEM=1
)

# Disable stack protector (MINIX doesn't use it)
add_compile_options(
    -fno-stack-protector
    -fno-builtin
)

# ============================================================================
# Include targets
# ============================================================================

# Root source directory is the project root
set(MINIX_SOURCE_DIR "${CMAKE_CURRENT_SOURCE_DIR}")

# Core libraries
add_subdirectory(crypto/external/gpl2/wolfssl)

# System libraries (libsys, libminc, libtimers, libbdev, etc.)
add_subdirectory(minix/lib)

# Kernel (prototype)
add_subdirectory(minix/kernel)

# Servers (core + optional)
add_subdirectory(minix/servers)

# Drivers (core + optional, storage must be last)
add_subdirectory(minix/drivers)

# Filesystem servers
add_subdirectory(minix/fs)

# Network services
add_subdirectory(minix/net)

# Userland commands
add_subdirectory(minix/commands)
add_subdirectory(minix/usr.bin)
add_subdirectory(minix/usr.sbin)

# Enable testing (must be before add_subdirectory for tests)
enable_testing()

# Tests
add_subdirectory(tests)
