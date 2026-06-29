# minix/kernel/CMakeLists.txt
# Kernel build (PROTOTYPE)
#
# This replicates minix/kernel/Makefile + minix/kernel/arch/*/Makefile.inc
# using CMake instead of BSD Make.
#
# BUILD STRATEGY
# ==============
# Compilation: Clang (works on Windows MSVC Clang with --target=*-elf)
# Linking:     ld.lld directly (Clang's -fuse-ld=lld/--ld-path fails on
#              Windows MSVC Clang; ld.lld works when invoked directly)
#
# On Windows, Clang (x86_64-pc-windows-msvc) with --target=x86_64-elf tries
# to invoke "gcc" as the linker driver, which doesn't exist. Even -fuse-ld=lld
# and --ld-path fail because Clang can't spawn the linker executable via its
# MSVC-based driver code. ld.lld works perfectly when called directly.

# ============================================================================
# Kernel sources
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

# do_devio / do_vdevio are x86-specific (use inb/outb port I/O).
# They are added to KERNEL_ARCH_SOURCES for x86_64 only.
# ARM64 uses memory-mapped I/O and has separate system calls.

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
    # Core arch sources
    set(KERNEL_ARCH_SOURCES
        arch/x86_64/head.S           # 64-bit boot entry, long mode switch
        arch/x86_64/mpx.S            # Context switch, syscall/irq/exception dispatch
        arch/x86_64/klib.S           # Architecture library (phys_copy, memset, MSR, I/O)
        arch/x86_64/arch_clock.c     # i8253, APIC, TSC timers
        arch/x86_64/arch_do_vmctl.c  # CR3/TLB VM control
        arch/x86_64/arch_system.c    # FPU, CPUID, context, syscall dispatch
        arch/x86_64/arch_reset.c     # Reset/shutdown
        arch/x86_64/direct_tty_utils.c  # Emergency VGA text I/O
        arch/x86_64/exception.c      # Exception handler (64-bit regs)
        arch/x86_64/i8259.c          # 8259 PIC driver
        arch/x86_64/limine.c         # Limine boot protocol
        arch/x86_64/memory.c         # VM: page table walk, copy, memset
        arch/x86_64/pg_utils.c       # Page table utilities (512 entries, 2MB pages)
        arch/x86_64/pre_init.c       # Boot-time kinfo + pagetable setup
        arch/x86_64/protect.c        # GDT/IDT setup, MMU init
        arch/x86_64/do_sdevio.c      # Safe device I/O
        arch/x86_64/apic.c           # APIC stub (TODO: implement)
        arch/x86_64/stubs.c          # Kernel libc stubs (strlcpy, usermapped_offset)
        arch/x86_64/_cpufeature.c    # CPU feature detection
        arch/x86_64/arch_watchdog.c  # NMI watchdog stubs
        arch/x86_64/do_readbios.c    # BIOS read stub
        arch/x86_64/do_iopenable.c   # I/O enable stub
        system/do_devio.c          # x86 port I/O system call
        system/do_vdevio.c         # x86 vectored port I/O system call
    )

    # SMP sources (conditional)
    if(CONFIG_SMP)
        list(APPEND KERNEL_ARCH_SOURCES
            arch/x86_64/arch_smp.c      # SMP initialization
            arch/x86_64/trampoline.S    # AP boot trampoline
        )
    endif()
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
        arch/aarch64/direct_tty_utils.c  # Emergency serial I/O (shutdown/panic)
        arch/aarch64/stubs.c            # Kernel libc stubs (memmove, strlcat, strncmp)
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
        direct_tty_utils.o
        stubs.o
    )
    # ARM64 has hardware integer division (UDIV/SDIV), so no
    # software division routines needed for the unpaged section.
    set(UNPAGED_MINLIB_OBJECTS)
    set(UNPAGED_SYS_OBJECTS)
    set(UNPAGED_MINIC_OBJECTS)
endif()

# ============================================================================
# Step 1: Compile all source files with Clang (OBJECT library)
# ============================================================================
#
# We use an OBJECT library instead of add_executable because Clang's
# linker driver doesn't work on Windows MSVC Clang with --target=*-elf.
# The OBJECT library compiles all .c/.S files into .o files with the
# correct compile flags, but doesn't attempt linking.

add_library(kernel_objects OBJECT
    ${KERNEL_CORE_SOURCES}
    ${KERNEL_SYSTEM_SOURCES}
    ${KERNEL_ARCH_SOURCES}
)

# Kernel-specific compile definitions
target_compile_definitions(kernel_objects PRIVATE __kernel__)

# MINIX compile flags (from add_minix_executable)
target_compile_options(kernel_objects PRIVATE
    -fno-stack-protector
    -fno-builtin
    -fno-asynchronous-unwind-tables
    "$<$<COMPILE_LANGUAGE:ASM>:-D__ASSEMBLY__>"
)

# MINIX system flag
target_compile_definitions(kernel_objects PRIVATE _MINIX_SYSTEM=1)

