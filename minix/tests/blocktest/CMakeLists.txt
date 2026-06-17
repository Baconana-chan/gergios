# minix/tests/blocktest/CMakeLists.txt
# Block device test

add_minix_executable(blocktest
    SOURCES
        # Add sources from minix/tests/blocktest/Makefile
    LIBS sys
    BINDIR /usr/tests
)
