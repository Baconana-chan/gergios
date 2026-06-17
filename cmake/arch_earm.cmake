# cmake/arch/earm.cmake
# Architecture-specific definitions for ARM (earm)

# CPU model for ARM
set(MACHINE_CPU "arm")

# GNU architecture naming
set(GNU_ARCH "armv7-a")

# GNAT platform triple
set(MACHINE_GNU_PLATFORM "${GNU_ARCH}-elf32-minix")

# ARM-specific compile flags
add_compile_options(
    -march=armv7-a
    -mno-unaligned-access
)

# ARM-specific link flags
set(KERNEL_LINKER_SCRIPT "arch/${MACHINE_ARCH}/kernel.lds")

# ARM-specific compile definitions
add_compile_definitions(
    __arm__
)
