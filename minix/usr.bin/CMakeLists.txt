# minix/usr.bin/CMakeLists.txt
# User binary commands
#
# Replicates minix/usr.bin/Makefile and minix/usr.bin/Makefile.inc

# Architecture-specific commands
if(MACHINE_ARCH STREQUAL "earm")
    add_subdirectory(eepromread)
endif()

# grep with links for egrep, fgrep, zgrep, zegrep, zfgrep
add_minix_executable(grep
    SOURCES
        binary.c
        file.c
        grep.c
        mmfile.c
        queue.c
        util.c
    LIBS z
    BINDIR /usr/bin
)

# Install grep man page
install(FILES grep.1 DESTINATION /usr/share/man/man1)

# Create symbolic links for grep variants
install(CODE "
    set(INSTALL_BIN_DIR \"\$ENV{DESTDIR}\${CMAKE_INSTALL_PREFIX}/bin\")
    execute_process(COMMAND \${CMAKE_COMMAND} -E create_symlink grep egrep
        WORKING_DIRECTORY \"\${INSTALL_BIN_DIR}\")
    execute_process(COMMAND \${CMAKE_COMMAND} -E create_symlink grep fgrep
        WORKING_DIRECTORY \"\${INSTALL_BIN_DIR}\")
")

# diff
add_minix_executable(diff
    SOURCES diff.c diffdir.c diffreg.c xmalloc.c
    BINDIR /usr/bin
)

# mined (MINIX editor)
add_minix_executable(mined
    SOURCES mined.c
    LIBS terminfo
    BINDIR /usr/bin
)

# ministat
add_minix_executable(ministat
    SOURCES ministat.c
    LIBS m
    BINDIR /usr/bin
)

# Install test data for ministat
install(FILES chameleon iguana DESTINATION /usr/share/ministat
    OPTIONAL)

# mtop (process monitor, needs curses)
add_minix_executable(mtop
    SOURCES mtop.c
    LIBS curses terminfo
    BINDIR /usr/bin
)

# toproto
add_minix_executable(toproto
    SOURCES toproto.c
    BINDIR /usr/bin
)

# trace
add_minix_executable(trace
    SOURCES trace.c
    BINDIR /usr/bin
)
