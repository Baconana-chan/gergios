# minix/servers/pm/CMakeLists.txt
# Process Manager (PM) server
#
# Replicates minix/servers/pm/Makefile

add_minix_service(pm
    SOURCES
        main.c
        forkexit.c
        exec.c
        time.c
        alarm.c
        signal.c
        utility.c
        table.c
        trace.c
        getset.c
        misc.c
        profile.c
        mcontext.c
        schedule.c
        event.c
    LIBS
        sys
        timers
)

# Include paths (matching CPPFLAGS.main.c+= -I..., misc.c+= -I..., etc.)
target_include_directories(pm PRIVATE
    ${MINIX_SOURCE_DIR}/minix
)
