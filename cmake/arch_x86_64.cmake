# cmake/arch_x86_64.cmake
# Architecture-specific definitions for x86_64
#
# This is a prototype for the x86_64 architecture port.
# Status: BUILD INFRASTRUCTURE ONLY — kernel ASM, pagetables, and
# server code for x86_64 have not been implemented yet.

# CPU model for x86_64
set(MACHINE_CPU "x86_64")

# GNU architecture naming
set(GNU_ARCH "x86_64")

# GNAT platform triple — 64-bit ELF
set(MACHINE_GNU_PLATFORM "${GNU_ARCH}-elf64-minix")

# x86_64-specific compile flags
# Use generic x86-64 architecture (no specific uarch tuning)
add_compile_options(-march=x86-64)

# On native builds (when not using a cross-toolchain), ensure 64-bit mode
if(NOT MINIX_TOOLCHAIN)
    add_compile_options(-m64)
endif()

# x86_64-specific link flags
# The kernel will use a custom linker script (Phase 2)
set(KERNEL_LINKER_SCRIPT "arch/${MACHINE_ARCH}/kernel.lds")

# x86_64-specific compile definitions
add_compile_definitions(
    __x86_64__
    _LP64
    __LP64__
)

# x86_64 ABI: the kernel uses the System V AMD64 ABI
# Key differences from i386:
#   - 64-bit pointers (8 bytes vs 4)
#   - 16 general-purpose registers (vs 8)
#   - 128-byte red zone (disabled via -mno-red-zone for kernel)
#   - syscall/sysret instruction (vs int 0x80)
#   - 4-level page tables (vs 2-level legacy or 3-level PAE)
