# minix/servers/sched/CMakeLists.txt
# Scheduler (SCHED)
#
# Replicates minix/servers/sched/Makefile

add_minix_service(sched
    SOURCES
        main.c
        schedule.c
        utility.c
    LIBS sys
)
