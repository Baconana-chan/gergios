# minix/drivers/tty/CMakeLists.txt
# Terminal driver (TTY)
#
# Replicates minix/drivers/tty/Makefile

add_subdirectory(tty)

if(NOT MKIMAGEONLY)
    # PTY driver (always available)
    add_subdirectory(pty)
endif()
