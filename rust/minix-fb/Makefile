# Makefile — minix-fb Rust library
#
# This is a library crate, installed as a static library for other Rust
# components to link against.

.include <bsd.own.mk>

PROG=		libminix_fb.a

CARGO_FLAGS?=	--target-dir ${.CURDIR}/target
CARGO=		cargo

.if defined(RELEASE)
CARGO_FLAGS+=	--release
.endif

all: ${PROG}

${PROG}:
	@echo "Building minix-fb library..."
	@cd ${.CURDIR} && ${CARGO} build --lib ${CARGO_FLAGS}
	@cp ${.CURDIR}/target/debug/libminix_fb.rlib ${.OBJDIR}/${PROG}

clean:
	rm -rf ${.CURDIR}/target

.include <bsd.prog.mk>
