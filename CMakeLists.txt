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
# Rust Integration (Phase 3)
# ============================================================================

# Rust toolchain detection
find_program(CARGO_EXECUTABLE cargo)
if(CARGO_EXECUTABLE)
    execute_process(
        COMMAND "${CARGO_EXECUTABLE}" "--version"
        OUTPUT_VARIABLE CARGO_VERSION
        OUTPUT_STRIP_TRAILING_WHITESPACE
    )
    message(STATUS "Rust toolchain found: ${CARGO_VERSION}")
else()
    message(WARNING "Rust toolchain (cargo) not found — Rust components will be skipped")
endif()

# Option to enable Rust sanitizers
option(RUST_SANITIZE_ADDRESS "Enable AddressSanitizer for Rust components" OFF)
option(RUST_SANITIZE_UNDEFINED "Enable UndefinedBehaviorSanitizer for Rust components" OFF)
option(RUST_SANITIZE_THREAD "Enable ThreadSanitizer for Rust components" OFF)

# Build sanitizer flags for Rust
function(get_rust_sanitizer_flags out_var)
    set(flags "")
    if(RUST_SANITIZE_ADDRESS)
        string(APPEND flags " -Z sanitizer=address")
    endif()
    if(RUST_SANITIZE_UNDEFINED)
        string(APPEND flags " -Z sanitizer=undefined")
    endif()
    if(RUST_SANITIZE_THREAD)
        string(APPEND flags " -Z sanitizer=thread")
    endif()
    string(STRIP "${flags}" flags)
    set(${out_var} "${flags}" PARENT_SCOPE)
endfunction()

# Function to add a Rust-based userland utility
function(add_rust_utility name)
    if(NOT CARGO_EXECUTABLE)
        message(STATUS "Rust toolchain not available: skipping ${name}")
        return()
    endif()

    set(RUST_DIR "${CMAKE_CURRENT_SOURCE_DIR}/rust/${name}")

    if(NOT EXISTS "${RUST_DIR}/Cargo.toml")
        message(WARNING "Rust crate '${name}' not found at ${RUST_DIR} — skipping")
        return()
    endif()

    set(CARGO_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/rust")

    # Get sanitizer flags
    get_rust_sanitizer_flags(RUST_SANITIZER_FLAGS)

    if(RUST_SANITIZER_FLAGS)
        message(STATUS "Rust ${name}: enabling sanitizers: ${RUST_SANITIZER_FLAGS}")
        # Use cmake -E env to set RUSTFLAGS correctly (COMMAND doesn't support ENV_VAR=value prefix)
        add_custom_target(${name}
            ALL
            COMMAND ${CMAKE_COMMAND} -E env "RUSTFLAGS=${RUST_SANITIZER_FLAGS}"
                "${CARGO_EXECUTABLE}" build
                --manifest-path "${RUST_DIR}/Cargo.toml"
                --target-dir "${CARGO_TARGET_DIR}"
                $<$<CONFIG:Release>:--release>
            COMMENT "Building Rust utility: ${name} (sanitizer: ${RUST_SANITIZER_FLAGS})"
            WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
        )
    else()
        add_custom_target(${name}
            ALL
            COMMAND "${CARGO_EXECUTABLE}" build
                --manifest-path "${RUST_DIR}/Cargo.toml"
                --target-dir "${CARGO_TARGET_DIR}"
                $<$<CONFIG:Release>:--release>
            COMMENT "Building Rust utility: ${name}"
            WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
        )
    endif()

    # Install target — use CMAKE_BUILD_TYPE (CMake standard)
    set(BUILD_SUBDIR "debug")
    if(CMAKE_BUILD_TYPE STREQUAL "Release")
        set(BUILD_SUBDIR "release")
    endif()

    install(
        PROGRAMS "${CARGO_TARGET_DIR}/${BUILD_SUBDIR}/${name}${CMAKE_EXECUTABLE_SUFFIX}"
        DESTINATION bin
    )
endfunction()

# Function to register Rust test targets with CTest
function(add_rust_test name)
    if(NOT CARGO_EXECUTABLE)
        return()
    endif()

    set(RUST_DIR "${CMAKE_CURRENT_SOURCE_DIR}/rust/${name}")

    if(NOT EXISTS "${RUST_DIR}/Cargo.toml")
        return()
    endif()

    add_test(
        NAME "rust_${name}"
        COMMAND "${CARGO_EXECUTABLE}" test
            --manifest-path "${RUST_DIR}/Cargo.toml"
        WORKING_DIRECTORY "${CMAKE_CURRENT_SOURCE_DIR}"
    )
endfunction()

# ============================================================================
# Global configuration
# ============================================================================

# Include MINIX-specific CMake modules
list(INSERT CMAKE_MODULE_PATH 0 "${CMAKE_CURRENT_SOURCE_DIR}/cmake")

# Load MINIX custom macros (add_minix_executable, add_minix_library, etc.)
include(macros)

# C standard: GergiOS uses C17 (ISO/IEC 9899:2018), migrating from C99/GNU99.
# GNU extensions are enabled because the kernel and some drivers use
# __asm__, __attribute__, and other GCC/Clang extensions extensively.
set(CMAKE_C_STANDARD 17)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS ON)

# Default build type
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE "Release" CACHE STRING "Build type" FORCE)
endif()

# ============================================================================
# MINIX-specific options (mapped from MK* and USE* variables)
# ============================================================================

include(options)

