# Makefile — minix-alloc (Rust library)
# BSD Make integration for MINIX build system

LIB=    minix_alloc
SRCS=

build:
	@echo "Building Rust library: minix-alloc"
	@cd ${.CURDIR} && cargo build --lib ${CARGO_FLAGS}

test:
	@cd ${.CURDIR} && cargo test --lib ${CARGO_FLAGS}

.include <bsd.lib.mk>