# Architecture-specific compile options
if(MACHINE_ARCH STREQUAL "x86_64")
    target_compile_options(kernel_objects PRIVATE
        # Use -mcmodel=large instead of -mcmodel=kernel because our
        # kernel virtual base (0xFFFF8000F0100000) is outside the
        # kernel code model range (top 2GB: 0xFFFFFFFF80000000+).
        -mcmodel=large
        -mno-red-zone
    )
elseif(MACHINE_ARCH STREQUAL "aarch64")
    target_compile_options(kernel_objects PRIVATE -mcmodel=large)
endif()

# Include paths
# Order matters — arch-specific paths first, then shared paths,
# then kernel source root last (to allow #include "glo.h" etc.
# without shadowing arch-specific headers).
target_include_directories(kernel_objects PRIVATE
    # From add_minix_executable
    "${MINIX_SOURCE_DIR}/include"
    "${MINIX_SOURCE_DIR}/minix/include"
    "${MINIX_SOURCE_DIR}/sys"
    "${MINIX_SOURCE_DIR}/sys/arch/${MACHINE_ARCH}/include"
    # From kernel CMakeLists.txt
    "${MINIX_SOURCE_DIR}/minix"
    "${CMAKE_CURRENT_BINARY_DIR}"
    # Architecture-specific
    "${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}"
    "${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}/include"
    "${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}/bsp/include"
    "${MINIX_SOURCE_DIR}/minix/include/arch/${MACHINE_ARCH}/include"
    "${MINIX_SOURCE_DIR}/minix/lib/libexec"
    # Kernel source root LAST
    "${CMAKE_CURRENT_SOURCE_DIR}"
)

# PAE support
if(MKPAE)
    target_compile_definitions(kernel_objects PRIVATE PAE=1)
endif()

# Bitcode support
if(USE_BITCODE)
    target_compile_options(kernel_objects PRIVATE -flto)
endif()

# ============================================================================
# Step 2: Link with linker directly (bypass Clang driver)
# ============================================================================
#
# On Windows MSVC Clang (x86_64-pc-windows-msvc), using --target=*-elf
# makes Clang try to invoke "gcc" as the linker driver, which doesn't
# exist. Neither -fuse-ld=lld nor --ld-path work because Clang's MSVC
# process-creation code can't spawn the ELF linker binary.
#
# The fix: use the linker directly. ld.lld (from LLVM) supports both
# x86_64 and AArch64 targets and auto-detects the architecture from
# input object files.

# Select the linker based on architecture
if(MACHINE_ARCH STREQUAL "x86_64")
    # ld.lld from LLVM 21.1.7 — supports ELF64 x86_64
    set(KERNEL_LINKER "C:/PROGRA~1/LLVM/bin/ld.lld" CACHE FILEPATH "Linker for x86_64 kernel")
elseif(MACHINE_ARCH STREQUAL "aarch64")
    # Use existing aarch64-elf-ld cross-linker (GNU binutils)
    # Falls back to ld.lld if aarch64-elf-ld is not available
    find_program(HAVE_AARCH64_LD aarch64-elf-ld
        PATHS "C:/Users/VIC/bin"
        DOC "Path to aarch64-elf-ld cross-linker"
    )
    if(HAVE_AARCH64_LD)
        set(KERNEL_LINKER "${HAVE_AARCH64_LD}" CACHE FILEPATH "Linker for ARM64 kernel")
        message(STATUS "ARM64 linker: ${HAVE_AARCH64_LD}")
    else()
        set(KERNEL_LINKER "ld.lld" CACHE FILEPATH "Linker for ARM64 kernel")
        message(STATUS "ARM64 linker: ld.lld (aarch64-elf-ld not found)")
    endif()
endif()

# Build the kernel binary using the selected linker
set(KERNEL_LDS "${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}/kernel.lds")

add_custom_command(
    OUTPUT ${CMAKE_BINARY_DIR}/kernel-${MACHINE_ARCH}
    COMMAND ${KERNEL_LINKER}
        -nostdlib
        -T "${KERNEL_LDS}"
        "$<$<BOOL:${MINIX_DESTDIR}>:-L${MINIX_DESTDIR}/usr/lib>"
        # Allow multiple symbol definitions (MINIX kernel has intentional
        # duplicates: pre_init.c provides boot-time stubs, other files
        # provide the real implementations used after paging is enabled).
        --allow-multiple-definition
        --error-limit=0
        $<TARGET_OBJECTS:kernel_objects>
        -o ${CMAKE_BINARY_DIR}/kernel-${MACHINE_ARCH}
        # Libraries are passed directly as .a file paths.
        # Note 1: libminc is NOT used here because its build pulls in complex
        # libc header dependencies that fail on Windows cross-compilation.
        # Instead, the needed libminc source files are compiled directly
        # into the kernel_objects OBJECT library (see KERNEL_LIBMINC_SOURCES).
        # Note 2: libsys and libexec are NOT linked because they are compiled
        # with the small code model (default) which generates R_X86_64_32(S)
        # relocations that conflict with the kernel's high virtual address
        # space. The needed functions from these libraries are provided
        # by stubs.c or compiled directly into the kernel.
        $<TARGET_FILE:timers>
    DEPENDS
        kernel_objects
        timers
    COMMENT "Linking kernel with ${KERNEL_LINKER}"
    COMMAND_EXPAND_LISTS
)

