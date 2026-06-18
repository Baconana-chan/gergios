# Makefile — minix-driver (Rust library)
# BSD Make integration for MINIX build system

LIB=    minix_driver
SRCS=

build:
	@echo "Building Rust library: minix-driver"
	@cd ${.CURDIR} && cargo build --lib ${CARGO_FLAGS}

test:
	@cd ${.CURDIR} && cargo test --lib ${CARGO_FLAGS}

.include <bsd.lib.mk>
