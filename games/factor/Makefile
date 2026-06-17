#	$NetBSD: Makefile,v 1.12 2005/03/09 03:11:21 christos Exp $
#	@(#)Makefile	8.1 (Berkeley) 5/31/93

# For MKCRYPTO
.include <bsd.own.mk>

PRIMES=	${NETBSDSRCDIR}/games/primes

PROG=	factor
SRCS=	factor.c pr_tbl.c
CPPFLAGS+=-I${PRIMES}

.if (${MKCRYPTO} != "no")
CPPFLAGS+=-DHAVE_OPENSSL
# wolfSSL migration: replace OpenSSL with wolfSSL
LDADD+=	-lwolfssl -lcrypt
DPADD+=	${LIBWOLFSSL} ${LIBCRYPT}
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl/openssl
.endif

MAN=	factor.6
.PATH:	${PRIMES}

.include <bsd.prog.mk>
