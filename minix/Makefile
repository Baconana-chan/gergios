# ============================================================================
# *** DEPRECATED: BSD Make (bmake) build system ***
#
# This file is part of the legacy BSD Make build system. All components
# have been migrated to CMake. See the root CMakeLists.txt and cmake/
# directory for the new CMake-based build system.
#
# The BSD Make build system is retained for reference and fallback during
# the transition period. New development should use CMake.
#
# To build with CMake (recommended):
#   mkdir build && cd build
#   cmake .. -DCMAKE_TOOLCHAIN_FILE=../cmake/toolchain-minix.cmake
#   ninja
#
# To build with BSD Make (legacy):
#   ./build.sh build
# ============================================================================

.include <bsd.own.mk>

SUBDIR+=	include .WAIT
SUBDIR+=	bin
SUBDIR+=	commands
SUBDIR+=	fs
SUBDIR+=	kernel
SUBDIR+=	lib
SUBDIR+=	llvm
SUBDIR+=	man
SUBDIR+=	net
SUBDIR+=	sbin
SUBDIR+=	servers
SUBDIR+=	share
.if ${MKATF} == "yes"
SUBDIR+=	tests
.endif
SUBDIR+=	usr.bin
SUBDIR+=	usr.sbin

# BJG - build drivers last as the ramdisk depends on some other drivers
SUBDIR+=	.WAIT drivers

.include <bsd.subdir.mk>
