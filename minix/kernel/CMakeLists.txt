# minix/kernel/CMakeLists.txt
# Kernel build (PROTOTYPE)
#
# This replicates minix/kernel/Makefile + minix/kernel/arch/*/Makefile.inc
# using CMake instead of BSD Make.

# ============================================================================
# Kernel executable
# ============================================================================

#
# Sources (shared between architectures)
#
set(KERNEL_CORE_SOURCES
    clock.c
    cpulocals.c
    interrupt.c
    main.c
    proc.c
    system.c
    table.c
    utility.c
    usermapped_data.c
)

# System call implementations
set(KERNEL_SYSTEM_SOURCES
    system/do_fork.c
    system/do_exec.c
    system/do_clear.c
    system/do_exit.c
    system/do_trace.c
    system/do_runctl.c
    system/do_update.c
    system/do_times.c
    system/do_setalarm.c
    system/do_stime.c
    system/do_settime.c
    system/do_vtimer.c
    system/do_irqctl.c
    system/do_copy.c
    system/do_umap.c
    system/do_umap_remote.c
    system/do_vumap.c
    system/do_memset.c
    system/do_setgrant.c
    system/do_privctl.c
    system/do_safecopy.c
    system/do_safememset.c
    system/do_diagctl.c
    system/do_getksig.c
    system/do_endksig.c
    system/do_kill.c
    system/do_sigsend.c
    system/do_sigreturn.c
    system/do_abort.c
    system/do_getinfo.c
    system/do_vmctl.c
    system/do_mcontext.c
    system/do_schedule.c
    system/do_schedctl.c
    system/do_statectl.c
)

# i386-specific system sources
if(MACHINE_ARCH STREQUAL "i386")
    list(APPEND KERNEL_SYSTEM_SOURCES
        system/do_devio.c
        system/do_vdevio.c
    )
endif()

# Architecture-specific sources
if(MACHINE_ARCH STREQUAL "i386")
    set(KERNEL_ARCH_SOURCES
        arch/i386/mpx.S
        arch/i386/arch_clock.c
        arch/i386/arch_do_vmctl.c
        arch/i386/arch_system.c
        arch/i386/klib.S
        arch/i386/memory.c
        arch/i386/protect.c
        arch/i386/direct_tty_utils.c
        arch/i386/arch_reset.c
        arch/i386/pg_utils.c
        arch/i386/do_iopenable.c
        arch/i386/do_readbios.c
        arch/i386/do_sdevio.c
        arch/i386/exception.c
        arch/i386/i8259.c
        arch/i386/oxpcie.c
        arch/i386/usermapped_data_arch.c
        arch/i386/io_inb.S
        arch/i386/io_inl.S
        arch/i386/io_intr.S
        arch/i386/io_inw.S
        arch/i386/io_outb.S
        arch/i386/io_outl.S
        arch/i386/io_outw.S
        arch/i386/usermapped_glo_ipc.S
    )
elseif(MACHINE_ARCH STREQUAL "earm")
    set(KERNEL_ARCH_SOURCES
        arch/earm/mpx.S
        arch/earm/arch_clock.c
        arch/earm/arch_do_vmctl.c
        arch/earm/arch_system.c
        arch/earm/klib.S
        arch/earm/memory.c
        arch/earm/protect.c
        arch/earm/direct_tty_utils.c
        arch/earm/arch_reset.c
        arch/earm/pg_utils.c
        arch/earm/phys_copy.S
        arch/earm/phys_memset.S
        arch/earm/exc.S
        arch/earm/exception.c
        arch/earm/hw_intr.c
        arch/earm/do_padconf.c
    )
endif()

# Optional sources
if(CONFIG_SMP)
    list(APPEND KERNEL_CORE_SOURCES smp.c)
    if(MACHINE_ARCH STREQUAL "i386")
        list(APPEND KERNEL_ARCH_SOURCES arch/i386/arch_smp.c arch/i386/trampoline.S)
    endif()
endif()

if(USE_WATCHDOG)
    list(APPEND KERNEL_CORE_SOURCES watchdog.c)
    if(MACHINE_ARCH STREQUAL "i386")
        list(APPEND KERNEL_ARCH_SOURCES arch/i386/arch_watchdog.c)
    endif()
