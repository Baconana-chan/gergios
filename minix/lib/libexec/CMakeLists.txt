# minix/lib/libexec/CMakeLists.txt
# Exec library (libexec)
#
# Provides libexec_clear_memset, libexec_load_elf, and related
# functions used by the kernel (protect.c) and servers (PM, VFS, RS, VM).

add_minix_library(exec
    SOURCES
        exec_elf.c
        exec_general.c
    INSTALL_DIR /usr/lib
)

# Include paths for libexec sources
# The source directory itself is needed for <libexec.h> (used with <> syntax).
# Include paths for libexec sources
# The source directory itself is needed for <libexec.h> (used with <> syntax).
# The arch-specific include path is needed for <machine/elf.h> which
# delegates to the per-architecture elf.h header.
target_include_directories(exec PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}"
    "${MINIX_SOURCE_DIR}/minix/include"
    "${MINIX_SOURCE_DIR}/include"
    "${MINIX_SOURCE_DIR}/sys"
    "${MINIX_SOURCE_DIR}/minix/include/arch/${MACHINE_ARCH}/include"
)
