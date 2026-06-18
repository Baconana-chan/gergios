# $NetBSD: Makefile,v 1.8 2011/08/17 09:20:14 christos Exp $

LIB=		fetch
SRCS=		fetch.c common.c ftp.c http.c file.c
DPSRCS= 	ftperr.h httperr.h
INCS=		fetch.h
INCSDIR=	/usr/include
MAN=		fetch.3
CLEANFILES=	ftperr.h httperr.h

.include <bsd.init.mk>

CPPFLAGS+=	-I.
CPPFLAGS+=	-DNETBSD 
CPPFLAGS+=	-DFTP_COMBINE_CWDS
CPPFLAGS+=	-DINET6

.if (${MKCRYPTO} != "no")
CPPFLAGS+=	-DWITH_SSL
# wolfSSL migration: replace OpenSSL with wolfSSL
CPPFLAGS+=	-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist
CPPFLAGS+=	-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl
CPPFLAGS+=	-I${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/dist/wolfssl/openssl

LDADD=		-lwolfssl

LIBDPLIBS+=	wolfssl	${NETBSDSRCDIR}/crypto/external/gpl2/wolfssl/lib
.endif

CLEANFILES+=	ftperr.h httperr.h

.PATH: ${LIBFETCHDIR}

ftperr.h: ${LIBFETCHDIR}/ftp.errors ${LIBFETCHDIR}/errlist.sh
	${HOST_SH} ${LIBFETCHDIR}/errlist.sh ftp_errlist FTP \
	    ${LIBFETCHDIR}/ftp.errors > ${.TARGET}

httperr.h: ${LIBFETCHDIR}/http.errors ${LIBFETCHDIR}/errlist.sh
	${HOST_SH} ${LIBFETCHDIR}/errlist.sh http_errlist HTTP \
	    ${LIBFETCHDIR}/http.errors > ${.TARGET}

.include <bsd.lib.mk>
