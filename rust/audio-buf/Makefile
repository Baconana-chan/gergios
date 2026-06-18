# Makefile — audio-buf (Rust library, no_std)
# BSD Make integration for MINIX build system

LIB=    audio_buf
SRCS=

build:
	@echo "Building Rust library: audio-buf"
	@cd ${.CURDIR} && cargo build --lib ${CARGO_FLAGS}

test:
	@cd ${.CURDIR} && cargo test --lib ${CARGO_FLAGS}

.include <bsd.lib.mk>
