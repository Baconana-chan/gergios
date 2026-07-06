# minix/usr.bin/mfsfuse/CMakeLists.txt
# mfsfuse — FUSE adapter for read-only access to MFS partitions
#
# Links against librefuse (FUSE compat) which in turn uses libpuffs.
# Mount: mfsfuse /dev/c0d0p1 /mnt/mfs

cmake_minimum_required(VERSION 3.20)

add_minix_executable(mfsfuse
    SOURCES mfsfuse.c
    LIBS refuse
)

# NOTE: mfsfuse uses fuse_main() from librefuse, not __wrap_main,
# so no -Wl,--wrap,main is needed.
