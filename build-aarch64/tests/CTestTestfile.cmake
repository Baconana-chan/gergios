# CMake generated Testfile for 
# Source directory: C:/Users/VIC/gergios/tests
# Build directory: C:/Users/VIC/gergios/build-aarch64/tests
# 
# This file includes the relevant testing commands required for 
# testing this directory and lists subdirectories to be tested as well.
add_test([=[kernel-compiles]=] "cmake" "--build" "." "--target" "kernel")
set_tests_properties([=[kernel-compiles]=] PROPERTIES  FIXTURES_SETUP "KERNEL_BUILD" WORKING_DIRECTORY "C:/Users/VIC/gergios/build-aarch64" _BACKTRACE_TRIPLES "C:/Users/VIC/gergios/tests/CMakeLists.txt;54;add_test;C:/Users/VIC/gergios/tests/CMakeLists.txt;0;")
add_test([=[kernel-size]=] "test" "-f" "C:/Users/VIC/gergios/build-aarch64/minix/kernel/kernel")
set_tests_properties([=[kernel-size]=] PROPERTIES  FIXTURES_REQUIRED "KERNEL_BUILD" TIMEOUT "10" _BACKTRACE_TRIPLES "C:/Users/VIC/gergios/tests/CMakeLists.txt;64;add_test;C:/Users/VIC/gergios/tests/CMakeLists.txt;0;")
add_test([=[cmake-config-check]=] "C:/Program Files/CMake/bin/cmake.exe" "-S" "C:/Users/VIC/gergios" "-B" "C:/Users/VIC/gergios/build-aarch64/config-check" "--graphviz=\"C:/Users/VIC/gergios/build-aarch64/config-check/graph.dot\"")
set_tests_properties([=[cmake-config-check]=] PROPERTIES  WORKING_DIRECTORY "C:/Users/VIC/gergios/build-aarch64" _BACKTRACE_TRIPLES "C:/Users/VIC/gergios/tests/CMakeLists.txt;107;add_test;C:/Users/VIC/gergios/tests/CMakeLists.txt;0;")
