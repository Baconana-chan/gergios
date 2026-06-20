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

# Architecture-specific sources
if(MACHINE_ARCH STREQUAL "earm")
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
elseif(MACHINE_ARCH STREQUAL "x86_64")
    set(KERNEL_ARCH_SOURCES
        arch/x86_64/mpx.S
        arch/x86_64/arch_clock.c
        arch/x86_64/arch_do_vmctl.c
        arch/x86_64/arch_system.c
        arch/x86_64/klib.S
        arch/x86_64/memory.c
        arch/x86_64/protect.c
        arch/x86_64/direct_tty_utils.c
        arch/x86_64/arch_reset.c
        arch/x86_64/pg_utils.c
        arch/x86_64/exception.c
        arch/x86_64/i8259.c
        arch/x86_64/limine.c
    )
elseif(MACHINE_ARCH STREQUAL "aarch64")
    # Phase 2: Kernel Bootstrap + Context Switching + Exception Handling
    set(KERNEL_ARCH_SOURCES
        arch/aarch64/head.S          # ARM64 entry, EL1 init, MMU
        arch/aarch64/vectors.S       # VBAR_EL1 exception vector table
        arch/aarch64/mpx.S           # Context save/restore, syscall dispatch
        arch/aarch64/klib.S          # Architecture library functions
        arch/aarch64/exception.c     # C exception handler, ESR decode, page fault
        arch/aarch64/hw_intr.c       # GICv3 interrupt controller driver
        arch/aarch64/arch_timer.c    # ARM Generic Timer driver
        arch/aarch64/arch_reset.c    # PSCI reset, PL011 UART, shutdown
        arch/aarch64/pg_utils.c      # Page table utilities
        arch/aarch64/arch_system.c   # System calls, CPU init, FPU, context
        arch/aarch64/arch_do_vmctl.c # VM control (TTBR, TLB flush)
        arch/aarch64/startup.c       # C startup: PL011 UART, hello
        arch/aarch64/pre_init.c      # Boot info setup
        arch/aarch64/fdt.c            # Device Tree Blob parser
        arch/aarch64/limine.c         # Limine AAC64 boot protocol
        arch/aarch64/memory.c        # VM: page table walk, copy, memset, phys maps
        arch/aarch64/protect.c       # MMU init, VBAR, page tables, boot procs
    )
endif()

# Optional sources
if(CONFIG_SMP)
    list(APPEND KERNEL_CORE_SOURCES smp.c)
endif()

if(USE_WATCHDOG)
    list(APPEND KERNEL_CORE_SOURCES watchdog.c)
endif()

if(USE_SYSDEBUG)
    list(APPEND KERNEL_CORE_SOURCES debug.c)
endif()

# Profiling
list(APPEND KERNEL_CORE_SOURCES profile.c)
list(APPEND KERNEL_SYSTEM_SOURCES system/do_sprofile.c)

# ============================================================================
# Unpaged object handling
# ============================================================================

# Architecture-specific unpaged objects (head.o, pre_init.o, etc.)
if(MACHINE_ARCH STREQUAL "earm")
    set(UNPAGED_ARCH_OBJECTS
        head.o
        pre_init.o
        direct_tty_utils.o
        pg_utils.o
        klib.o
        utility.o
        arch_reset.o
    )
elseif(MACHINE_ARCH STREQUAL "x86_64")
    set(UNPAGED_ARCH_OBJECTS
        head.o
        pre_init.o
        limine.o
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
elseif(MACHINE_ARCH STREQUAL "aarch64")
    # Phase 2: Kernel Bootstrap — unpaged objects
    set(UNPAGED_ARCH_OBJECTS
        head.o
        vectors.o
        mpx.o
        klib.o
        pre_init.o
        startup.o
    )
    # ARM64 has hardware integer division (UDIV/SDIV), so no
    # software division routines needed for the unpaged section.
    set(UNPAGED_MINLIB_OBJECTS)
    set(UNPAGED_SYS_OBJECTS)
    set(UNPAGED_MINIC_OBJECTS)
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
        "${MINIX_SOURCE_DIR}/minix/include/minix/com.h"
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
