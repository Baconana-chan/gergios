# minix/lib/libpuffs/CMakeLists.txt
# libpuffs — NetBSD Userspace Filesystem (puffs) library
#
# This library combines:
#   1. Core puffs from lib/libpuffs/ (puffs.c, creds.c, subr.c, etc.)
#   2. MINIX adapter from minix/lib/libpuffs/ (fsdriver callbacks, mount, etc.)
#
# The MINIX adapter provides the fsdriver integration layer so that puffs-based
# filesystems run as standard MINIX fsdriver servers.
#
# NOTE: Puffs server binaries MUST add:
#   target_link_options(<server_name> PRIVATE "-Wl,--wrap,main")
# because minix/lib/libpuffs/main.c uses __wrap_main/__real_main to intercept
# the mount request from VFS before the real main() runs.
#
# Dependencies: libpuffs → none (but puffs servers link against libsys, libfsdriver)
# Dependent: librefuse (FUSE compat layer) → depends on libpuffs

cmake_minimum_required(VERSION 3.20)

# ─── Core puffs sources (lib/libpuffs/) ─────────────────────────────
set(PUFFS_CORE_SOURCES
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/callcontext.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/creds.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/dispatcher.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/flush.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/framebuf.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/null.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/opdump.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/paths.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/pnode.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/puffs.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/requests.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/subr.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs/suspend.c
)

# ─── MINIX adapter sources (minix/lib/libpuffs/) ───────────────────
set(PUFFS_MINIX_SOURCES
    inode.c
    link.c
    main.c
    misc.c
    mount.c
    open.c
    path.c
    protect.c
    read.c
    stadir.c
    table.c
    time.c
    utility.c
)

# ─── Library target ────────────────────────────────────────────────
add_library(libpuffs STATIC
    ${PUFFS_CORE_SOURCES}
    ${PUFFS_MINIX_SOURCES}
)

# ─── Include paths ─────────────────────────────────────────────────
target_include_directories(libpuffs PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../lib/libpuffs
    ${CMAKE_CURRENT_SOURCE_DIR}/../../../sys
)

# ─── Compile definitions ───────────────────────────────────────────
target_compile_definitions(libpuffs PRIVATE
    _MINIX_SYSTEM
    __MINIX
    _SYSTEM=1
)

# ─── Install ───────────────────────────────────────────────────────
install(TARGETS libpuffs ARCHIVE DESTINATION lib)
