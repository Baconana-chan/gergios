#	$NetBSD: Makefile,v 1.2 2013/09/11 23:04:10 joerg Exp $

.include <bsd.own.mk>

LIB=event_openssl

SRCS= \
bufferevent_openssl.c

# wolfSSL migration: replace OpenSSL with wolfSSL
LIBDPLIBS+= \
	event	${.CURDIR}/../libevent \
	wolfssl	${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/lib

CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl
CPPFLAGS+=-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl/openssl

.include <bsd.lib.mk>
