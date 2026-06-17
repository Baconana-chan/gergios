# $NetBSD: Makefile,v 1.0 2026/06/17 00:00:00 minix Exp $

.include <bsd.own.mk>

.if ${MKCRYPTO} != "no"

HELPER_NAME=		wolfssl_migrate
HELPER_SRCS=		h_wolfssl_migrate.c

TESTSDIR=	${TESTSBASE}/crypto/libcrypto

PROG=			h_${HELPER_NAME}
SRCS=			${HELPER_SRCS}
MAN=			# empty
BINDIR=			${TESTSDIR}

# wolfSSL library and include paths
LDADD+=			-lwolfssl -lcrypt
DPADD+=			${LIBWOLFSSL} ${LIBCRYPT}

CPPFLAGS+=		-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist
CPPFLAGS+=		-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl
CPPFLAGS+=		-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl/openssl

.include <bsd.test.mk>
.include <bsd.prog.mk>

.else
NOPROG=
.include <bsd.prog.mk>
.endif
