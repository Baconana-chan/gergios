# Makefile — procfs-path (Rust library, no_std)
# BSD Make integration for MINIX build system

LIB=    procfs_path
SRCS=

build:
	@echo "Building Rust library: procfs-path"
	@cd ${.CURDIR} && cargo build --lib ${CARGO_FLAGS}

test:
	@cd ${.CURDIR} && cargo test --lib ${CARGO_FLAGS}

.include <bsd.lib.mk>
