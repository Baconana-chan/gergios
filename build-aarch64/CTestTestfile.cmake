# CMake generated Testfile for 
# Source directory: C:/Users/VIC/gergios
# Build directory: C:/Users/VIC/gergios/build-aarch64
# 
# This file includes the relevant testing commands required for 
# testing this directory and lists subdirectories to be tested as well.
add_test([=[rust_audio-buf]=] "C:/Users/VIC/.cargo/bin/cargo.exe" "test" "--manifest-path" "C:/Users/VIC/gergios/rust/audio-buf/Cargo.toml")
set_tests_properties([=[rust_audio-buf]=] PROPERTIES  WORKING_DIRECTORY "C:/Users/VIC/gergios" _BACKTRACE_TRIPLES "C:/Users/VIC/gergios/CMakeLists.txt;125;add_test;C:/Users/VIC/gergios/CMakeLists.txt;316;add_rust_test;C:/Users/VIC/gergios/CMakeLists.txt;0;")
add_test([=[rust_procfs-path]=] "C:/Users/VIC/.cargo/bin/cargo.exe" "test" "--manifest-path" "C:/Users/VIC/gergios/rust/procfs-path/Cargo.toml")
set_tests_properties([=[rust_procfs-path]=] PROPERTIES  WORKING_DIRECTORY "C:/Users/VIC/gergios" _BACKTRACE_TRIPLES "C:/Users/VIC/gergios/CMakeLists.txt;125;add_test;C:/Users/VIC/gergios/CMakeLists.txt;317;add_rust_test;C:/Users/VIC/gergios/CMakeLists.txt;0;")
add_test([=[rust_net-parse]=] "C:/Users/VIC/.cargo/bin/cargo.exe" "test" "--manifest-path" "C:/Users/VIC/gergios/rust/net-parse/Cargo.toml")
set_tests_properties([=[rust_net-parse]=] PROPERTIES  WORKING_DIRECTORY "C:/Users/VIC/gergios" _BACKTRACE_TRIPLES "C:/Users/VIC/gergios/CMakeLists.txt;125;add_test;C:/Users/VIC/gergios/CMakeLists.txt;318;add_rust_test;C:/Users/VIC/gergios/CMakeLists.txt;0;")
subdirs("minix/lib/libsys")
subdirs("minix/lib/libmthread")
subdirs("minix/lib/libbdev")
subdirs("minix/lib/libblockdriver")
subdirs("minix/lib/libchardriver")
subdirs("minix/lib/libddekit")
subdirs("minix/lib/libexec")
subdirs("minix/lib/libfsdriver")
subdirs("minix/lib/liblwip")
subdirs("minix/lib/libtimers")
subdirs("minix/lib/libvtreefs")
subdirs("minix/kernel")
subdirs("minix/servers")
subdirs("minix/drivers")
subdirs("tests")
