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
# Default ON for x86 and x86_64; arch_earm.cmake and arch_aarch64.cmake
# force OFF for non-x86 architectures.
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
# Architecture-specific compile definitions are applied in the root
# CMakeLists.txt AFTER arch_${MACHINE_ARCH}.cmake is loaded, so that
# arch-specific overrides (e.g., forcing USE_APIC=OFF for ARM) are
# respected when the compile definitions are added.
#
# See CMakeLists.txt section "Global compile definitions and flags"
# for the USE_* → -D mapping.
# ============================================================================
