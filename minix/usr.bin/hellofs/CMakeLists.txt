# minix/usr.bin/hellofs/CMakeLists.txt
# hellofs — Minimal FUSE example filesystem
#
# Demonstrates the FUSE API working on MINIX via librefuse.
# Mount: hellofs /mnt/point && cat /mnt/point/hello.txt

cmake_minimum_required(VERSION 3.20)

add_minix_executable(hellofs
    SOURCES main.c
    LIBS refuse
)

# NOTE: hellofs doesn't need -Wl,--wrap,main because it uses
# fuse_main() from librefuse which handles puffs init internally.
