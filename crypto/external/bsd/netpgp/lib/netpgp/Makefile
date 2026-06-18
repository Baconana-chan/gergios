# $NetBSD: Makefile,v 1.2 2014/12/05 04:52:02 agc Exp $

.include <bsd.own.mk>

USE_FORT?= yes

LIB= netpgp
SRCS+= compress.c create.c crypto.c
SRCS+= keyring.c misc.c netpgp.c openssl_crypto.c packet-parse.c
SRCS+= packet-print.c packet-show.c reader.c signature.c
SRCS+= symmetric.c validate.c writer.c
SRCS+= ssh2pgp.c bufgap.c
CPPFLAGS+= -I${.CURDIR} -I${EXTDIST}/include
MAN= libnetpgp.3

EXTDIST=${.CURDIR}/../../dist

.PATH: ${EXTDIST}/include ${EXTDIST}/src/lib

INCS+= netpgp.h
INCSDIR=/usr/include

WOLFSSL=${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl
LIBDPLIBS+=	mj		${.CURDIR}/../../libmj
LIBDPLIBS+=	wolfssl		${WOLFSSL}/lib
LIBDPLIBS+=	z		${NETBSDSRCDIR}/lib/libz
LIBDPLIBS+=	bz2		${NETBSDSRCDIR}/lib/libbz2
CPPFLAGS+=	-I${WOLFSSL}/dist
CPPFLAGS+=	-I${WOLFSSL}/dist/wolfssl
CPPFLAGS+=	-I${WOLFSSL}/dist/wolfssl/openssl

.include <bsd.lib.mk>
