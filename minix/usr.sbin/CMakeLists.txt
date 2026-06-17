# minix/usr.sbin/CMakeLists.txt
# System binary commands
#
# Replicates minix/usr.sbin/Makefile and minix/usr.sbin/Makefile.inc

add_subdirectory(mkfs.mfs)

add_minix_executable(btrace
    SOURCES btrace.c
    BINDIR /usr/sbin
)
install(FILES btrace.8 DESTINATION /usr/share/man/man8)

add_minix_executable(diskctl
    SOURCES diskctl.c
    BINDIR /usr/sbin
)
install(FILES diskctl.8 DESTINATION /usr/share/man/man8)

add_minix_executable(fbdctl
    SOURCES fbdctl.c
    BINDIR /usr/sbin
)

add_minix_executable(mkproto
    SOURCES mkproto.c
    BINDIR /usr/sbin
)
