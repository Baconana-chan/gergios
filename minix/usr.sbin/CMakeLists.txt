# minix/usr.sbin/CMakeLists.txt
# System binary commands
#
# Replicates minix/usr.sbin/Makefile and minix/usr.sbin/Makefile.inc

add_subdirectory_if_exists(mkfs.mfs)

add_minix_executable(btrace
    SOURCES btrace/btrace.c
    BINDIR /usr/sbin
)
install(FILES btrace/btrace.8 DESTINATION /usr/share/man/man8)

add_minix_executable(diskctl
    SOURCES diskctl/diskctl.c
    BINDIR /usr/sbin
)
install(FILES diskctl/diskctl.8 DESTINATION /usr/share/man/man8)

add_minix_executable(fbdctl
    SOURCES fbdctl/fbdctl.c
    BINDIR /usr/sbin
)

add_minix_executable(mkproto
    SOURCES mkproto/mkproto.c
    BINDIR /usr/sbin
)
