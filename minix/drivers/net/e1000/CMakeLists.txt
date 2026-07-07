# e1000 — Rust Intel PRO/1000 Gigabit Ethernet Driver
#
# The driver logic is implemented in Rust (rust/e1000/).  The Rust staticlib
# is built by the root CMakeLists.txt (add_rust_library(e1000)).  This
# CMakeLists.txt creates a minimal C service executable that links against
# the Rust library.
#
# Dependencies:
#   sys    — PCI, IRQ, VM, SEF, timing (libsys FFI)
#   netdriver — netdriver_task and netdriver data movement (libnetdriver FFI)

# Create the service executable
add_minix_service(e1000
    SOURCES e1000.c
    LIBS netdriver
)

# Link the Rust static library into the service
# Note: rust_e1000 is defined by add_rust_library(e1000) in the root CMakeLists.txt
target_link_libraries(e1000 PRIVATE rust_e1000)
