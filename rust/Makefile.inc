# Makefile.inc - BSD Make integration for Rust components
#
# This file provides the infrastructure to build Rust utilities
# through the BSD Make system (used by MINIX).
#
# Variables:
#   RUST_TARGET_DIR  - Where cargo places build artifacts
#   CARGO_FLAGS      - Additional flags for cargo

RUST_TARGET_DIR?=	${.CURDIR}/target
CARGO_FLAGS?=		--target-dir ${RUST_TARGET_DIR}

.if defined(RELEASE)
CARGO_FLAGS+=		--release
.endif

# Helper target: build a Rust utility
.if defined(PROG)
${PROG}:
	@echo "Building Rust utility: ${PROG}"
	@cd ${.CURDIR} && cargo build --manifest-path ${.CURDIR}/Cargo.toml \
	    --bin ${PROG} ${CARGO_FLAGS}
.endif
