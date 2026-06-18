# Makefile - BSD Make subdir integration for Rust components
#
# This allows make (BSD Make) to recursively build Rust utilities.
#
# Usage:
#   cd ${NETBSDSRCDIR}/rust && make
#
# Or from the root build:
#   make -C rust

.include <bsd.own.mk>

SUBDIR+=	audio-buf
SUBDIR+=	basename
SUBDIR+=	dirname
SUBDIR+=	echo
SUBDIR+=	false
SUBDIR+=	grep
SUBDIR+=	minix-alloc
SUBDIR+=	minix-driver
SUBDIR+=	minix-rs
SUBDIR+=	net-parse
SUBDIR+=	procfs-path
SUBDIR+=	seq
SUBDIR+=	sleep
SUBDIR+=	true
SUBDIR+=	yes

.include <bsd.subdir.mk>
