# cmake/arch/i386.cmake
# Architecture-specific definitions for i386

# CPU model for i386
set(MACHINE_CPU "i386")

# GNU architecture naming
set(GNU_ARCH "i586")

# GNAT platform triple
set(MACHINE_GNU_PLATFORM "${GNU_ARCH}-elf32-minix")

# i386-specific compile flags
add_compile_options(-march=i586)

# On native builds (when not using a cross-toolchain), ensure 32-bit mode;
# cross-compilers already target 32-bit by default.
if(NOT MINIX_TOOLCHAIN)
    add_compile_options(-m32)
endif()

# i386-specific link flags
# The kernel uses a custom linker script
set(KERNEL_LINKER_SCRIPT "arch/${MACHINE_ARCH}/kernel.lds")

# i386-specific compile definitions
add_compile_definitions(
    __i386__
)