# Main kernel target
add_custom_target(kernel ALL
    DEPENDS ${CMAKE_BINARY_DIR}/kernel-${MACHINE_ARCH}
)

# Copy kernel binary (flat binary for bootloaders)
add_custom_target(kernel-image ALL
    COMMAND ${CMAKE_COMMAND} -E copy
        "${CMAKE_BINARY_DIR}/kernel-${MACHINE_ARCH}"
        "${CMAKE_BINARY_DIR}/kernel-${MACHINE_ARCH}.bin"
    DEPENDS kernel
    COMMENT "Copying kernel binary to build directory"
)

# Install
install(FILES ${CMAKE_BINARY_DIR}/kernel-${MACHINE_ARCH}
    DESTINATION /usr/sbin
    RENAME kernel
)

# ============================================================================
# Custom targets for header generation
# ============================================================================

# Generate procoffsets.h (assembly offset definitions)
set(PROCOFFSETS_H "${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}/procoffsets.h")
if(EXISTS "${PROCOFFSETS_H}")
    # Pre-generated file exists — use it directly
    message(STATUS "Using pre-generated procoffsets.h: ${PROCOFFSETS_H}")
    configure_file("${PROCOFFSETS_H}"
        "${CMAKE_CURRENT_BINARY_DIR}/procoffsets.h" COPYONLY)
else()
    generate_kernel_offsets(TARGET kernel_objects
        CONFIG_FILE "arch/${MACHINE_ARCH}/procoffsets.cf"
        DEPENDS
            "${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}/procoffsets.cf"
            "${CMAKE_CURRENT_SOURCE_DIR}/kernel.h"
            "${CMAKE_CURRENT_SOURCE_DIR}/proc.h"
            "${CMAKE_CURRENT_SOURCE_DIR}/../include/arch/${MACHINE_ARCH}/include/archtypes.h"
    )
endif()

# Generate extracted headers (for debug.o dependency)
set(EXTRACTED_HEADER_DIR "${CMAKE_CURRENT_BINARY_DIR}")

# extracted-errno.h: extract error numbers from include/errno.h
set(EXTRACTED_ERRNO_H "${EXTRACTED_HEADER_DIR}/extracted-errno.h")
set(PREGEN_ERRNO_H "${CMAKE_CURRENT_SOURCE_DIR}/extracted-errno.h")
if(EXISTS "${PREGEN_ERRNO_H}")
    message(STATUS "Using pre-generated extracted-errno.h")
    configure_file("${PREGEN_ERRNO_H}" "${EXTRACTED_ERRNO_H}" COPYONLY)
else()
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
endif()
target_sources(kernel_objects PRIVATE "${EXTRACTED_ERRNO_H}")

# extracted-mfield.h: extract message fields from libc/libsys/*.c
set(EXTRACTED_MFIELD_H "${EXTRACTED_HEADER_DIR}/extracted-mfield.h")
set(PREGEN_MFIELD_H "${CMAKE_CURRENT_SOURCE_DIR}/extracted-mfield.h")
if(EXISTS "${PREGEN_MFIELD_H}")
    message(STATUS "Using pre-generated extracted-mfield.h")
    configure_file("${PREGEN_MFIELD_H}" "${EXTRACTED_MFIELD_H}" COPYONLY)
else()
    add_custom_command(
        OUTPUT "${EXTRACTED_MFIELD_H}"
        DEPENDS
            "${CMAKE_CURRENT_SOURCE_DIR}/extract-mfield.sh"
        COMMAND
            ${CMAKE_CURRENT_SOURCE_DIR}/extract-mfield.sh
            > "${EXTRACTED_MFIELD_H}"
        COMMENT "Generating extracted-mfield.h"
    )
endif()
target_sources(kernel_objects PRIVATE "${EXTRACTED_MFIELD_H}")

# extracted-mtype.h: extract message types from minix/com.h
set(EXTRACTED_MTYPE_H "${EXTRACTED_HEADER_DIR}/extracted-mtype.h")
set(PREGEN_MTYPE_H "${CMAKE_CURRENT_SOURCE_DIR}/extracted-mtype.h")
if(EXISTS "${PREGEN_MTYPE_H}")
    message(STATUS "Using pre-generated extracted-mtype.h")
    configure_file("${PREGEN_MTYPE_H}" "${EXTRACTED_MTYPE_H}" COPYONLY)
else()
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
endif()
target_sources(kernel_objects PRIVATE "${EXTRACTED_MTYPE_H}")
