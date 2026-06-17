# minix/servers/ipc/CMakeLists.txt
# IPC server
#
# Replicates minix/servers/ipc/Makefile

add_minix_service(ipc
    SOURCES
        main.c
        utility.c
        shm.c
        sem.c
    LIBS sys
)

# Install configuration file
install(FILES ipc.conf
    DESTINATION /etc/system.conf.d
    RENAME ipc
)
