# Makefile for minix-admin -- GergiOS Admin Shell
#
# Usage:
#   make          Build (release)
#   make test     Run tests
#   make install  Install binary + man page
#   make clean    Clean build artifacts

PROG=		minix-admin
MAN=		minix-admin.8

BINDIR?=	/usr/local/sbin
MANDIR?=	/usr/share/man/man8

.include <bsd.prog.mk>

# Rust build (not using bsd.prog.mk for Rust)
build:
	cargo build --release --bin ${PROG}

test:
	cargo test -p ${PROG}

install: build
	install -d ${DESTDIR}${BINDIR}
	install -m 555 target/release/${PROG} ${DESTDIR}${BINDIR}/
	install -d ${DESTDIR}${MANDIR}
	install -m 444 ${MAN} ${DESTDIR}${MANDIR}/${PROG}.8

clean:
	cargo clean
	rm -f ${PROG}

.PHONY: build test install clean
