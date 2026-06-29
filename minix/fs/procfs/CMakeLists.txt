# minix/fs/procfs/CMakeLists.txt
# Process filesystem server

add_minix_service(procfs
    SOURCES
        buf.c
        cpuinfo.c
        main.c
        pid.c
        root.c
        service.c
        tree.c
        util.c
    LIBS vtreefs fsdriver sys
)

target_include_directories(procfs PRIVATE
    ${MINIX_SOURCE_DIR}/minix
    ${MINIX_SOURCE_DIR}/minix/fs
    ${MINIX_SOURCE_DIR}/minix/servers
)
