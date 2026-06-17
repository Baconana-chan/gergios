# minix/servers/rs/CMakeLists.txt
# Reincarnation Server (RS)
#
# Replicates minix/servers/rs/Makefile

add_minix_service(rs
    SOURCES
        exec.c
        main.c
        request.c
        manager.c
        table.c
        utility.c
        error.c
        update.c
    LIBS sys exec
)

# PCI support
if(USE_PCI)
    target_compile_definitions(rs PRIVATE USE_PCI)
endif()

target_include_directories(rs PRIVATE ${MINIX_SOURCE_DIR}/minix)