# ============================================================================
# i386 Hard Deprecation (Phase 3)
# ============================================================================

# Since Phase 3, i386 builds require explicit opt-in.
# Without MKI386=ON, building for i386 is a fatal error.
option(MKI386 "Enable i386 (32-bit x86) build (Phase 3 hard deprecated)" OFF)

# ============================================================================
# Architecture detection
# ============================================================================

# MACHINE_ARCH is set by the toolchain file or the presets
# Possible values: i386 (Phase 3 hard deprecated), x86_64 (default), earm, aarch64 (future)
#
# Since Phase 2, x86_64 is the default build target.
# Since Phase 3, i386 requires MKI386=ON.
if(NOT MACHINE_ARCH)
    if(CMAKE_SYSTEM_PROCESSOR MATCHES "^(x86_64|amd64)$")
        set(MACHINE_ARCH "x86_64")
    elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "^(i.86|x86|amd64)$")
        # i386 host detected — fall back to x86_64
        message(WARNING "i386 host architecture detected. i386 is Phase 3 hard deprecated — defaulting to x86_64.")
        set(MACHINE_ARCH "x86_64")
    elseif(CMAKE_SYSTEM_PROCESSOR MATCHES "^(arm|earm)")
        set(MACHINE_ARCH "earm")
    else()
        message(STATUS "Unknown host architecture '${CMAKE_SYSTEM_PROCESSOR}', defaulting to x86_64")
        set(MACHINE_ARCH "x86_64" CACHE STRING "Target architecture")
    endif()
endif()

# Phase 3 guard: i386 requires explicit MKI386=ON
if(MACHINE_ARCH STREQUAL "i386" AND NOT MKI386)
    message(FATAL_ERROR "\n"
        "  ================================================================\n"
        "  i386 (32-bit x86) is Phase 3 HARD DEPRECATED.\n"
        "  i386 builds are NOT allowed by default.\n"
        "  ================================================================\n"
        "  To build for i386, you MUST explicitly enable it:\n"
        "    cmake -DMACHINE_ARCH=i386 -DMKI386=ON ..\n"
        "\n"
        "  i386 support is community-only, best-effort.\n"
        "  See docs/i386-hard-deprecation-notice.md for full policy.\n"
        "  ================================================================\n")
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
    __minix
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

# Core libraries (skip wolfSSL for aarch64 — dist not fully migrated)
if(NOT MACHINE_ARCH STREQUAL "aarch64")
    add_subdirectory(crypto/external/gpl2/wolfssl)
endif()

# System libraries (libsys, libminc, libtimers, libbdev, etc.)
# Skip minc for aarch64 — libc arch sources not yet migrated
if(MACHINE_ARCH STREQUAL "aarch64")
    add_subdirectory(minix/lib/libsys)
    add_subdirectory(minix/lib/libmthread)
    add_subdirectory_if_exists(minix/lib/libbdev)
    add_subdirectory_if_exists(minix/lib/libblockdriver)
    add_subdirectory_if_exists(minix/lib/libchardriver)
    add_subdirectory_if_exists(minix/lib/libddekit)
    add_subdirectory_if_exists(minix/lib/libexec)
    add_subdirectory_if_exists(minix/lib/libfsdriver)
    add_subdirectory_if_exists(minix/lib/liblwip)
    add_subdirectory_if_exists(minix/lib/libtimers)
    add_subdirectory_if_exists(minix/lib/libvtreefs)
else()
    add_subdirectory(minix/lib)
endif()

# Kernel (prototype)
add_subdirectory(minix/kernel)

# Servers (core + optional)
add_subdirectory(minix/servers)

# Drivers (core + optional, storage must be last)
add_subdirectory(minix/drivers)

# Filesystem servers (skip for aarch64 — not yet migrated)
if(NOT MACHINE_ARCH STREQUAL "aarch64")
    add_subdirectory(minix/fs)
endif()

# Network services (skip for aarch64 — CMake prototype incomplete)
if(NOT MACHINE_ARCH STREQUAL "aarch64")
    add_subdirectory(minix/net)
endif()

# Rust userland utilities (Phase 3)
if(CARGO_EXECUTABLE)
    add_rust_utility(basename)
    add_rust_utility(dirname)
    add_rust_utility(echo)
    add_rust_utility(false)
    add_rust_utility(grep)
    add_rust_utility(seq)
    add_rust_utility(sleep)
    add_rust_utility(true)
    add_rust_utility(yes)
endif()

# Userland commands (skip for aarch64 until CMake migration is complete)
if(NOT MACHINE_ARCH STREQUAL "aarch64")
    add_subdirectory(minix/commands)
    add_subdirectory(minix/usr.bin)
    add_subdirectory(minix/usr.sbin)
endif()

# Enable testing (must be before add_subdirectory for tests)
enable_testing()

# Rust component tests (Phase 3 + Phase 4)
# Skip for aarch64 cross-compilation
if(CARGO_EXECUTABLE AND NOT MACHINE_ARCH STREQUAL "aarch64")
    add_rust_test(basename)
    add_rust_test(dirname)
    add_rust_test(echo)
    add_rust_test(grep)
    add_rust_test(minix-rs)
endif()

# Phase 4: Library crate tests (run via cargo test --lib)
if(CARGO_EXECUTABLE)
    add_rust_test(audio-buf)
    add_rust_test(procfs-path)
    add_rust_test(net-parse)
endif()

# Tests
add_subdirectory(tests)