endif()

if(USE_SYSDEBUG)
    list(APPEND KERNEL_CORE_SOURCES debug.c)
endif()

if(USE_APIC AND MACHINE_ARCH STREQUAL "i386")
    list(APPEND KERNEL_ARCH_SOURCES arch/i386/apic.c arch/i386/apic_asm.S)
endif()

if(USE_ACPI AND MACHINE_ARCH STREQUAL "i386")
    list(APPEND KERNEL_ARCH_SOURCES arch/i386/acpi.c)
endif()

if(USE_DEBUGREG AND MACHINE_ARCH STREQUAL "i386")
    list(APPEND KERNEL_ARCH_SOURCES arch/i386/breakpoints.c arch/i386/debugreg.S)
endif()

# Profiling
list(APPEND KERNEL_CORE_SOURCES profile.c)
list(APPEND KERNEL_SYSTEM_SOURCES system/do_sprofile.c)

# ============================================================================
# Unpaged object handling
# ============================================================================

# Architecture-specific unpaged objects (head.o, pre_init.o, etc.)
if(MACHINE_ARCH STREQUAL "i386")
    set(UNPAGED_ARCH_OBJECTS
        head.o
        pre_init.o
        direct_tty_utils.o
        pg_utils.o
        klib.o
        utility.o
        arch_reset.o
        io_inb.o
        io_outb.o
    )
    # Unpaged from libc
    set(UNPAGED_MINLIB_OBJECTS
        _cpufeature.o
        _cpuid.o
        get_bp.o
    )
    # Unpaged from libsys
    set(UNPAGED_SYS_OBJECTS
        assert.o
        stacktrace.o
    )
    # Unpaged from libminc (compiler-rt equivalents)
    set(UNPAGED_MINIC_OBJECTS
        atoi.o
        printf.o
        subr_prf.o
        strcmp.o
        strcpy.o
        strlen.o
        strncmp.o
        memcpy.o
        memmove.o
        memset.o
        udivdi3.o
        umoddi3.o
        qdivrem.o
    )
elseif(MACHINE_ARCH STREQUAL "earm")
    set(UNPAGED_ARCH_OBJECTS
        head.o
        pre_init.o
        direct_tty_utils.o
        pg_utils.o
        klib.o
        utility.o
        arch_reset.o
    )
    set(UNPAGED_MINLIB_OBJECTS
        get_bp.o
    )
    set(UNPAGED_SYS_OBJECTS
        assert.o
        stacktrace.o
    )
    set(UNPAGED_MINIC_OBJECTS
        atoi.o
        printf.o
        subr_prf.o
        strcmp.o
        strcpy.o
        strlen.o
        strncmp.o
        memcpy.o
        memmove.o
        memset.o
        divmodsi4.o
        divsi3.o
        udivsi3.o
        umodsi3.o
        umoddi3.o
        udivmoddi4.o
        __aeabi_idiv0.o
        aeabi_idivmod.o
        aeabi_uidivmod.o
        udivmodsi4.o
        aeabi_uldivmod.o
    )
endif()

# ============================================================================
# Build the kernel
# ============================================================================

# Use add_minix_executable with KERNEL flag
add_minix_executable(kernel
    SOURCES
        ${KERNEL_CORE_SOURCES}
        ${KERNEL_SYSTEM_SOURCES}
        ${KERNEL_ARCH_SOURCES}
    LIBS
        timers
        sys
        exec
    BINDIR
        /usr/sbin
    KERNEL
)

# Kernel-specific flags
target_compile_definitions(kernel PRIVATE __kernel__)

# Linker flags: custom linker script + no standard libs
target_link_options(kernel PRIVATE
    -nostdlib
    -T "${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}/kernel.lds"
    -L${MINIX_DESTDIR}/usr/lib
)

# Include paths (order is important!)
target_include_directories(kernel PRIVATE
    "${MINIX_SOURCE_DIR}/minix"
    "${CMAKE_CURRENT_BINARY_DIR}"       # for generated offset headers
)

# Architecture-specific include paths
target_include_directories(kernel PRIVATE
    "${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}"
    "${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}/include"
    "${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}/bsp/include"
    "${MINIX_SOURCE_DIR}/minix/include/arch/${MACHINE_ARCH}/include"
)

