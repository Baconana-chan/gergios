# minix/servers/ds/CMakeLists.txt
# Data Store Server (DS)
#
# Replicates minix/servers/ds/Makefile

add_minix_service(ds
    SOURCES
        main.c
        store.c
    LIBS sys
)

# Workaround for linker pass not seeing weak symbols
set_source_files_properties(store.c PROPERTIES
    COMPILE_DEFINITIONS "regcomp=_regcomp;regfree=_regfree"
)
