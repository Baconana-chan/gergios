# minix/net/uds/CMakeLists.txt
# UNIX Domain Sockets driver (UDS)
#
# Replicates minix/net/uds/Makefile

add_minix_service(uds
    SOURCES
        uds.c
        io.c
        stat.c
    LIBS sockevent sockdriver sys timers
)

# Install config file and man page
install(FILES uds.conf DESTINATION /etc/system.conf.d RENAME uds)
install(FILES unix.8 DESTINATION /usr/share/man/man8)
