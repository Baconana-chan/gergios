# minix/servers/vfs/CMakeLists.txt
# Virtual File System (VFS) server
#
# Replicates minix/servers/vfs/Makefile

set(VFS_SOURCES
    main.c open.c read.c write.c pipe.c dmap.c
    path.c device.c mount.c link.c exec.c
    filedes.c stadir.c protect.c time.c
    lock.c misc.c utility.c select.c table.c
    vnode.c vmnt.c request.c
    tll.c comm.c worker.c coredump.c
    bdev.c cdev.c sdev.c smap.c socket.c
)

# Coverage support
if(MKCOVERAGE)
    list(APPEND VFS_SOURCES gcov.c)
endif()

add_minix_service(vfs
    SOURCES ${VFS_SOURCES}
    LIBS sys timers exec mthread
)

# VFS compile flags (from CFLAGS += -Wall -Wextra -Wno-sign-compare -Werror)
target_compile_options(vfs PRIVATE
    -Wall -Wextra -Wno-sign-compare -Werror
)
