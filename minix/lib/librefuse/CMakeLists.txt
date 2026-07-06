# minix/lib/librefuse/CMakeLists.txt
# librefuse — FUSE compatibility layer on top of libpuffs
#
# Provides the standard FUSE (libfuse) API for writing userspace filesystems.
# Implements the FUSE interface by translating to puffs operations.
#
# Dependencies: librefuse → libpuffs
# Users: Any FUSE-based filesystem (e.g. sshfs, encfs, mfsfuse)

cmake_minimum_required(VERSION 3.20)

# ─── Sources (lib/librefuse/) ──────────────────────────────────────
set(REFUSE_SOURCES
    ${CMAKE_CURRENT_SOURCE_DIR}/../../lib/librefuse/refuse.c
    ${CMAKE_CURRENT_SOURCE_DIR}/../../lib/librefuse/refuse_opt.c
)

# ─── Library target ────────────────────────────────────────────────
add_library(librefuse STATIC
    ${REFUSE_SOURCES}
)

# ─── Include paths ─────────────────────────────────────────────────
target_include_directories(librefuse PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}/../../lib/librefuse
)

# ─── Link dependencies ─────────────────────────────────────────────
target_link_libraries(librefuse PUBLIC libpuffs)

# NOTE: __MINIX is required because librefuse/refuse.c includes <fuse.h> → <puffs.h>
# which has #if defined(__minix) guards for MINIX-specific vs pthread code paths.
target_compile_definitions(librefuse PRIVATE __MINIX)

# ─── Install ───────────────────────────────────────────────────────
install(TARGETS librefuse ARCHIVE DESTINATION lib)
