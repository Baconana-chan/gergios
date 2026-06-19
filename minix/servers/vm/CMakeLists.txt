# minix/servers/vm/CMakeLists.txt
# Virtual Memory (VM) server
#
# Replicates minix/servers/vm/Makefile
# VM has architecture-specific sources for its linker script

add_minix_service(vm
    SOURCES
        main.c alloc.c utility.c exit.c fork.c break.c
        mmap.c slaballoc.c region.c pagefaults.c pagetable.c
        rs.c pb.c regionavl.c
        mem_anon.c mem_directphys.c mem_anon_contig.c mem_shared.c
        mem_cache.c cache.c vfs.c mem_file.c fdref.c acl.c
    LIBS sys exec
)

# VM include paths
target_include_directories(vm PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${CMAKE_CURRENT_SOURCE_DIR}/arch/${MACHINE_ARCH}
    ${MINIX_SOURCE_DIR}/minix
)

# PAE support
if(MKPAE)
    target_compile_definitions(vm PRIVATE PAE=1)
endif()

# ARM / ARM64 linker script
if(MACHINE_ARCH STREQUAL "earm")
    target_link_options(vm PRIVATE
        -T "${CMAKE_CURRENT_SOURCE_DIR}/arch/earm/vm.lds"
    )
endif()

if(MACHINE_ARCH STREQUAL "aarch64")
    target_compile_definitions(vm PRIVATE __aarch64__)
endif()

# Magic pass flags (VM-specific: skip malloc instrumentation)
set_target_properties(vm PROPERTIES
    MAGIC_FLAGS "-magic-disable-mem-functions -magic-disable-malloc-skip"
)
