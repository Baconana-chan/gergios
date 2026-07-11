# Makefile for gertoys — consolidated BSD games utilities

.include <bsd.own.mk>

PROG=	gertoys
RUSTC=	rustc
EDITION=	2021

.include <bsd.prog.mk>

# Note: gertoys is built via cargo as part of the Rust workspace.
# This Makefile provides the BSD build system interface.
# The actual build is handled by add_rust_utility in CMakeLists.txt
# or by the cargo build in rust/Makefile.
all:
	@echo "gertoys is built via the Rust workspace (cargo build)"
