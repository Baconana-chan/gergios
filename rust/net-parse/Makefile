# Makefile — net-parse (Rust library, no_std)
# BSD Make integration for MINIX build system

LIB=    net_parse
SRCS=

build:
	@echo "Building Rust library: net-parse"
	@cd ${.CURDIR} && cargo build --lib ${CARGO_FLAGS}

test:
	@cd ${.CURDIR} && cargo test --lib ${CARGO_FLAGS}

.include <bsd.lib.mk>
