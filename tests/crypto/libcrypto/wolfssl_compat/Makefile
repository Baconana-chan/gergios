# $NetBSD: Makefile,v 1.0 2026/06/17 00:00:00 minix Exp $

.include <bsd.own.mk>

.if ${MKCRYPTO} != "no"

WOLFSSLSRC=${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist

PROG=	h_wolfssl_compat
SRCS=	h_wolfssl_compat.c

CPPFLAGS+=-I${WOLFSSLSRC}
CPPFLAGS+=-I${WOLFSSLSRC}/wolfssl
CPPFLAGS+=-I${WOLFSSLSRC}/wolfssl/openssl

LDADD+=	-lwolfssl
DPADD+=	${LIBWOLFSSL}

.include <bsd.prog.mk>

.else
NOPROG=
.include <bsd.prog.mk>
.endif
