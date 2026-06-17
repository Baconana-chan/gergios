# minix/servers/is/CMakeLists.txt
# Information Server (IS)
#
# Replicates minix/servers/is/Makefile

add_minix_service(is
    SOURCES
        main.c
        dmp.c
        dmp_kernel.c
        dmp_pm.c
        dmp_fs.c
        dmp_rs.c
        dmp_ds.c
        dmp_vm.c
    LIBS sys
)

# Include paths for dump modules (using COMPILE_FLAGS to append, not override)
target_include_directories(is PRIVATE
    ${MINIX_SOURCE_DIR}/minix
    ${MINIX_SOURCE_DIR}/minix/servers
    ${MINIX_SOURCE_DIR}/minix/fs
)

# Must match kernel's IRQ table size
if(USE_APIC)
    target_compile_options(is PRIVATE -DUSE_APIC)
endif()
