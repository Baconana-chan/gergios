# cmake/macros.cmake
# Reusable CMake macros for MINIX build system
#
# This file provides CMake functions that replicate common patterns
# from bsd.prog.mk and minix.service.mk.

include_guard(GLOBAL)

# ============================================================================
# add_minix_executable
#
# Adds an executable with MINIX-specific flags and linking.
# Replicates the PROG pattern from bsd.prog.mk + minix.service.mk.
#
# Usage:
#   add_minix_executable(name
#       SOURCES file1.c file2.c
#       LIBS sys timers
#       NO_DEFAULT_LIBS
#       BINDIR /usr/sbin
#   )
# ============================================================================

function(add_minix_executable TARGET)
    cmake_parse_arguments(PARSE_ARGV 1 ARG
        "NO_DEFAULT_LIBS;NO_STACK_PROTECTOR;KERNEL"
        "BINDIR"
        "SOURCES;LIBS"
    )

    # Create the executable
    if(ARG_KERNEL)
        # Kernel: special handling with custom linker script
        add_executable(${TARGET} ${ARG_SOURCES})
    else()
        # Normal service or userland program
        add_executable(${TARGET} ${ARG_SOURCES})
    endif()

    # MINIX-specific compile options
    if(NOT ARG_NO_STACK_PROTECTOR)
        target_compile_options(${TARGET} PRIVATE -fno-stack-protector)
    endif()

    # No builtin functions (MINIX has its own minimal libc)
    target_compile_options(${TARGET} PRIVATE -fno-builtin)

    # MINIX system flag
    target_compile_definitions(${TARGET} PRIVATE _MINIX_SYSTEM=1)

    # Assembly flag
    target_compile_options(${TARGET} PRIVATE -D__ASSEMBLY__)

    # Link with MINIX-specific libraries
    if(NOT ARG_NO_DEFAULT_LIBS)
        # Services link against -lsys first
        target_link_libraries(${TARGET} PRIVATE sys)

        if(NOT "${ARG_LIBS}" MATCHES "c")
            # Use libminc instead of libc for service code
            target_link_libraries(${TARGET} PRIVATE minc)
        endif()
    endif()

    # User-specified libraries
    if(ARG_LIBS)
        target_link_libraries(${TARGET} PRIVATE ${ARG_LIBS})
    endif()

    # Install destination
    if(ARG_BINDIR)
        install(TARGETS ${TARGET} RUNTIME DESTINATION ${ARG_BINDIR})
    else()
        install(TARGETS ${TARGET} RUNTIME DESTINATION /usr/sbin)
    endif()

    # Static linking (MINIX default, but NOT for kernel which uses -nostdlib)
    if(NOT ARG_KERNEL)
        target_link_options(${TARGET} PRIVATE -static)
    endif()
endfunction()


# ============================================================================
# add_minix_library
#
# Adds a MINIX static library with proper flags.
# Replicates the LIB pattern from bsd.lib.mk.
#
# Usage:
#   add_minix_library(name
#       SOURCES file1.c file2.c
#       INSTALL_DIR /usr/lib
#   )
# ============================================================================

function(add_minix_library TARGET)
    cmake_parse_arguments(PARSE_ARGV 1 ARG
        ""
        "INSTALL_DIR"
        "SOURCES;LIBS"
    )

    # Create static library (MINIX doesn't use shared libs)
    add_library(${TARGET} STATIC ${ARG_SOURCES})

    # MINIX compile flags
    target_compile_options(${TARGET} PRIVATE
        -fno-stack-protector
        -fno-builtin
    )

    # Dependencies
    if(ARG_LIBS)
        target_link_libraries(${TARGET} PUBLIC ${ARG_LIBS})
    endif()

    # Install
    if(ARG_INSTALL_DIR)
        install(TARGETS ${TARGET} ARCHIVE DESTINATION ${ARG_INSTALL_DIR})
    else()
        install(TARGETS ${TARGET} ARCHIVE DESTINATION /usr/lib)
    endif()
endfunction()


# ============================================================================
# add_unpaged_objects
#
# Creates unpaged kernel objects with __k_unpaged_ symbol prefix.
# Replicates the unpaged object handling from arch/*/Makefile.inc.
#
# Usage:
#   add_unpaged_objects(TARGET kernel
#       OBJECTS head.o pre_init.o
#       FROM_DIR ${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}
#   )
# ============================================================================

