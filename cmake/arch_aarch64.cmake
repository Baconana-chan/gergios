# cmake/arch_aarch64.cmake
# Architecture-specific definitions for ARM64 (AArch64)
#
# ARM64 is the 64-bit ARM architecture (ARMv8-A and later).
# This is a ground-up port — ARM64 shares NO code paths with
# 32-bit ARM (earm) at the assembly/register level.
#
# See planning/08_arm64_migration_plan.md for the full port plan.

# CPU model for ARM64
set(MACHINE_CPU "aarch64")

# GNU architecture naming
set(GNU_ARCH "aarch64")

# GNAT platform triple — 64-bit ELF
set(MACHINE_GNU_PLATFORM "${GNU_ARCH}-elf64-minix")

# ARM64-specific compile flags
# ARMv8-A is the baseline (Cortex-A53/A72)
add_compile_options(
    -march=armv8-a
    -mstrict-align       # Prevent unaligned memory access in kernel
)

# Omit frame pointer in Release builds (saves x29 register)
# Debug builds keep frame pointers for stack trace support
add_compile_options(
    "$<$<CONFIG:Release>:-fomit-frame-pointer>"
)

# ARM64-specific link flags
# The kernel will use a custom linker script (Phase 2)
set(KERNEL_LINKER_SCRIPT "arch/${MACHINE_ARCH}/kernel.lds")

# ARM64-specific compile definitions
add_compile_definitions(
    __aarch64__
    __ARM_ARCH_8__
    _LP64
    __LP64__
)

# Disable x86-specific options that don't apply to ARM64
# (same pattern as arch_earm.cmake)
set(USE_WATCHDOG OFF CACHE BOOL "Watchdog driver support" FORCE)
set(USE_ACPI OFF CACHE BOOL "ACPI power management support" FORCE)
set(USE_APIC OFF CACHE BOOL "APIC interrupt controller support" FORCE)
set(USE_DEBUGREG OFF CACHE BOOL "Debug register support" FORCE)
set(USE_PCI OFF CACHE BOOL "PCI bus support" FORCE)

# ARM64-specific notes:
# - Uses 4-level translation tables (4KB pages, 48-bit VA)
# - Exception model: 4 exception levels (EL0–EL3)
# - Generic Interrupt Controller (GIC v2/v3) instead of APIC
# - ARM Generic Timer instead of LAPIC/HPET
# - SVC #0 for system calls instead of SYSCALL/SYSENTER
# - 31 general-purpose registers (x0–x30) vs x86_64's 16
# - Device Tree (FDT) instead of ACPI for platform description