# PAE support
if(MKPAE)
    target_compile_definitions(kernel PRIVATE PAE=1)
endif()

# Gold linker support
if(HAVE_GOLD)
    target_link_options(kernel PRIVATE
        -Wl,--no-ctors-in-init-array
    )
endif()

# Bitcode support (MINIX-specific LLVM feature)
if(USE_BITCODE)
    target_compile_options(kernel PRIVATE -flto)
endif()

# ============================================================================
# Custom targets for header generation
# ============================================================================

# Generate procoffsets.h (assembly offset definitions)
generate_kernel_offsets(TARGET kernel
    CONFIG_FILE "arch/${MACHINE_ARCH}/procoffsets.cf"
    DEPENDS
        "${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}/procoffsets.cf"
        "${CMAKE_CURRENT_SOURCE_DIR}/kernel.h"
        "${CMAKE_CURRENT_SOURCE_DIR}/proc.h"
        "${CMAKE_CURRENT_SOURCE_DIR}/../include/arch/${MACHINE_ARCH}/include/archtypes.h"
)

# Generate extracted headers (for debug.o dependency)
# These are generated by shell scripts from the original Makefile
set(EXTRACTED_HEADER_DIR "${CMAKE_CURRENT_BINARY_DIR}")

# extracted-errno.h: extract error numbers from include/errno.h
set(EXTRACTED_ERRNO_H "${EXTRACTED_HEADER_DIR}/extracted-errno.h")
add_custom_command(
    OUTPUT "${EXTRACTED_ERRNO_H}"
    DEPENDS
        "${CMAKE_CURRENT_SOURCE_DIR}/extract-errno.sh"
        "${MINIX_SOURCE_DIR}/include/errno.h"
    COMMAND
        ${CMAKE_COMMAND} -E env
        HOST_SH="${CMAKE_COMMAND}"
        ${CMAKE_CURRENT_SOURCE_DIR}/extract-errno.sh
        > "${EXTRACTED_ERRNO_H}"
    COMMENT "Generating extracted-errno.h"
)
target_sources(kernel PRIVATE "${EXTRACTED_ERRNO_H}")

# extracted-mfield.h: extract message fields from libc/libsys/*.c
set(EXTRACTED_MFIELD_H "${EXTRACTED_HEADER_DIR}/extracted-mfield.h")
add_custom_command(
    OUTPUT "${EXTRACTED_MFIELD_H}"
    DEPENDS
        "${CMAKE_CURRENT_SOURCE_DIR}/extract-mfield.sh"
    COMMAND
        ${CMAKE_CURRENT_SOURCE_DIR}/extract-mfield.sh
        > "${EXTRACTED_MFIELD_H}"
    COMMENT "Generating extracted-mfield.h"
)
target_sources(kernel PRIVATE "${EXTRACTED_MFIELD_H}")

# extracted-mtype.h: extract message types from minix/com.h
set(EXTRACTED_MTYPE_H "${EXTRACTED_HEADER_DIR}/extracted-mtype.h")
add_custom_command(
    OUTPUT "${EXTRACTED_MTYPE_H}"
    DEPENDS
        "${CMAKE_CURRENT_SOURCE_DIR}/extract-mtype.sh"
        "${MINIX_SOURCE_DIR}/include/minix/com.h"
    COMMAND
        ${CMAKE_CURRENT_SOURCE_DIR}/extract-mtype.sh
        > "${EXTRACTED_MTYPE_H}"
    COMMENT "Generating extracted-mtype.h"
)
target_sources(kernel PRIVATE "${EXTRACTED_MTYPE_H}")

# ============================================================================
# Custom command to copy kernel binary
# ============================================================================

# The kernel build should produce a flat binary (no ELF headers expected
# by some bootloaders, but MINIX uses ELF)
add_custom_target(kernel-image ALL
    COMMAND ${CMAKE_COMMAND} -E copy
        "$<TARGET_FILE:kernel>"
        "${CMAKE_BINARY_DIR}/kernel-${MACHINE_ARCH}.bin"
    DEPENDS kernel
    COMMENT "Copying kernel binary to build directory"
)
