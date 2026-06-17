# minix/lib/libexec/CMakeLists.txt
# Exec library (libexec)
#
# TODO: Add actual sources from minix/lib/libexec/Makefile
# Currently a stub to maintain CMake target dependency chain.

if(NOT EXISTS \"${CMAKE_CURRENT_SOURCE_DIR}/Makefile\")
    # Create a minimal stub library for dependency resolution
    file(WRITE \"${CMAKE_CURRENT_BINARY_DIR}/libexec_stub.c\"
        \"// libexec stub - replace with actual sources\\n\")
    add_minix_library(exec
        SOURCES \"${CMAKE_CURRENT_BINARY_DIR}/libexec_stub.c\"
        INSTALL_DIR /usr/lib
    )
else()
    # TODO: Parse the Makefile and add real sources here
    add_minix_library(exec
        SOURCES
            # Add sources from minix/lib/libexec/Makefile here
        INSTALL_DIR /usr/lib
    )
endif()