function(add_unpaged_objects)
    cmake_parse_arguments(PARSE_ARGV 0 ARG
        ""
        "TARGET;FROM_DIR"
        "OBJECTS"
    )

    if(NOT ARG_TARGET OR NOT ARG_OBJECTS)
        message(FATAL_ERROR "add_unpaged_objects: TARGET and OBJECTS are required")
    endif()

    set(UNPAGED_DIR "${CMAKE_CURRENT_BINARY_DIR}/unpaged")
    file(MAKE_DIRECTORY "${UNPAGED_DIR}")

    foreach(OBJ ${ARG_OBJECTS})
        set(INPUT_OBJ "${ARG_FROM_DIR}/${OBJ}")
        set(OUTPUT_OBJ "${UNPAGED_DIR}/unpaged_${OBJ}")

        # Create a custom command that wraps OBJCOPY to prefix symbols
        add_custom_command(
            OUTPUT "${OUTPUT_OBJ}"
            DEPENDS "${INPUT_OBJ}"
            COMMAND ${CMAKE_OBJCOPY}
                --prefix-symbols=__k_unpaged_
                "${INPUT_OBJ}" "${OUTPUT_OBJ}"
            COMMENT "Creating unpaged object: unpaged_${OBJ}"
        )

        # Add to the target's object list
        target_sources(${ARG_TARGET} PRIVATE "${OUTPUT_OBJ}")
    endforeach()
endfunction()


# ============================================================================
# generate_kernel_offsets
#
# Generates assembly offset headers (procoffsets.h) from C struct definitions.
# Replicates the procoffsets.h generation from arch/*/Makefile.inc.
#
# Usage:
#   generate_kernel_offsets(TARGET kernel
#       CONFIG_FILE procoffsets.cf
#       DEPENDS kernel.h proc.h
#   )
# ============================================================================

function(generate_kernel_offsets)
    cmake_parse_arguments(PARSE_ARGV 0 ARG
        ""
        "TARGET;CONFIG_FILE"
        "DEPENDS"
    )

    set(OFFSETS_OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/procoffsets.h")

    add_custom_command(
        OUTPUT "${OFFSETS_OUTPUT}"
        DEPENDS ${ARG_DEPENDS}
        COMMAND ${CMAKE_COMMAND} -E cat "${CMAKE_CURRENT_SOURCE_DIR}/${ARG_CONFIG_FILE}"
            | ${TOOL_GENASSYM} -- ${CMAKE_C_COMPILER}
                ${CMAKE_C_FLAGS} ${CMAKE_C_FLAGS_${CMAKE_BUILD_TYPE}}
            > "${OFFSETS_OUTPUT}.tmp"
        COMMAND ${CMAKE_COMMAND} -E rename "${OFFSETS_OUTPUT}.tmp" "${OFFSETS_OUTPUT}"
        COMMENT "Generating procoffsets.h"
    )

    target_sources(${ARG_TARGET} PRIVATE "${OFFSETS_OUTPUT}")
    target_include_directories(${ARG_TARGET} PRIVATE "${CMAKE_CURRENT_BINARY_DIR}")
endfunction()


# ============================================================================
# add_minix_service
#
# Wrapper for MINIX system services (servers/drivers).
# Combines add_minix_executable with minix.service.mk conventions.
#
# Usage:
#   add_minix_service(myservice
#       SOURCES main.c helper.c
#       LIBS sys timers
#   )
# ============================================================================

function(add_minix_service TARGET)
    cmake_parse_arguments(PARSE_ARGV 1 ARG
        ""
        ""
        "SOURCES;LIBS"
    )

    add_minix_executable(${TARGET}
        SOURCES ${ARG_SOURCES}
        LIBS ${ARG_LIBS}
        NO_DEFAULT_LIBS
    )

    # Services link with -nodefaultlibs
    target_link_options(${TARGET} PRIVATE -nodefaultlibs)

    # Services use libsys and libminc
    target_link_libraries(${TARGET} PRIVATE sys)

    if(NOT "${ARG_LIBS}" MATCHES "c")
        target_link_libraries(${TARGET} PRIVATE minc)
    endif()
endfunction()
