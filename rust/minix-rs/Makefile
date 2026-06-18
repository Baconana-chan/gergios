# Makefile for minix-rs (library crate)

.include "../Makefile.inc"
.include <bsd.lib.mk>

build:
	cargo build --lib ${CARGO_FLAGS} --manifest-path ${.CURDIR}/Cargo.toml

test: build
	cargo test --lib --manifest-path ${.CURDIR}/Cargo.toml
