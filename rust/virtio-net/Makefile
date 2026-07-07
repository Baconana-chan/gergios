# Makefile — virtio-net (Rust virtio networking driver library)
# BSD Make integration for MINIX build system

LIB=    virtio_net
SRCS=

build:
	@echo "Building Rust library: virtio-net"
	@cd ${.CURDIR} && cargo build --lib ${CARGO_FLAGS}

test:
	@cd ${.CURDIR} && cargo test --lib ${CARGO_FLAGS}

.include <bsd.lib.mk>
