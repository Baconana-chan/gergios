# minix/drivers/storage/at_wini/CMakeLists.txt
# AT disk driver (AT_WINI)
#
# Replicates minix/drivers/storage/at_wini/Makefile

add_minix_service(at_wini
    SOURCES
        at_wini.c
        liveupdate.c
    LIBS blockdriver sys timers
)
