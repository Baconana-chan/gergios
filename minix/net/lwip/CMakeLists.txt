# minix/net/lwip/CMakeLists.txt
# lwIP TCP/IP socket driver service
#
# Replicates minix/net/lwip/Makefile

add_minix_service(lwip
    SOURCES
        lwip.c
        mempool.c
        pchain.c
        addr.c
        addrpol.c
        tcpisn.c
        mcast.c
        ipsock.c
        pktsock.c
        tcpsock.c
        udpsock.c
        rawsock.c
        ifdev.c
        ifaddr.c
        loopif.c
        ethif.c
        ndev.c
        rttree.c
        route.c
        rtsock.c
        lnksock.c
        lldata.c
        mibtree.c
        ifconf.c
        bpfdev.c
        bpf_filter.c
        util.c
    LIBS lwip sockevent sockdriver chardriver sys timers
)

# Include paths for lwIP
target_include_directories(lwip PRIVATE
    ${MINIX_SOURCE_DIR}/minix/lib/liblwip/dist/src/include
    ${MINIX_SOURCE_DIR}/minix/lib/liblwip/lib
)

# IPv6 support
if(USE_INET6)
    target_compile_definitions(lwip PRIVATE INET6)
endif()

# Suppress lwIP macro warnings
target_compile_options(lwip PRIVATE -Wno-address)

# Install config file
install(FILES lwip.conf DESTINATION /etc/system.conf.d RENAME lwip)
