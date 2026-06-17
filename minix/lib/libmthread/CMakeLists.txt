# minix/lib/libmthread/CMakeLists.txt
# Mini-threads library (libmthread)
#
# Replicates minix/lib/libmthread/Makefile

add_minix_library(mthread
    SOURCES
        allocate.c
        attribute.c
        condition.c
        event.c
        key.c
        misc.c
        mutex.c
        pthread_compat.c
        queue.c
        rwlock.c
        scheduler.c
    INSTALL_DIR /usr/lib
)

target_compile_options(mthread PRIVATE -fno-builtin)
