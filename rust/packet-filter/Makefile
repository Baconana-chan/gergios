# Makefile — packet-filter (Rust BPF verifier library, no_std)
# BSD Make integration for MINIX build system

LIB=    packet_filter
SRCS=

build:
	@echo "Building Rust library: packet-filter"
	@cd ${.CURDIR} && cargo build --lib ${CARGO_FLAGS}

test:
	@cd ${.CURDIR} && cargo test --lib ${CARGO_FLAGS}

.include <bsd.lib.mk>
