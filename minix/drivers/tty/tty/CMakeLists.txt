# minix/drivers/tty/tty/CMakeLists.txt
# Terminal driver
#
# Replicates minix/drivers/tty/tty/Makefile

# Architecture-specific sources
if(MACHINE_ARCH STREQUAL "earm")
    set(TTY_ARCH_SOURCES
        arch/earm/arch_tty.c   # example — adjust to actual files
    )
else()
    set(TTY_ARCH_SOURCES "")
endif()

add_minix_service(tty
    SOURCES
        tty.c
        ${TTY_ARCH_SOURCES}
    LIBS chardriver sys timers
)

target_include_directories(tty PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${MINIX_SOURCE_DIR}
)

# Keymaps subdirectory (install only)
install(DIRECTORY keymaps/ DESTINATION /etc/keymaps)
