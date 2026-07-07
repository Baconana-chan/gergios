# Makefile for the Intel Pro/1000 driver.
PROG=	e1000
SRCS=	e1000.c

FILES=${PROG}.conf
FILESNAME=${PROG}
FILESDIR= /etc/system.conf.d

# ── Rust e1000 staticlib integration ───────────────────────────────────────
# The Rust e1000 driver crate (rust/e1000/) compiles to a staticlib that
# contains the actual driver logic.  The C shim (e1000.c) calls it via
# e1000_rust_main().
#
# .PHONY dependency ensures cargo is invoked every time Make evaluates the
# staticlib target.  Cargo handles its own incremental compilation, so it
# quickly exits early when nothing has changed.
#
# NOTE: When passing CARGO_FLAGS=--release, also set CARGO_PROFILE=release
#       so the library path matches the build output directory.

RUST_E1000_DIR?=	${NETBSDSRCDIR}/rust/e1000
.if !exists(${RUST_E1000_DIR})
RUST_E1000_DIR=		${.CURDIR}/../../../../rust/e1000
.endif

CARGO_PROFILE?=		debug
RUST_E1000_LIB=		${RUST_E1000_DIR}/target/${CARGO_PROFILE}/libe1000.a

DPADD+=	${RUST_E1000_LIB} ${LIBNETDRIVER} ${LIBSYS} ${LIBGERGIOSDRIVER}
LDADD+=	-L${RUST_E1000_DIR}/target/${CARGO_PROFILE} -le1000 \
	-lnetdriver -lsys -lgergios_driver

# ── Build rules ────────────────────────────────────────────────────────────
# beforedepend runs before dependency generation; this guarantees the
# Rust staticlib exists when the linker runs.

beforedepend: ${RUST_E1000_LIB}

${RUST_E1000_LIB}: .PHONY ${RUST_E1000_DIR}/Cargo.toml
	@echo "  BUILD  Rust e1000 staticlib (${CARGO_PROFILE})"
	cd ${RUST_E1000_DIR} && cargo build --lib ${CARGO_FLAGS}

# ── C shim headers ─────────────────────────────────────────────────────────
CPPFLAGS+=	-I${.CURDIR}/../../../lib/libgergios_driver

WARNS?= 5

.include <minix.service.mk>
