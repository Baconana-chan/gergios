# minix/lib/libtimers/CMakeLists.txt
# Timer library (libtimers)
#
# Replicates minix/lib/libtimers/Makefile

add_minix_library(timers
    SOURCES
        tmrs_set.c
        tmrs_clr.c
        tmrs_exp.c
    INSTALL_DIR /usr/lib
)
