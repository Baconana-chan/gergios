# cmake/options.cmake
# MINIX-specific build options (mapped from MK* and USE* variables in bsd.own.mk)
#
# This file replicates the option handling from share/mk/bsd.own.mk
# and minix.service.mk for the CMake build.
#
# Usage: cmake -DUSE_WATCHDOG=ON -DUSE_SMP=ON ..

include(CMakeDependentOption)

# ============================================================================
# MK* variables (default YES)
# ============================================================================

# These correspond to the _MKVARS.yes list in bsd.own.mk
option(MKCRYPTO "Enable crypto support" ON)
option(MKCXX "Enable C++ support" ON)
option(MKATF "Enable ATF/Automated Test Framework" ON)
option(MKDOC "Build documentation" ON)
option(MKMAN "Build man pages" ON)
option(MKNLS "Build NLS support" ON)
option(MKPROFILE "Build profiling libraries" OFF)
option(MKPIC "Build position-independent code" ON)
option(MKDEBUG "Build with debug symbols" OFF)

# MINIX-specific MK* options
option(MKSYSDEBUG "Enable system debugging" ON)
option(MKLIVEUPDATE "Enable live update support" ON)
option(MKLLVMCMDS "Build LLVM tools" ON)
option(MKSMALL "Build small footprint variant" OFF)
option(MKBITCODE "Build with LLVM bitcode" OFF)
option(MKMAGIC "Build with MAGIC pass" OFF)
option(MKASR "Build with ASR (Address Space Randomization)" OFF)
option(MKCOVERAGE "Build with coverage profiling" OFF)

# Components that are BSD-Make-only for now (games, external) are not
# included in the CMake build. The CMake build focuses on the MINIX
# microkernel core (kernel, servers, drivers, libraries).
# For games and external packages, use BSD Make or pkgsrc.

# ============================================================================
# USE_* variables (default YES, depend on MK*)
# ============================================================================

# x86-specific options (from bsd.own.mk: _MKVARS.yes adds these
# only when MACHINE_ARCH matches x86 architectures)
# Default ON for x86 and x86_64; arch_earm.cmake forces OFF for ARM.
option(USE_WATCHDOG "Watchdog driver support" ON)
option(USE_ACPI "ACPI power management support" ON)
option(USE_APIC "APIC interrupt controller support" ON)
option(USE_DEBUGREG "Debug register support" ON)
option(USE_PCI "PCI bus support" ON)

cmake_dependent_option(USE_SYSDEBUG "Enable system debugging routines" ON
    "MKSYSDEBUG" OFF)

cmake_dependent_option(USE_LIVEUPDATE "Enable live update" ON
    "MKLIVEUPDATE" OFF)

cmake_dependent_option(USE_BITCODE "Enable LLVM bitcode" ON
    "MKBITCODE" OFF)

cmake_dependent_option(USE_MAGIC "Enable MAGIC runtime library" ON
    "MKMAGIC" OFF)

cmake_dependent_option(USE_ASR "Enable ASR rerandomization" ON
    "MKASR" OFF)

# SMP (not from bsd.own.mk directly, but used in MINIX)
option(CONFIG_SMP "Enable symmetric multiprocessing" OFF)
set(CONFIG_MAX_CPUS "4" CACHE STRING "Maximum number of CPUs for SMP")

# ============================================================================
# Compile definitions from options
# ============================================================================

# Map USE_* options to -D defines
if(USE_WATCHDOG)
    add_compile_definitions(USE_WATCHDOG=1)
endif()
if(USE_ACPI)
    add_compile_definitions(USE_ACPI)
endif()
if(USE_APIC)
    add_compile_definitions(USE_APIC)
endif()
if(USE_DEBUGREG)
    add_compile_definitions(USE_DEBUGREG)
endif()
if(USE_SYSDEBUG)
    add_compile_definitions(USE_SYSDEBUG=1)
endif()
if(USE_LIVEUPDATE)
    add_compile_definitions(USE_UPDATE=1)
endif()
if(USE_PCI)
    add_compile_definitions(USE_PCI)
endif()
if(CONFIG_SMP)
    add_compile_definitions(CONFIG_SMP)
    add_compile_definitions(CONFIG_MAX_CPUS=${CONFIG_MAX_CPUS})
endif()
if(MKCOVERAGE)
    # Coverage profiling flags (equivalent to scripts/generate_coverage.sh)
    add_compile_options(-fprofile-instr-generate -fcoverage-mapping)
endif()
